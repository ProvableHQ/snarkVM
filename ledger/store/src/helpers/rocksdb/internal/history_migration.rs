// Copyright 2024-2025 Aleo Network Foundation
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

//! A one-time migration of historical mapping updates from little-endian to big-endian
//! height keys.
//!
//! # Why this is done on raw bytes
//!
//! A `MappingUpdateMap` key is `(ProgramID, Identifier, Plaintext, HeightBytes)`, serialized by
//! `bincode` behind a 4-byte context of `[network_id, map_id]`. `HeightBytes` is a `[u8; 4]`,
//! which `bincode` writes as four raw bytes with no length prefix, and it is the *last* field of
//! the tuple. **The height is therefore the final four bytes of the raw key, and the migration is
//! a suffix byte-reversal that leaves the value bytes completely untouched.**
//!
//! Going through the typed `Map` API instead would deserialize a `Plaintext` and a `Value<N>` per
//! entry and immediately re-serialize both to the identical bytes. That round-trip costs ~4 KiB of
//! peak RSS per entry, which is unaffordable here: on mainnet the `credits.aleo` `bonded`,
//! `delegated` and `committee` mappings are rewritten in full by `replace_mapping` on *every*
//! block, so each of their ~507 keys accrues one history entry per block. A node with a few months
//! of history has billions of entries concentrated in a few hundred keys.
//!
//! # Shape of the work
//!
//! Two very different populations share the heights map, and this migration has to be efficient
//! for both:
//!
//! - a few hundred staking keys carrying *millions* of heights each, which dominate the entry
//!   count, and
//! - potentially millions of ordinary keys carrying a handful of heights each, which dominate the
//!   key count.
//!
//! So the scan advances a cursor forward (never re-seeking from the head of the map, which would
//! make each pass skip every tombstone left by the last one), and a single key's history is
//! streamed in bounded chunks rather than materialized.
//!
//! # Endianness collisions
//!
//! Both encodings occupy the same keyspace and are the same width, so a destination key can be
//! byte-identical to a source key that has not been consumed yet: `BE(h')` equals `LE(h)` exactly
//! when `h' == byte_reverse(h)`. Heights whose partner is also present are therefore separated out
//! and migrated together in one final batch, where reading everything before writing anything makes
//! the ordering safe. Every other height provably collides with nothing in the set, so it can be
//! streamed freely. Palindromic heights (`LE(h) == BE(h)`, e.g. 65_792) are their own partner and
//! fall into the same batch.
//!
//! # Resumption
//!
//! A key's heights row is deleted only once all of its entries have moved, so an interrupted
//! migration simply redoes the key it was working on. An entry whose source is already gone is
//! recognised as already-migrated by confirming its destination exists; if *neither* exists the
//! migration fails loudly rather than silently dropping history.

use super::{DataMap, PREFIX_LEN, RocksDB};

use anyhow::{Result, bail, ensure};
use serde::{Serialize, de::DeserializeOwned};
use std::time::{Duration, Instant};

/// The number of historical entries moved per write batch.
///
/// This bounds memory: a single key's history can run to billions of entries, so it is never read
/// in one piece. Entries are small (a raw key plus untouched value bytes), so this is a modest
/// allocation chosen to keep batches large enough to amortize the write path.
const CHUNK_SIZE: usize = 50_000;

/// How often to report progress while migrating.
///
/// This runs before the node serves anything and can take hours on an archive node, so silence
/// here is indistinguishable from a hang.
const REPORT_INTERVAL: Duration = Duration::from_secs(30);

/// What a migration run did, for logging and tests.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MigrationStats {
    /// The number of legacy mapping keys migrated.
    pub keys: u64,
    /// The number of historical entries rewritten from little-endian to big-endian.
    pub entries: u64,
    /// The number of entries found already migrated, i.e. work redone after an interruption.
    pub resumed: u64,
    /// The number of entries that had to be batched together to avoid an encoding collision.
    pub collisions: u64,
}

/// Returns the height whose big-endian encoding is byte-identical to `height`'s little-endian one.
///
/// This is an involution, so it maps a source key to the destination key that would overwrite it
/// and vice versa.
const fn byte_reverse(height: u32) -> u32 {
    u32::from_be_bytes(height.to_le_bytes())
}

/// Returns the raw key for `height` under `prefix`, using the given 4-byte encoding.
fn entry_key(prefix: &[u8], height_bytes: [u8; 4]) -> Vec<u8> {
    let mut key = Vec::with_capacity(prefix.len() + 4);
    key.extend_from_slice(prefix);
    key.extend_from_slice(&height_bytes);
    key
}

/// Migrates every legacy little-endian historical mapping update to a big-endian height key.
///
/// `heights_context` and `update_context` are the 4-byte `[network_id, map_id]` prefixes of the
/// `MappingUpdateHeightsMap` and `MappingUpdateMap` respectively.
///
/// This is intended to run at startup, before the store serves any request, and writes directly to
/// the database rather than through the atomic-batch machinery: there is no concurrent writer, and
/// the batches here are far larger than that path is meant to carry.
pub(crate) fn migrate_legacy_history(
    database: &RocksDB,
    heights_context: &[u8],
    update_context: &[u8],
) -> Result<MigrationStats> {
    ensure!(heights_context.len() == PREFIX_LEN, "Malformed heights-map context");
    ensure!(update_context.len() == PREFIX_LEN, "Malformed update-map context");

    let mut stats = MigrationStats::default();
    let started = Instant::now();
    let mut last_report = started;
    // Whether any work was found. A node with nothing to migrate should say nothing at all.
    let mut announced = false;
    // The raw key of the last heights row migrated, used to resume the forward scan. Re-seeking
    // from the head of the map instead would make every pass walk the tombstones of all previous
    // passes, which is quadratic in the number of keys.
    let mut cursor: Option<Vec<u8>> = None;

    while let Some((heights_key, heights_value)) = next_heights_row(database, heights_context, cursor.as_deref())? {
        if !announced {
            tracing::info!(
                "Migrating legacy history entries to the big-endian height schema. \
                 The node will not serve requests until this completes."
            );
            announced = true;
        }

        // The heights row and its historical entries share a key body; only the context differs.
        let mut prefix = Vec::with_capacity(PREFIX_LEN + heights_key.len() - PREFIX_LEN);
        prefix.extend_from_slice(update_context);
        prefix.extend_from_slice(&heights_key[PREFIX_LEN..]);

        // The only part of a legacy entry that is ever deserialized: the heights themselves.
        let heights: Vec<u32> = bincode::deserialize(&heights_value)?;
        // The legacy write path pushed heights in increasing block order, and the legacy read path
        // binary-searches them, so they are sorted. Verify rather than assume: `binary_search`
        // below is meaningless otherwise, and a wrong answer here would silently drop entries.
        ensure!(heights.is_sorted(), "Legacy heights are not sorted; cannot migrate safely");

        // Separate the heights whose destination would overwrite another height's source.
        let (colliding, isolated): (Vec<u32>, Vec<u32>) =
            heights.iter().partition(|height| heights.binary_search(&byte_reverse(**height)).is_ok());

        // Isolated heights collide with nothing in this key, so they can stream in any order.
        for chunk in isolated.chunks(CHUNK_SIZE) {
            migrate_chunk(database, &prefix, chunk, &mut stats)?;
        }
        // Colliding heights must be read before any of them is written. There are few of them
        // (a height only collides if its byte-reversal is also an update height for this key), so
        // one batch is safe regardless of how deep the history is.
        migrate_chunk(database, &prefix, &colliding, &mut stats)?;
        stats.collisions += colliding.len() as u64;

        // Only now is the key fully migrated, so only now may its heights row go. An interruption
        // before this point simply repeats the key.
        database.delete(&heights_key)?;

        stats.keys += 1;
        cursor = Some(heights_key);

        if last_report.elapsed() >= REPORT_INTERVAL {
            let elapsed = started.elapsed().as_secs_f64();
            tracing::info!(
                "History migration: {} keys, {} entries ({:.0} entries/s)",
                stats.keys,
                stats.entries,
                stats.entries as f64 / elapsed
            );
            last_report = Instant::now();
        }
    }

    if announced {
        tracing::info!(
            "History migration complete: {} keys, {} entries in {:.1?}",
            stats.keys,
            stats.entries,
            started.elapsed()
        );
    }

    Ok(stats)
}

/// Returns the first heights row strictly after `cursor`, or the first row of the map if `cursor`
/// is `None`. Returns `None` once the map is exhausted.
fn next_heights_row(
    database: &RocksDB,
    heights_context: &[u8],
    cursor: Option<&[u8]>,
) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
    let mut iterator = database.raw_iterator();
    match cursor {
        // Seek past the row just migrated. Its own tombstone is the only one skipped, because the
        // scan never rewinds.
        Some(cursor) => {
            iterator.seek(cursor);
            if iterator.valid() && iterator.key() == Some(cursor) {
                iterator.next();
            }
        }
        None => iterator.seek(heights_context),
    }

    if !iterator.valid() {
        iterator.status()?;
        return Ok(None);
    }
    let Some((key, value)) = iterator.item() else {
        iterator.status()?;
        return Ok(None);
    };
    // Stop at the first key outside this map: the database is one keyspace shared by every map,
    // partitioned only by the context prefix.
    if !key.starts_with(heights_context) {
        return Ok(None);
    }
    Ok(Some((key.to_vec(), value.to_vec())))
}

/// Moves `heights` for one mapping key from little-endian to big-endian keys, in a single batch.
///
/// Every source is read before any write is queued, and every deletion is queued before any
/// insertion, so that where a source and a destination are the same raw key the surviving value is
/// the migrated one.
fn migrate_chunk(database: &RocksDB, prefix: &[u8], heights: &[u32], stats: &mut MigrationStats) -> Result<()> {
    if heights.is_empty() {
        return Ok(());
    }

    let legacy_keys = heights.iter().map(|height| entry_key(prefix, height.to_le_bytes())).collect::<Vec<_>>();
    let values = database.multi_get(&legacy_keys);

    // Entries whose source is missing are either already migrated (an interrupted run being
    // repeated) or genuinely absent, which would be history loss. Distinguish the two by looking
    // for the destination, batched so that a resumed run costs one extra lookup per chunk.
    let mut missing = Vec::new();
    for (index, value) in values.iter().enumerate() {
        if value.as_ref().map_err(|e| anyhow::anyhow!("{e}"))?.is_none() {
            missing.push(index);
        }
    }
    if !missing.is_empty() {
        let destinations =
            missing.iter().map(|&index| entry_key(prefix, heights[index].to_be_bytes())).collect::<Vec<_>>();
        for (slot, present) in missing.iter().zip(database.multi_get(&destinations)) {
            if present.map_err(|e| anyhow::anyhow!("{e}"))?.is_none() {
                bail!("Missing legacy mapping update at height {}, and it has no migrated entry", heights[*slot]);
            }
        }
        stats.resumed += missing.len() as u64;
    }

    let mut batch = rocksdb::WriteBatch::default();
    for key in &legacy_keys {
        batch.delete(key);
    }
    for (index, value) in values.into_iter().enumerate() {
        // Already accounted for above; the destination is in place.
        let Some(value) = value.map_err(|e| anyhow::anyhow!("{e}"))? else { continue };
        batch.put(entry_key(prefix, heights[index].to_be_bytes()), value);
        stats.entries += 1;
    }
    database.write(batch)?;

    Ok(())
}

/// Migrates the finalize store's legacy history, given its two maps.
///
/// A thin typed shim over [`migrate_legacy_history`]: it exists only to read the two maps' raw
/// context prefixes and their shared database handle, which the migration works in terms of.
pub(crate) fn migrate_finalize_history<KH, VH, KU, VU>(
    heights_map: &DataMap<KH, VH>,
    update_map: &DataMap<KU, VU>,
) -> Result<MigrationStats>
where
    KH: Serialize + DeserializeOwned,
    VH: Serialize + DeserializeOwned,
    KU: Serialize + DeserializeOwned,
    VU: Serialize + DeserializeOwned,
{
    migrate_legacy_history(&heights_map.database, &heights_map.context, &update_map.context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_reverse_is_an_involution() {
        for height in [0u32, 1, 255, 256, 65_536, 65_792, 1_000_000, 21_639_560, u32::MAX] {
            assert_eq!(byte_reverse(byte_reverse(height)), height);
        }
    }

    #[test]
    fn test_byte_reverse_identifies_colliding_encodings() {
        // A height whose two encodings are identical is its own partner.
        assert_eq!(65_792u32.to_le_bytes(), 65_792u32.to_be_bytes());
        assert_eq!(byte_reverse(65_792), 65_792);

        // Heights whose encodings swap with each other are partners.
        assert_eq!(256u32.to_le_bytes(), 65_536u32.to_be_bytes());
        assert_eq!(byte_reverse(256), 65_536);
        assert_eq!(byte_reverse(65_536), 256);
    }

    #[test]
    fn test_byte_reverse_matches_raw_encodings_exhaustively() {
        // The partner relation is the whole basis for which heights are safe to stream, so check
        // it against the encodings themselves rather than against hand-picked examples.
        let mut rng_state = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..100_000 {
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 7;
            rng_state ^= rng_state << 17;
            let height = rng_state as u32;
            assert_eq!(height.to_le_bytes(), byte_reverse(height).to_be_bytes());
        }
    }
}
