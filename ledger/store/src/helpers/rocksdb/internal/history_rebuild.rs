// Copyright (c) 2019-2026 Provable Inc.
// This file is part of the snarkVM library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:

// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Storage migration v0 -> v1: discard the finalize state and rebuild it by replaying blocks.
//!
//! # What is being repaired
//!
//! Three incompatible layouts of `MappingUpdateMap` shipped in quick succession, and nothing on
//! disk records which one wrote a given entry:
//!
//! | snarkOS | height encoding | `MappingUpdateHeightsMap` |
//! |---|---|---|
//! | <= v4.7.4 | little-endian | written, and lists every entry |
//! | v4.7.5, v4.8.0 | little-endian | **not written** |
//! | v4.8.1+ | big-endian for keys with no heights row, little-endian for keys with one | frozen or appended |
//!
//! # Why the entries cannot be repaired in place
//!
//! `HeightBytes` is the final field of the raw key, so re-encoding an entry is a suffix
//! byte-reversal -- which makes it tempting to classify each entry and move the little-endian ones.
//! That cannot be made complete, because the two encodings collide and the collision already
//! destroyed data on the running node.
//!
//! `LE(256)` and `BE(65_536)` are the same four bytes. A key written at height 256 while the node
//! ran a little-endian build, and again at height 65,536 after it upgraded, produced *one* raw key:
//! the second write overwrote the first, and height 256's value is gone. Nothing on disk can bring
//! it back. The collision needs `byte_reverse(h)` to also be a real height, which for a ~20.8M tip
//! bounds it at roughly 0.6% of the little-endian window -- and `credits.aleo/{committee, delegated,
//! bonded}` reach that bound exactly, because `replace_mapping` rewrites every key in the mapping on
//! every reward block.
//!
//! So any repair that relocates existing entries inherits those holes. Rebuilding the values is the
//! only way to fill them, and the values are recoverable: the blocks are still on disk, and
//! replaying their finalize operations reproduces every mapping update at the height it happened.
//!
//! # What is discarded
//!
//! Everything the finalize and committee stores own. The mapping state is discarded along with the
//! history because a replay must run forward from genesis: reproducing the update at height `h`
//! requires the state as it stood at `h - 1`, and the only state on disk is the one at the tip.
//!
//! Blocks, transactions, transitions and deployments are never touched. They are the source the
//! rebuild reads from.
//!
//! # Why this is raw
//!
//! Clearing a map through the typed API would deserialize every key and value only to drop them.
//! Working on raw prefixes also means the clear runs whatever cargo features are enabled: a build
//! that never opens the typed history map can still discard it.

use aleo_std_storage::StorageMode;
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{
    CommitteeMap,
    Database as _,
    MapID,
    MetadataKey,
    ProgramMap,
    RocksDB,
    get_metadata,
    map_context,
    put_metadata,
};

/// Whether this process may open a ledger whose storage schema predates this build.
///
/// The version gate exists to keep a node out of a database it would misread. The rebuild is the
/// one caller that must open exactly such a database, so it says so explicitly rather than the gate
/// guessing at intent from the storage mode or the calling crate.
static DOWNLEVEL_OPEN_ALLOWED: AtomicBool = AtomicBool::new(false);

/// Permits this process to open a ledger whose storage schema predates this build.
///
/// Intended for the rebuild alone. A node must never call it: the gate is what stops a build from
/// reading little-endian history entries as big-endian.
pub fn allow_downlevel_open() {
    DOWNLEVEL_OPEN_ALLOWED.store(true, Ordering::SeqCst);
}

/// Restores the storage schema gate, undoing [`allow_downlevel_open`].
///
/// The bypass is a process-wide latch, so leaving it set outlives the rebuild that needed it: a
/// rebuild that fails partway would otherwise let the same process go on to open the half-emptied
/// finalize state it just refused to leave behind.
pub fn disallow_downlevel_open() {
    DOWNLEVEL_OPEN_ALLOWED.store(false, Ordering::SeqCst);
}

/// Returns whether [`allow_downlevel_open`] has been called.
pub(crate) fn downlevel_open_allowed() -> bool {
    DOWNLEVEL_OPEN_ALLOWED.load(Ordering::SeqCst)
}

/// The maps the rebuild discards and replays.
///
/// The committee maps are here because the committee store is written by ratification, inside the
/// same atomic batch as the mapping updates, and a replay reproduces it. Leaving it would also
/// leave the rebuild without a resume point: `CommitteeStorage::insert` requires each height to
/// follow the last, so a cleared committee store is what makes a resumed replay verify its own
/// position instead of trusting a recorded cursor.
/// `RejectedReason` is deliberately absent. Its rows are keyed by transaction id rather than by
/// height, so the encoding fault never touched them -- and a replay could not put them back. They
/// are written from `atomic_finalize` only for transaction ids present in `VM::pending_rejected_reasons`,
/// which is populated exclusively by speculation, a step a replay does not perform. Clearing them
/// would empty the map for good.
const REBUILT_MAPS: &[MapID] = &[
    MapID::Program(ProgramMap::ProgramID),
    MapID::Program(ProgramMap::KeyValueID),
    MapID::Program(ProgramMap::MappingUpdate),
    MapID::Program(ProgramMap::MappingUpdateHeights),
    MapID::Program(ProgramMap::StakingRewards),
    MapID::Committee(CommitteeMap::CurrentRound),
    MapID::Committee(CommitteeMap::RoundToHeight),
    MapID::Committee(CommitteeMap::Committee),
];

/// The maps holding historical mapping updates.
///
/// Both are consulted when deciding whether a ledger has history to rebuild: a node that ran only
/// <= v4.7.4 has a heights row for every key, and one that ran only v4.7.5+ has none.
const HISTORY_MAPS: &[MapID] =
    &[MapID::Program(ProgramMap::MappingUpdate), MapID::Program(ProgramMap::MappingUpdateHeights)];

/// Returns the exclusive upper bound of the key range a prefix covers.
///
/// `None` when the prefix is all `0xFF` and so has no successor, meaning the range runs to the end
/// of the keyspace. A map prefix is `[network_id, map_id]` and neither is `0xFFFF` today, but the
/// bound is handled rather than assumed away, since getting it wrong deletes an unrelated map.
fn prefix_end(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut end = prefix.to_vec();
    while let Some(byte) = end.last_mut() {
        if *byte == u8::MAX {
            end.pop();
        } else {
            *byte += 1;
            return Some(end);
        }
    }
    None
}

/// Returns whether any key exists under the given prefix.
fn prefix_is_occupied(database: &rocksdb::DB, prefix: &[u8]) -> bool {
    database.prefix_iterator(prefix).next().is_some()
}

/// Opens the ledger at `storage`, bypassing the storage schema gate for the rest of the process.
///
/// The handle is the one the node's stores already share, so a caller that has a store open gets
/// that same database back rather than a second connection to it.
pub fn open_for_rebuild<S: Into<StorageMode>>(network_id: u16, storage: S) -> Result<RocksDB> {
    allow_downlevel_open();
    RocksDB::open(network_id, storage)
}

/// Returns the storage schema version the database was last written under.
pub fn schema_version(database: &rocksdb::DB, network_id: u16) -> Result<u32> {
    super::get_metadata_u32(database, network_id, MetadataKey::StorageVersion)
}

/// Records the storage schema version the database is now in.
pub fn set_schema_version(database: &rocksdb::DB, network_id: u16, version: u32) -> Result<()> {
    put_metadata(database, network_id, MetadataKey::StorageVersion, &version.to_le_bytes())
}

/// Returns whether the ledger holds historical mapping updates.
///
/// A ledger with none has nothing to rebuild, so the version is stamped in passing and a node that
/// never enabled the `history` feature never sees a rebuild prompt for work that does not exist.
pub fn has_history(database: &rocksdb::DB, network_id: u16) -> Result<bool> {
    Ok(HISTORY_MAPS.iter().any(|map_id| prefix_is_occupied(database, &map_context(network_id, *map_id))))
}

/// Returns whether the ledger holds historical staking rewards.
///
/// Asked separately from the history because it is written under a separate cargo feature, and a
/// build without that feature would discard these entries and rebuild nothing in their place.
pub fn has_staking_rewards(database: &rocksdb::DB, network_id: u16) -> Result<bool> {
    Ok(prefix_is_occupied(database, &map_context(network_id, MapID::Program(ProgramMap::StakingRewards))))
}

/// Returns whether the finalize state has been discarded and is awaiting replay.
///
/// Recorded rather than inferred from an empty finalize store, which cannot distinguish a ledger
/// mid-rebuild from a fresh one: both are empty, but only the first must refuse to serve reads.
pub fn is_rebuilding(database: &rocksdb::DB, network_id: u16) -> Result<bool> {
    Ok(get_metadata(database, network_id, MetadataKey::RebuildInProgress)?.is_some_and(|flag| flag == [1]))
}

/// Records that the finalize state has been discarded, or that the replay has finished.
pub fn set_rebuilding(database: &rocksdb::DB, network_id: u16, rebuilding: bool) -> Result<()> {
    put_metadata(database, network_id, MetadataKey::RebuildInProgress, &[u8::from(rebuilding)])
}

/// Records which optional maps the discarded data occupied, as `(history, staking rewards)`.
///
/// Written by the clear, because after it nothing on disk says what was there. A resumed run cannot
/// re-derive this -- an emptied history map and one that never existed look identical -- and
/// without the record a rebuild begun by a `history` build could be finished by one without it,
/// which would stamp the schema version over a history that had been discarded and never rewritten.
fn record_required_features(database: &rocksdb::DB, network_id: u16, history: bool, rewards: bool) -> Result<()> {
    let mask = u8::from(history) | (u8::from(rewards) << 1);
    put_metadata(database, network_id, MetadataKey::RebuildRequiredFeatures, &[mask])
}

/// Returns which optional maps the discarded data occupied, as `(history, staking rewards)`.
///
/// An absent record reads as neither, which is what a ledger clear of both would have written.
pub fn required_features(database: &rocksdb::DB, network_id: u16) -> Result<(bool, bool)> {
    let mask = get_metadata(database, network_id, MetadataKey::RebuildRequiredFeatures)?
        .and_then(|bytes| bytes.first().copied())
        .unwrap_or(0);
    Ok((mask & 1 != 0, mask & 2 != 0))
}

/// Discards every map the replay reproduces, and compacts the space back.
///
/// Marks the rebuild in progress *before* deleting anything. A crash between the two would
/// otherwise leave a partly emptied finalize store that reports itself intact, which is the one
/// state worse than the corruption being repaired. Ordered the other way the failure is benign: a
/// flag set over an untouched store just makes the next run clear it again, which it would do
/// anyway since this is idempotent.
///
/// The compaction is deliberate. `delete_range` only writes tombstones, and a replay that then
/// seeks through hundreds of millions of them pays for every one on every read.
pub fn clear_rebuilt_state(database: &rocksdb::DB, network_id: u16) -> Result<()> {
    let (history, rewards) = (has_history(database, network_id)?, has_staking_rewards(database, network_id)?);
    record_required_features(database, network_id, history, rewards)?;
    set_rebuilding(database, network_id, true)?;

    for map_id in REBUILT_MAPS {
        let start = map_context(network_id, *map_id);
        let Some(end) = prefix_end(&start) else { continue };

        let mut batch = rocksdb::WriteBatch::default();
        batch.delete_range(&start, &end);
        database.write(batch)?;

        database.compact_range(Some(&start), Some(&end));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_end_carries() {
        assert_eq!(prefix_end(&[0, 0]), Some(vec![0, 1]));
        assert_eq!(prefix_end(&[0, 0xFF]), Some(vec![1]));
        assert_eq!(prefix_end(&[0xFF, 0xFF]), None);
    }

    /// The bound must cover every key of the map, and nothing outside it.
    #[test]
    fn test_prefix_end_bounds_one_map() {
        let start = map_context(3, MapID::Program(ProgramMap::MappingUpdate));
        let end = prefix_end(&start).unwrap();

        // A key of this map, however long its body, sorts below the bound.
        let mut longest = start.clone();
        longest.extend_from_slice(&[0xFF; 64]);
        assert!(longest.as_slice() < end.as_slice());

        // Nothing at or above the bound carries this map's prefix, so no other map is in range.
        assert!(!end.starts_with(&start));
    }
}
