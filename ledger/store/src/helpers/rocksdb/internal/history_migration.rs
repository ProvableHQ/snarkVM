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

//! Storage migration v0 -> v1: rewrite historical mapping updates to big-endian height keys.
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
//! A migration driven by the heights map would therefore be blind to everything written by v4.7.5
//! and v4.8.0 — which, on a node that reverted the v4.8.1 change, is most of its history. This one
//! is driven by `MappingUpdateMap` itself, so the heights map is only ever deleted, never trusted.
//!
//! # Why raw bytes
//!
//! A `MappingUpdateMap` key is `(ProgramID, Identifier, Plaintext, HeightBytes)` behind a 4-byte
//! `[network_id, map_id]` context. `HeightBytes` is a `[u8; 4]`, which bincode writes bare with no
//! length prefix, and it is the last field — so the height is the final four bytes of the raw key,
//! and re-encoding it is a suffix byte-reversal that leaves the value untouched. Going through the
//! typed API would deserialize a `Plaintext` and a `Value` per entry only to re-serialize both to
//! identical bytes, at roughly 4 KiB of peak memory per entry, which an archive node cannot afford:
//! the `credits.aleo` staking mappings are rewritten in full on every block, so a few hundred keys
//! accrue one entry each per block, forever.
//!
//! Working on raw keys also means this runs whatever cargo features are enabled: a build that never
//! opens the typed map can still repair it.
//!
//! # Refusing a mixed database
//!
//! Entries written big-endian by v4.8.1+ cannot be told apart from little-endian ones by
//! inspection — any four bytes are a valid height under either reading. But a *height* is small,
//! and the byte-reversal of a small number is usually large, so an entry whose little-endian
//! reading is implausibly high cannot be little-endian, and its presence proves a v4.8.1+ build
//! wrote here. Rather than guess at a mixture, this refuses the database and asks for a resync.
//!
//! # Resuming
//!
//! A key's entries are ambiguous once partially migrated: a moved entry sits at
//! `BE(h) == LE(byte_reverse(h))` and reads back as a plausible height of its own. So the migration
//! does not re-derive its work from the data. Before touching a key it records that key's original
//! heights in storage metadata, and advances an index into that list as batches commit. An
//! interrupted run reads the list back and continues from the index, never re-deriving what it has
//! already moved.

use super::{MapID, MetadataKey, PREFIX_LEN, ProgramMap, get_metadata, metadata_key};
use serde::{Deserialize, Serialize};

use anyhow::{Result, bail, ensure};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

/// The number of historical entries rewritten per write batch.
///
/// A single key's history can run to millions of entries, so it is never held whole.
const CHUNK_SIZE: usize = 50_000;

/// A ceiling on any block height, used only to bootstrap the search for the real one.
///
/// Deliberately generous. The plausibility bound that decides an entry's encoding is derived from
/// the data itself (see [`observed_tip`]); this constant only has to exclude values that could
/// never be a height under any circumstances, so that the derivation has something certain to work
/// from.
const CEILING_HEIGHT: u32 = 1 << 27;

/// How often to report progress.
const REPORT_INTERVAL: Duration = Duration::from_secs(30);

/// The cursor index recorded once every key has been migrated.
const CURSOR_COMPLETE: u64 = u64::MAX;

/// Running totals, reported on a timer.
///
/// Carried through the per-key work rather than checked only between keys: on the shape this exists
/// for -- a few hundred keys with millions of entries each -- one key is hundreds of batches.
struct Progress {
    started: Instant,
    last_report: Instant,
    keys: u64,
    entries: u64,
}

impl Progress {
    fn new() -> Self {
        let now = Instant::now();
        Self { started: now, last_report: now, keys: 0, entries: 0 }
    }

    fn report(&mut self) {
        if self.last_report.elapsed() < REPORT_INTERVAL {
            return;
        }
        let elapsed = self.started.elapsed().as_secs_f64();
        tracing::info!(
            "Migrating history: {} keys, {} entries ({:.0} entries/s)",
            self.keys,
            self.entries,
            self.entries as f64 / elapsed
        );
        self.last_report = Instant::now();
    }
}

/// How an entry's four height bytes should be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    /// Written by snarkOS <= v4.8.0; needs migrating.
    Little,
    /// Written by snarkOS v4.8.1+; already in the target form.
    Big,
}

/// The entries of one mapping key, sorted by the encoding each was written in.
struct KeyEntries {
    /// Heights still in little-endian form, ascending. These are the work.
    little: Vec<u32>,
    /// How many entries were already big-endian.
    big: u64,
    /// Entries whose encoding cannot be settled from the database alone.
    undecidable: Vec<Undecidable>,
}

/// An entry that reads as a plausible height under either encoding, on a key holding both.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Undecidable {
    /// The height this reads as if written little-endian.
    pub little: u32,
    /// The height this reads as if written big-endian.
    pub big: u32,
}

/// What a migration would do, determined without writing anything.
#[derive(Debug, Default, Clone)]
pub struct RepairPlan {
    /// The storage schema version recorded in the database.
    pub schema_version: u32,
    /// The highest height any entry could be, derived from the entries themselves.
    pub tip: u32,
    /// The number of mapping keys holding history.
    pub keys: u64,
    /// Entries to be rewritten.
    pub little_endian: u64,
    /// Entries already in the target form.
    pub big_endian: u64,
    /// Entries whose height cannot be determined. A non-empty list means the ledger is unrepairable.
    pub undecidable: Vec<Undecidable>,
}

/// Returns the height whose big-endian encoding is byte-identical to `height`'s little-endian one.
const fn byte_reverse(height: u32) -> u32 {
    u32::from_be_bytes(height.to_le_bytes())
}

/// Returns the 4-byte `[network_id, map_id]` prefix of a map.
fn context(network_id: u16, map_id: MapID) -> Vec<u8> {
    let mut raw = Vec::with_capacity(PREFIX_LEN);
    raw.extend_from_slice(&network_id.to_le_bytes());
    raw.extend_from_slice(&u16::from(map_id).to_le_bytes());
    raw
}

/// Returns the raw key for `height` under a mapping key's prefix.
fn entry_key(prefix: &[u8], height_bytes: [u8; 4]) -> Vec<u8> {
    let mut key = Vec::with_capacity(prefix.len() + 4);
    key.extend_from_slice(prefix);
    key.extend_from_slice(&height_bytes);
    key
}

/// Returns whether the ledger holds any historical mapping data at all.
///
/// One seek. Lets a node distinguish "this ledger needs migrating" from "there is nothing here to
/// migrate", which is the common case for a node that never enabled the `history` feature.
pub(crate) fn has_history(database: &rocksdb::DB, network_id: u16) -> Result<bool> {
    let update_context = context(network_id, MapID::Program(ProgramMap::MappingUpdate));
    let mut iterator = database.raw_iterator();
    iterator.seek(&update_context);
    if !iterator.valid() {
        iterator.status()?;
        return Ok(false);
    }
    Ok(iterator.key().is_some_and(|key| key.starts_with(&update_context)))
}

/// Returns a lower bound on the chain tip, derived from the entries themselves.
///
/// The plausibility of a reading is what separates the two encodings, and a height is bounded by
/// the chain tip -- which the storage layer does not know. It can be derived, though: exactly one
/// of an entry's two readings is its real height, so the true tip is at least the *smaller* of
/// them. Taking the largest such value over every entry therefore yields a bound that no real
/// height exceeds, which is the property that matters: judged against it, every entry keeps at
/// least one plausible reading, so a correct ledger can never be condemned as corrupt.
///
/// Deriving it matters more than it looks. Judged against [`CEILING_HEIGHT`] alone, an entry is
/// ambiguous whenever its height's low byte is small -- one entry in thirty-two. Judged against the
/// real tip, only entries whose *both* readings are real heights are ambiguous, which on a
/// 76,514-block ledger is a handful of byte patterns rather than 1,672.
fn observed_tip(database: &rocksdb::DB, update_context: &[u8]) -> Result<u32> {
    let mut tip = 0u32;
    let mut iterator = database.raw_iterator();
    iterator.seek(update_context);
    while iterator.valid() {
        let Some(key) = iterator.key() else { break };
        if !key.starts_with(update_context) {
            break;
        }
        ensure!(
            key.len() >= update_context.len() + 4,
            "Malformed historical mapping key of {} bytes; the ledger is corrupt and must be \
             resynced from genesis",
            key.len()
        );
        let suffix = <[u8; 4]>::try_from(&key[key.len() - 4..]).expect("checked length");
        tip = tip.max(u32::from_le_bytes(suffix).min(u32::from_be_bytes(suffix)));
        iterator.next();
    }
    iterator.status()?;
    ensure!(
        tip < CEILING_HEIGHT,
        "Historical mapping entries imply a chain tip of {tip}, which is not a plausible block \
         height; the ledger is corrupt and must be resynced from genesis"
    );
    Ok(tip)
}

/// Reads every entry of one mapping key and decides how each was written.
///
/// Both encodings can occur under a single key, and this is the expected shape rather than an edge
/// case: a key first written during the v4.7.5/v4.8.0 window has no heights row, so when a v4.8.1+
/// build took over it took the big-endian branch for that key while its earlier entries stayed
/// little-endian.
///
/// A reading above `tip` cannot be a height, which settles almost every entry. Where both readings
/// are plausible, a key that shows no evidence of one encoding was never written in it. Only a key
/// genuinely holding both, with an entry that could belong to either, is undecidable -- and that is
/// reported rather than guessed at, because the two candidates are equally consistent with the
/// bytes on disk and choosing wrongly would move an entry to a height it never had.
fn classify_entries(database: &rocksdb::DB, update_context: &[u8], body: &[u8], tip: u32) -> Result<KeyEntries> {
    let mut prefix = Vec::with_capacity(update_context.len() + body.len());
    prefix.extend_from_slice(update_context);
    prefix.extend_from_slice(body);

    let mut suffixes = Vec::new();
    let mut iterator = database.raw_iterator();
    iterator.seek(&prefix);
    while iterator.valid() {
        let Some(key) = iterator.key() else { break };
        if !key.starts_with(&prefix) {
            break;
        }
        // A key under this prefix that is not an entry of this mapping key means the layout is not
        // what this migration understands. Stopping quietly here would silently leave the rest of
        // the key unmigrated.
        ensure!(
            key.len() == prefix.len() + 4,
            "Malformed historical mapping entry of {} bytes under a {}-byte key prefix; the ledger \
             is corrupt and must be resynced from genesis",
            key.len(),
            prefix.len()
        );
        suffixes.push(<[u8; 4]>::try_from(&key[prefix.len()..]).expect("checked length"));
        iterator.next();
    }
    iterator.status()?;

    let mut little = Vec::new();
    let mut big = 0u64;
    let mut ambiguous = Vec::new();
    let (mut saw_little, mut saw_big) = (false, false);
    for suffix in &suffixes {
        let as_little = u32::from_le_bytes(*suffix);
        let as_big = u32::from_be_bytes(*suffix);
        match (as_little <= tip, as_big <= tip) {
            (true, false) => {
                saw_little = true;
                little.push(as_little);
            }
            (false, true) => {
                saw_big = true;
                big += 1;
            }
            (true, true) => ambiguous.push((as_little, as_big)),
            // Unreachable: the tip is derived as the largest of every entry's smaller reading, so
            // each entry retains at least one reading at or below it.
            (false, false) => bail!(
                "Historical mapping entry with height bytes {suffix:?} reads as {as_little} and \
                 {as_big}, both above the derived chain tip of {tip}; the ledger is corrupt and \
                 must be resynced from genesis"
            ),
        }
    }

    // A key showing no evidence of an encoding was never written in it, which settles the ambiguous
    // entries outright. Only a key holding both leaves anything genuinely open.
    let mut undecidable = Vec::new();
    for (as_little, as_big) in ambiguous {
        // A palindromic suffix reads as the same height either way, so there is nothing to decide:
        // whichever build wrote it, the entry is already at the key a big-endian write would give
        // it. Counted as migrated rather than moved, to avoid a write that changes nothing.
        if as_little == as_big {
            big += 1;
            continue;
        }
        match (saw_little, saw_big) {
            (_, false) => little.push(as_little),
            (false, true) => big += 1,
            (true, true) => undecidable.push(Undecidable { little: as_little, big: as_big }),
        }
    }

    little.sort_unstable();
    Ok(KeyEntries { little, big, undecidable })
}

/// Determines what a migration would do, without writing anything.
///
/// Shares its classification with [`migrate`], so the two cannot drift: an operator asking whether
/// a ledger is repairable gets the answer the migration itself would reach.
pub fn plan(database: &rocksdb::DB, network_id: u16) -> Result<RepairPlan> {
    let update_context = context(network_id, MapID::Program(ProgramMap::MappingUpdate));
    let mut report = RepairPlan {
        schema_version: super::get_metadata_u32(database, network_id, MetadataKey::StorageVersion)?,
        tip: observed_tip(database, &update_context)?,
        ..Default::default()
    };

    let mut cursor: Option<Vec<u8>> = None;
    while let Some(body) = next_body(database, &update_context, cursor.as_deref())? {
        let classified = classify_entries(database, &update_context, &body, report.tip)?;
        report.keys += 1;
        report.little_endian += classified.little.len() as u64;
        report.big_endian += classified.big;
        report.undecidable.extend(classified.undecidable);
        cursor = Some(body);
    }
    Ok(report)
}

/// Rewrites every historical mapping update to a big-endian height key.
///
/// Decides everything before writing anything. The whole map is classified first, and if any entry
/// cannot be attributed to a height the migration stops having written nothing at all -- so a
/// ledger reported as unrepairable stays exactly as it was, and no restart can turn that verdict
/// into a silent success.
pub fn migrate(database: &rocksdb::DB, network_id: u16) -> Result<()> {
    let update_context = context(network_id, MapID::Program(ProgramMap::MappingUpdate));
    let heights_context = context(network_id, MapID::Program(ProgramMap::MappingUpdateHeights));

    // Resume state from an interrupted run, which also carries the tip that run decided on: a
    // partially migrated map would derive a different one.
    let resume = match get_metadata(database, network_id, MetadataKey::StorageMigrationCursor)? {
        Some(bytes) => Some(bincode::deserialize::<(Vec<u8>, u64, u32)>(&bytes)?),
        None => None,
    };
    if matches!(&resume, Some((_, index, _)) if *index == CURSOR_COMPLETE) {
        return Ok(());
    }

    let tip = match &resume {
        Some((_, _, tip)) => *tip,
        None => {
            // Nothing has been written yet, so this is the moment to refuse. Everything is
            // classified up front; only once that succeeds does anything move.
            let plan = plan(database, network_id)?;
            if !plan.undecidable.is_empty() {
                let first = &plan.undecidable[0];
                bail!(
                    "{} historical mapping entries cannot be attributed to a block height. The \
                     first reads as height {} little-endian and {} big-endian, and its key holds \
                     entries in both encodings, so the two are equally consistent with what is on \
                     disk. Nothing has been modified. The ledger must be resynced from genesis.",
                    plan.undecidable.len(),
                    first.little,
                    first.big
                );
            }
            tracing::info!(
                "Migrating {} historical mapping entries across {} keys ({} already migrated)",
                plan.little_endian,
                plan.keys,
                plan.big_endian
            );
            plan.tip
        }
    };

    let mut progress = Progress::new();
    let (mut cursor, mut resume_index) = match resume {
        Some((body, index, _)) => (Some(body), index),
        None => (None, 0),
    };

    loop {
        // A cursor is only ever written after work has been committed, so it always names a key
        // that is genuinely part-done. There is no "recorded but not started" state to mistake for
        // "finished".
        let body = match (cursor.take(), resume_index > 0) {
            (Some(body), true) => body,
            (previous, _) => {
                resume_index = 0;
                match next_body(database, &update_context, previous.as_deref())? {
                    Some(next) => next,
                    None => break,
                }
            }
        };

        let heights = match resume_index > 0 {
            true => read_recorded_heights(database, network_id)?,
            false => classify_entries(database, &update_context, &body, tip)?.little,
        };

        let mut prefix = Vec::with_capacity(update_context.len() + body.len());
        prefix.extend_from_slice(&update_context);
        prefix.extend_from_slice(&body);

        migrate_key(database, network_id, &prefix, &body, &heights, resume_index, tip, &mut progress)?;
        resume_index = 0;
        progress.keys += 1;
        progress.report();
        cursor = Some(body);
    }

    // The heights map is not consulted by anything after this point, and keeping it would leave the
    // read-modify-write path that made it grow in the first place.
    drop_map(database, &heights_context)?;
    let mut batch = rocksdb::WriteBatch::default();
    batch.put(
        metadata_key(network_id, MetadataKey::StorageMigrationCursor),
        bincode::serialize(&(Vec::<u8>::new(), CURSOR_COMPLETE, tip))?,
    );
    batch.delete(metadata_key(network_id, MetadataKey::StorageMigrationHeights));
    database.write(batch)?;

    tracing::info!(
        "History migration complete: {} keys, {} entries in {:.1?}",
        progress.keys,
        progress.entries,
        progress.started.elapsed()
    );
    Ok(())
}

/// Reads back the heights recorded for an interrupted key.
fn read_recorded_heights(database: &rocksdb::DB, network_id: u16) -> Result<Vec<u32>> {
    match get_metadata(database, network_id, MetadataKey::StorageMigrationHeights)? {
        Some(bytes) => Ok(bincode::deserialize(&bytes)?),
        None => bail!("A history migration was interrupted, but the heights it was working on are missing"),
    }
}

/// Returns the next mapping key body strictly after `after`, or the first one when `after` is
/// `None`. Returns `None` once the map is exhausted.
///
/// Bodies are prefix-free -- a `Plaintext` is length-delimited, so one complete encoding cannot be
/// a strict prefix of another -- so all of a key's entries are contiguous and this never revisits
/// one.
fn next_body(database: &rocksdb::DB, update_context: &[u8], after: Option<&[u8]>) -> Result<Option<Vec<u8>>> {
    let mut iterator = database.raw_iterator();
    match after {
        Some(after) => {
            let mut limit = Vec::with_capacity(update_context.len() + after.len() + 4);
            limit.extend_from_slice(update_context);
            limit.extend_from_slice(after);
            limit.extend_from_slice(&[0xFF; 4]);
            iterator.seek(&limit);
            if iterator.valid() && iterator.key() == Some(limit.as_slice()) {
                iterator.next();
            }
        }
        None => iterator.seek(update_context),
    }

    if !iterator.valid() {
        iterator.status()?;
        return Ok(None);
    }
    let Some(key) = iterator.key() else {
        iterator.status()?;
        return Ok(None);
    };
    if !key.starts_with(update_context) {
        return Ok(None);
    }
    // Stopping quietly on a malformed key would leave every key after it unmigrated, with no error.
    ensure!(
        key.len() >= update_context.len() + 4,
        "Malformed historical mapping key of {} bytes; the ledger is corrupt and must be resynced \
         from genesis",
        key.len()
    );
    Ok(Some(key[update_context.len()..key.len() - 4].to_vec()))
}

/// Groups colliding heights with their partners, so a batch never splits a pair.
fn partner_groups(colliding: &[u32]) -> Vec<Vec<u32>> {
    let mut claimed = HashSet::with_capacity(colliding.len());
    let mut groups = Vec::new();
    for &height in colliding {
        if !claimed.insert(height) {
            continue;
        }
        let partner = byte_reverse(height);
        if partner == height {
            groups.push(vec![height]);
        } else {
            claimed.insert(partner);
            groups.push(vec![height, partner]);
        }
    }
    groups
}

/// Packs whole groups into batches of at most `limit` heights, so partners stay together.
fn chunk_groups(groups: &[Vec<u32>], limit: usize) -> Vec<Vec<u32>> {
    let mut chunks = Vec::new();
    let mut current: Vec<u32> = Vec::new();
    for group in groups {
        if !current.is_empty() && current.len() + group.len() > limit {
            chunks.push(std::mem::take(&mut current));
        }
        current.extend_from_slice(group);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Returns the batches this key will be migrated in, in the order they will be applied.
///
/// Deterministic in `heights` alone, so a resumed run rebuilds exactly the same list and can skip
/// the batches already committed by counting entries.
fn work_order(heights: &[u32]) -> Vec<Vec<u32>> {
    let lookup = heights.iter().copied().collect::<HashSet<_>>();
    let (colliding, isolated): (Vec<u32>, Vec<u32>) =
        heights.iter().partition(|height| lookup.contains(&byte_reverse(**height)));
    let mut batches: Vec<Vec<u32>> = isolated.chunks(CHUNK_SIZE).map(<[u32]>::to_vec).collect();
    batches.extend(chunk_groups(&partner_groups(&colliding), CHUNK_SIZE));
    batches
}

/// Migrates one mapping key, skipping the batches a previous run already committed.
#[allow(clippy::too_many_arguments)]
fn migrate_key(
    database: &rocksdb::DB,
    network_id: u16,
    prefix: &[u8],
    body: &[u8],
    heights: &[u32],
    start: u64,
    tip: u32,
    progress: &mut Progress,
) -> Result<()> {
    let mut processed = 0u64;
    let mut first_write = true;
    for batch in work_order(heights) {
        let next = processed + batch.len() as u64;
        // Batches are atomic, so a committed one is skipped whole.
        if next <= start {
            processed = next;
            continue;
        }
        // The heights this key is being migrated against are recorded with the first batch that
        // actually writes, never before: a cursor naming a key with nothing committed could not be
        // told apart from one naming a key that is finished.
        let record = first_write.then_some(heights);
        progress.entries += write_chunk(database, network_id, prefix, &batch, body, next, tip, record)?;
        processed = next;
        first_write = false;
        progress.report();
    }
    Ok(())
}

/// Moves a batch of heights to big-endian keys, recording the progress in the same write.
///
/// This is the invariant the resume design rests on: the cursor and the data it describes commit
/// together or not at all. Written separately, an interruption between them replays a batch that
/// already committed -- which for byte-reversal partners silently swaps their values back, since
/// each one's source is the other's destination.
///
/// Within the batch every source is read before any write is queued, and deletions precede
/// insertions, so where a source and a destination are the same raw key the migrated value wins.
#[allow(clippy::too_many_arguments)]
fn write_chunk(
    database: &rocksdb::DB,
    network_id: u16,
    prefix: &[u8],
    heights: &[u32],
    body: &[u8],
    processed: u64,
    tip: u32,
    record_heights: Option<&[u32]>,
) -> Result<u64> {
    if heights.is_empty() {
        return Ok(0);
    }
    let sources = heights.iter().map(|height| entry_key(prefix, height.to_le_bytes())).collect::<Vec<_>>();
    let values = database.multi_get(&sources);

    let mut moved = Vec::with_capacity(heights.len());
    for (index, value) in values.into_iter().enumerate() {
        let Some(value) = value.map_err(|e| anyhow::anyhow!("{e}"))? else {
            bail!("Missing historical mapping entry at height {}", heights[index]);
        };
        moved.push((heights[index], value));
    }

    // A destination that is neither one of this batch's own sources nor empty holds a different
    // entry, and writing over it would destroy it.
    let sources_set = sources.iter().collect::<HashSet<_>>();
    for (height, _) in &moved {
        let destination = entry_key(prefix, height.to_be_bytes());
        if !sources_set.contains(&destination) && database.get(&destination)?.is_some() {
            bail!(
                "The big-endian key for height {height} is already occupied by a different entry; \
                 the ledger is corrupt and must be resynced from genesis"
            );
        }
    }

    let mut batch = rocksdb::WriteBatch::default();
    for source in &sources {
        batch.delete(source);
    }
    let count = moved.len() as u64;
    for (height, value) in moved {
        batch.put(entry_key(prefix, height.to_be_bytes()), value);
    }
    if let Some(heights) = record_heights {
        batch.put(metadata_key(network_id, MetadataKey::StorageMigrationHeights), bincode::serialize(&heights)?);
    }
    batch.put(
        metadata_key(network_id, MetadataKey::StorageMigrationCursor),
        bincode::serialize(&(body, processed, tip))?,
    );
    database.write(batch)?;

    Ok(count)
}

/// Removes every entry under a map's prefix.
fn drop_map(database: &rocksdb::DB, map_context: &[u8]) -> Result<()> {
    let mut upper = map_context.to_vec();
    // The exclusive upper bound of the prefix: the next map's context.
    for byte in upper.iter_mut().rev() {
        match *byte {
            0xFF => *byte = 0x00,
            _ => {
                *byte += 1;
                break;
            }
        }
    }
    let mut batch = rocksdb::WriteBatch::default();
    batch.delete_range(map_context, &upper);
    Ok(database.write(batch)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETWORK: u16 = 0;

    /// Opens an empty database configured exactly as `RocksDB::open` configures production.
    ///
    /// The prefix extractor matters: with one installed, `raw_iterator` runs in prefix-seek mode,
    /// where iterating beyond the seeked key's extracted prefix is not guaranteed. The migration
    /// seeks to keys longer than the prefix and walks off the end of a map by design, and
    /// `drop_map` issues a range delete whose endpoints straddle a prefix boundary. Opening these
    /// tests with default options would exercise none of that.
    fn database() -> (rocksdb::DB, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut options = rocksdb::Options::default();
        options.create_if_missing(true);
        options.set_compression_type(rocksdb::DBCompressionType::Lz4);
        options.set_prefix_extractor(rocksdb::SliceTransform::create_fixed_prefix(PREFIX_LEN));
        let db = rocksdb::DB::open(&options, dir.path()).expect("open");
        (db, dir)
    }

    fn update_ctx() -> Vec<u8> {
        context(NETWORK, MapID::Program(ProgramMap::MappingUpdate))
    }

    fn heights_ctx() -> Vec<u8> {
        context(NETWORK, MapID::Program(ProgramMap::MappingUpdateHeights))
    }

    /// Writes a legacy little-endian entry for `body` at `height`.
    fn seed(db: &rocksdb::DB, body: &[u8], height: u32, value: &[u8]) {
        let mut prefix = update_ctx();
        prefix.extend_from_slice(body);
        db.put(entry_key(&prefix, height.to_le_bytes()), value).unwrap();
    }

    /// Reads back the migrated (big-endian) value for `body` at `height`.
    fn read_migrated(db: &rocksdb::DB, body: &[u8], height: u32) -> Option<Vec<u8>> {
        let mut prefix = update_ctx();
        prefix.extend_from_slice(body);
        db.get(entry_key(&prefix, height.to_be_bytes())).unwrap()
    }

    /// Counts every entry under a context prefix.
    fn count(db: &rocksdb::DB, ctx: &[u8]) -> usize {
        let mut iterator = db.raw_iterator();
        iterator.seek(ctx);
        let mut total = 0;
        while iterator.valid() {
            match iterator.key() {
                Some(key) if key.starts_with(ctx) => total += 1,
                _ => break,
            }
            iterator.next();
        }
        total
    }

    /// Every entry moves to its big-endian key, whatever the heights map does or does not say.
    #[test]
    fn test_migrates_every_entry() {
        let (db, _dir) = database();
        for height in [1u32, 7, 1_000, 30_000] {
            seed(&db, b"alpha", height, format!("v{height}").as_bytes());
        }
        migrate(&db, NETWORK).unwrap();
        for height in [1u32, 7, 1_000, 30_000] {
            assert_eq!(read_migrated(&db, b"alpha", height), Some(format!("v{height}").into_bytes()));
        }
        assert_eq!(count(&db, &update_ctx()), 4);
    }

    /// The heights map is never consulted, only removed.
    ///
    /// This is the property that makes the repair robust to every layout that shipped: it does not
    /// matter whether the heights map is absent, complete, stale, or actively wrong, because the
    /// entries themselves are the source of truth.
    #[test]
    fn test_heights_map_is_ignored_and_dropped() {
        let (db, _dir) = database();
        for height in [5u32, 50, 500] {
            seed(&db, b"beta", height, format!("v{height}").as_bytes());
        }

        // A heights row that disagrees with reality in every available way: it omits a height that
        // exists (500), and lists two that never did (11 and 99_999).
        let mut heights_key = heights_ctx();
        heights_key.extend_from_slice(b"beta");
        db.put(&heights_key, bincode::serialize(&vec![5u32, 11, 99_999]).unwrap()).unwrap();
        // A row for a key with no entries at all.
        let mut orphan_row = heights_ctx();
        orphan_row.extend_from_slice(b"nonexistent");
        db.put(&orphan_row, bincode::serialize(&vec![1u32]).unwrap()).unwrap();

        migrate(&db, NETWORK).unwrap();

        // Every real entry migrated, including the one the row omitted.
        for height in [5u32, 50, 500] {
            assert_eq!(read_migrated(&db, b"beta", height), Some(format!("v{height}").into_bytes()));
        }
        // Nothing was invented for the heights the row made up.
        assert_eq!(count(&db, &update_ctx()), 3);
        // And the heights map is gone entirely.
        assert_eq!(count(&db, &heights_ctx()), 0);
    }

    /// Entries written by v4.7.5/v4.8.0 have no heights row at all, and must still migrate.
    #[test]
    fn test_migrates_entries_with_no_heights_row() {
        let (db, _dir) = database();
        seed(&db, b"gamma", 100, b"listed");
        seed(&db, b"gamma", 200, b"orphan");
        let mut heights_key = heights_ctx();
        heights_key.extend_from_slice(b"gamma");
        // The row knows only about 100 -- exactly the v4.8.0 hole.
        db.put(&heights_key, bincode::serialize(&vec![100u32]).unwrap()).unwrap();

        migrate(&db, NETWORK).unwrap();

        assert_eq!(read_migrated(&db, b"gamma", 100), Some(b"listed".to_vec()));
        assert_eq!(read_migrated(&db, b"gamma", 200), Some(b"orphan".to_vec()));
    }

    /// Byte-reversal partners survive, including a palindrome that is its own partner.
    #[test]
    fn test_endian_collisions() {
        let (db, _dir) = database();
        // LE(256) == BE(65_536), and 65_792 encodes identically either way.
        assert_eq!(256u32.to_le_bytes(), 65_536u32.to_be_bytes());
        assert_eq!(65_792u32.to_le_bytes(), 65_792u32.to_be_bytes());
        for (height, value) in [(256u32, b"a".as_slice()), (65_536, b"b"), (65_792, b"c"), (1_000, b"d")] {
            seed(&db, b"delta", height, value);
        }
        migrate(&db, NETWORK).unwrap();
        assert_eq!(read_migrated(&db, b"delta", 256), Some(b"a".to_vec()));
        assert_eq!(read_migrated(&db, b"delta", 65_536), Some(b"b".to_vec()));
        assert_eq!(read_migrated(&db, b"delta", 65_792), Some(b"c".to_vec()));
        assert_eq!(read_migrated(&db, b"delta", 1_000), Some(b"d".to_vec()));
    }

    /// Writes a big-endian entry, as snarkOS v4.8.1+ would have.
    fn seed_big(db: &rocksdb::DB, body: &[u8], height: u32, value: &[u8]) {
        let mut prefix = update_ctx();
        prefix.extend_from_slice(body);
        db.put(entry_key(&prefix, height.to_be_bytes()), value).unwrap();
    }

    /// A key holding both encodings migrates the little-endian half and leaves the rest alone.
    ///
    /// This is the shape a real ledger has, not an edge case. A key first written during the
    /// v4.7.5/v4.8.0 window has no heights row, so a v4.8.1+ build took the big-endian branch for
    /// it while its earlier entries stayed little-endian. Refusing the mixture would refuse the
    /// databases this repair exists to fix.
    #[test]
    fn test_mixed_key_migrates_only_the_little_endian_half() {
        let (db, _dir) = database();
        // Written under v4.8.0: little-endian, no heights row.
        seed(&db, b"epsilon", 30_643, b"le-early");
        seed(&db, b"epsilon", 53_441, b"le-late");
        // Written under v4.9.1 once the key had no row to send it down the legacy branch.
        seed_big(&db, b"epsilon", 53_442, b"be-early");
        seed_big(&db, b"epsilon", 76_514, b"be-late");

        migrate(&db, NETWORK).unwrap();

        // Everything is now readable at its big-endian key, at its true height.
        for (height, value) in
            [(30_643u32, b"le-early".as_slice()), (53_441, b"le-late"), (53_442, b"be-early"), (76_514, b"be-late")]
        {
            assert_eq!(read_migrated(&db, b"epsilon", height), Some(value.to_vec()), "height {height}");
        }
        assert_eq!(count(&db, &update_ctx()), 4);
    }

    /// An entry that fits either encoding, on a key that genuinely holds both, is reported rather
    /// than guessed at.
    ///
    /// Harder to arrange than it used to be, which is the point: the derived tip settles almost
    /// everything. It needs an unambiguous entry in each encoding, an ambiguous entry whose *both*
    /// readings fall at or below the tip, and therefore a tip high enough for that to be possible
    /// at all -- below 65,536 no such entry exists.
    #[test]
    fn test_undecidable_entry_is_reported() {
        let (db, _dir) = database();
        // Unambiguously little-endian, and high enough to lift the tip past 65,536.
        seed(&db, b"eta", 70_000, b"le");
        // Unambiguously big-endian: its little-endian reading is ~1.9 billion.
        seed_big(&db, b"eta", 70_001, b"be");
        // Suffix [0, 1, 0, 0]: 256 little-endian, 65,536 big-endian, both at or below the tip.
        seed(&db, b"eta", 256, b"?");

        let error = migrate(&db, NETWORK).unwrap_err().to_string();
        assert!(error.contains("1 historical mapping entries"), "count not reported: {error}");
        assert!(error.contains("256") && error.contains("65536"), "candidate heights not named: {error}");
        // Nothing may have been written: the verdict is reached before any entry moves, so a
        // ledger reported unrepairable is left exactly as it was.
        assert!(error.contains("Nothing has been modified"), "no such assurance given: {error}");
        assert_eq!(read_migrated(&db, b"eta", 70_000), None, "an entry was migrated despite the refusal");
        assert!(
            get_metadata(&db, NETWORK, MetadataKey::StorageMigrationCursor).unwrap().is_none(),
            "a cursor was recorded despite the refusal; a restart could mistake it for progress"
        );
    }

    /// A run interrupted at a batch boundary resumes without re-applying committed work.
    ///
    /// The cursor is written in the same batch as the data it describes, so it always names a batch
    /// boundary. Replaying a committed batch would swap byte-reversal partners back, which is what
    /// this arrangement produces if the skip is wrong: 256 and 65,536 are partners and form their
    /// own batch after the isolated one.
    #[test]
    fn test_resumes_at_a_batch_boundary() {
        let (db, _dir) = database();
        // 70,000 is here to lift the derived tip above 65,536; without it the tip would be 256 and
        // an entry at height 65,536 would rightly be judged implausible.
        let seeded: [(u32, &[u8]); 5] = [(10, b"a"), (20, b"b"), (256, b"c"), (65_536, b"d"), (70_000, b"e")];
        for (height, value) in seeded {
            seed(&db, b"theta", height, value);
        }
        let heights = vec![10u32, 20, 256, 65_536, 70_000];
        assert_eq!(work_order(&heights), vec![vec![10, 20, 70_000], vec![256, 65_536]], "batching assumption");

        // Apply the first batch by hand, exactly as a committed run would have left it.
        let mut prefix = update_ctx();
        prefix.extend_from_slice(b"theta");
        for (height, value) in [(10u32, b"a".as_slice()), (20, b"b"), (70_000, b"e")] {
            db.put(entry_key(&prefix, height.to_be_bytes()), value).unwrap();
            db.delete(entry_key(&prefix, height.to_le_bytes())).unwrap();
        }
        db.put(metadata_key(NETWORK, MetadataKey::StorageMigrationHeights), bincode::serialize(&heights).unwrap())
            .unwrap();
        db.put(
            metadata_key(NETWORK, MetadataKey::StorageMigrationCursor),
            bincode::serialize(&(b"theta".to_vec(), 3u64, 70_000u32)).unwrap(),
        )
        .unwrap();

        migrate(&db, NETWORK).unwrap();

        for (height, value) in seeded {
            assert_eq!(read_migrated(&db, b"theta", height), Some(value.to_vec()), "height {height}");
        }
    }

    /// Many keys, each with entries, all migrate; keys are visited exactly once.
    #[test]
    fn test_many_keys() {
        let (db, _dir) = database();
        for index in 0..200u32 {
            let body = format!("key{index:04}");
            for height in [index + 1, index + 1_000] {
                seed(&db, body.as_bytes(), height, format!("{index}:{height}").as_bytes());
            }
        }
        migrate(&db, NETWORK).unwrap();
        for index in 0..200u32 {
            let body = format!("key{index:04}");
            for height in [index + 1, index + 1_000] {
                assert_eq!(read_migrated(&db, body.as_bytes(), height), Some(format!("{index}:{height}").into_bytes()));
            }
        }
        assert_eq!(count(&db, &update_ctx()), 400);
    }

    /// `drop_map` removes exactly its own map, and does not spill into the next one.
    #[test]
    fn test_drop_map_respects_prefix_boundary() {
        let (db, _dir) = database();
        let mut heights_key = heights_ctx();
        heights_key.extend_from_slice(b"row");
        db.put(&heights_key, b"x").unwrap();

        // An entry in the map immediately after the heights map in prefix order.
        let mut neighbour = heights_ctx();
        neighbour[PREFIX_LEN - 2] = neighbour[PREFIX_LEN - 2].wrapping_add(1);
        neighbour.extend_from_slice(b"keep");
        db.put(&neighbour, b"y").unwrap();

        drop_map(&db, &heights_ctx()).unwrap();

        assert_eq!(count(&db, &heights_ctx()), 0);
        assert_eq!(db.get(&neighbour).unwrap(), Some(b"y".to_vec()));
    }

    #[test]
    fn test_byte_reverse_is_an_involution() {
        for height in [0u32, 1, 255, 256, 65_536, 65_792, 1_000_000, 21_639_560, u32::MAX] {
            assert_eq!(byte_reverse(byte_reverse(height)), height);
            assert_eq!(height.to_le_bytes(), byte_reverse(height).to_be_bytes());
        }
    }

    #[test]
    fn test_chunk_groups_never_splits_a_partner_pair() {
        let groups = vec![vec![1u32, 2], vec![3], vec![4, 5], vec![6, 7]];
        for limit in 1..=8 {
            for chunk in chunk_groups(&groups, limit) {
                for group in &groups {
                    let present = group.iter().filter(|h| chunk.contains(h)).count();
                    assert!(present == 0 || present == group.len(), "group {group:?} split at limit {limit}");
                }
            }
        }
    }

    /// A completed migration is not started over, even if the version was never recorded.
    ///
    /// The runner advances the schema version only after this returns, so a crash in between would
    /// otherwise re-run it against big-endian data. That is not a no-op: a big-endian entry at a low
    /// height reads back as a plausible little-endian height and would be moved a second time.
    #[test]
    fn test_completed_migration_is_not_repeated() {
        let (db, _dir) = database();
        for height in [3u32, 30, 300] {
            seed(&db, b"zeta", height, format!("v{height}").as_bytes());
        }
        migrate(&db, NETWORK).unwrap();

        // Exactly the crash window: the migration finished, the version was never written.
        migrate(&db, NETWORK).unwrap();

        for height in [3u32, 30, 300] {
            assert_eq!(read_migrated(&db, b"zeta", height), Some(format!("v{height}").into_bytes()));
        }
        assert_eq!(count(&db, &update_ctx()), 3);
    }

    /// The derived tip is a bound no real entry exceeds, so a correct ledger is never condemned.
    ///
    /// This is the property the whole classification rests on. An earlier version derived the tip
    /// only from entries whose encoding was already certain, which on a key whose highest entries
    /// were ambiguous produced a tip below them -- and then rejected those entries as implausible
    /// under either reading, condemning a repairable ledger.
    #[test]
    fn test_derived_tip_keeps_every_entry_plausible() {
        let (db, _dir) = database();
        // Heights whose encodings collide, so every one of them is ambiguous under any bound: the
        // derivation cannot lean on unambiguous entries here.
        for height in [256u32, 65_536, 65_792] {
            seed(&db, b"iota", height, b"v");
        }
        let tip = observed_tip(&db, &update_ctx()).unwrap();
        assert_eq!(tip, 65_792, "the tip is the largest of each entry's smaller reading");

        // And with that tip, no entry is implausible both ways -- which is what lets the migration
        // proceed rather than reporting corruption.
        let entries = classify_entries(&db, &update_ctx(), b"iota", tip).unwrap();
        assert_eq!(entries.little.len() + entries.big as usize + entries.undecidable.len(), 3);

        // A lone high entry lifts the tip for everything else.
        let (db2, _dir2) = database();
        seed(&db2, b"kappa", 5, b"v");
        seed(&db2, b"kappa", 900_000, b"v");
        assert_eq!(observed_tip(&db2, &update_ctx()).unwrap(), 900_000);
    }

    /// A palindromic suffix is not ambiguous: it reads as the same height either way.
    ///
    /// `[0, 1, 1, 0]` is height 65,792 under both encodings, and the big-endian key for 65,792 is
    /// that same suffix -- so the entry is already where it belongs whichever build wrote it.
    /// Reporting these as undecidable would condemn a ledger over entries that need no decision:
    /// on a real three-format ledger they were half of what remained.
    #[test]
    fn test_palindromic_heights_need_no_decision() {
        assert_eq!(65_792u32.to_le_bytes(), 65_792u32.to_be_bytes());

        let (db, _dir) = database();
        // A key holding both encodings, so ambiguity is otherwise possible.
        seed(&db, b"lambda", 70_000, b"le");
        seed_big(&db, b"lambda", 70_001, b"be");
        seed(&db, b"lambda", 65_792, b"palindrome");

        migrate(&db, NETWORK).unwrap();

        assert_eq!(read_migrated(&db, b"lambda", 65_792), Some(b"palindrome".to_vec()));
        assert_eq!(read_migrated(&db, b"lambda", 70_000), Some(b"le".to_vec()));
        assert_eq!(read_migrated(&db, b"lambda", 70_001), Some(b"be".to_vec()));
    }

    /// Returns the process's peak resident set size, in KiB.
    fn peak_rss_kb() -> u64 {
        std::fs::read_to_string("/proc/self/status")
            .expect("read /proc/self/status")
            .lines()
            .find_map(|line| line.strip_prefix("VmHWM:"))
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|kb| kb.parse().ok())
            .expect("parse VmHWM")
    }

    /// Resets the kernel's peak-RSS high-water mark, so the figure below is the migration's own.
    fn reset_peak_rss() {
        std::fs::write("/proc/self/clear_refs", "5").expect("reset peak RSS");
    }

    /// Measures the repair against the production data shape: few keys, very long histories.
    ///
    /// This is what the `credits.aleo` staking mappings look like, since `replace_mapping` records
    /// an entry for every one of their keys on every block.
    ///
    /// **Vary `BENCH_HEIGHTS`, not `BENCH_KEYS`.** Peak memory is O(entries in the largest single
    /// key), not O(total entries): the classified height list, its serialized copy, the partner
    /// lookup set and the work order all hold one element per entry *of the key being migrated*.
    /// Sweeping key count at fixed depth -- as an earlier round of measurements did -- holds the
    /// only dimension that drives memory constant, and reports a flatness that is an artifact of
    /// the sweep rather than a property of the code.
    ///
    ///   BENCH_KEYS / BENCH_HEIGHTS override the defaults (447 keys, the live `bonded` size).
    #[test]
    #[ignore = "benchmark; run explicitly with --ignored"]
    fn bench_repair_production_shape() {
        let keys: usize = std::env::var("BENCH_KEYS").ok().and_then(|v| v.parse().ok()).unwrap_or(447);
        let heights: u32 = std::env::var("BENCH_HEIGHTS").ok().and_then(|v| v.parse().ok()).unwrap_or(2_000);

        let (db, _dir) = database();
        // A stand-in for a `bond_state` value: a validator address plus microcredits.
        let value = vec![0x5Au8; 60];

        let seeding = std::time::Instant::now();
        for index in 0..keys {
            let body = format!("staker{index:06}");
            for height in 0..heights {
                seed(&db, body.as_bytes(), height, &value);
            }
        }
        let entries = keys * heights as usize;
        println!("  seeded {keys} keys x {heights} heights = {entries} entries in {:?}", seeding.elapsed());

        reset_peak_rss();
        let baseline = peak_rss_kb();

        let timer = std::time::Instant::now();
        migrate(&db, NETWORK).unwrap();
        let elapsed = timer.elapsed();
        let growth = peak_rss_kb().saturating_sub(baseline);

        println!(
            "BENCH repair keys={keys} heights={heights} entries={entries} elapsed={elapsed:?} \
             rate={:.0}/s growth={:.3}GiB bytes_per_entry={:.1}",
            entries as f64 / elapsed.as_secs_f64(),
            growth as f64 / 1_048_576.0,
            (growth as f64 * 1024.0) / entries as f64,
        );

        assert_eq!(count(&db, &update_ctx()), entries);
    }
}
