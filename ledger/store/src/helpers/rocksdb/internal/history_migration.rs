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

use anyhow::{Result, bail};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

/// The number of historical entries rewritten per write batch.
///
/// A single key's history can run to millions of entries, so it is never held whole.
const CHUNK_SIZE: usize = 50_000;

/// A height at or above which a value cannot plausibly be a block height.
///
/// Used only to recognise an entry that cannot be little-endian. Mainnet passed 21.6M in 2026 and
/// gains roughly 28k blocks a day, so this leaves over a decade of headroom while still catching
/// the byte-reversal of essentially any real height.
const IMPLAUSIBLE_HEIGHT: u32 = 1 << 27;

/// How often to report progress. This blocks startup and can run for hours on an archive node,
/// where silence is indistinguishable from a hang.
const REPORT_INTERVAL: Duration = Duration::from_secs(30);

/// Running totals, reported on a timer.
///
/// Carried through the per-key work rather than checked only between keys: on the shape this exists
/// for -- a few hundred keys with millions of entries each -- one key is hundreds of batches, and
/// reporting per key would go silent for exactly as long as the interval is meant to prevent.
struct Progress {
    started: Instant,
    last_report: Instant,
    keys: u64,
    entries: u64,
    already_big: u64,
}

impl Progress {
    fn new() -> Self {
        let now = Instant::now();
        Self { started: now, last_report: now, keys: 0, entries: 0, already_big: 0 }
    }

    /// Logs a line if the interval has elapsed since the last one.
    fn report(&mut self) {
        if self.last_report.elapsed() < REPORT_INTERVAL {
            return;
        }
        let elapsed = self.started.elapsed().as_secs_f64();
        tracing::info!(
            "History repair: {} keys, {} entries ({:.0} entries/s)",
            self.keys,
            self.entries,
            self.entries as f64 / elapsed
        );
        self.last_report = Instant::now();
    }
}

/// The cursor index recorded once every key has been migrated.
///
/// The version is advanced by the migration runner *after* this returns, so a crash in between
/// would otherwise re-run a completed migration over data that is already big-endian -- which is
/// not merely wasteful but wrong, since a big-endian entry at a low height reads back as a
/// plausible little-endian one and would be moved a second time.
const CURSOR_COMPLETE: u64 = u64::MAX;

/// Returns the height whose big-endian encoding is byte-identical to `height`'s little-endian one.
///
/// An involution, so it maps a source key to the destination that would overwrite it, and back.
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

/// Rewrites every historical mapping update to a big-endian height key.
pub(crate) fn migrate(database: &rocksdb::DB, network_id: u16) -> Result<()> {
    let update_context = context(network_id, MapID::Program(ProgramMap::MappingUpdate));
    let heights_context = context(network_id, MapID::Program(ProgramMap::MappingUpdateHeights));

    // Resume state, if a previous run was interrupted.
    let (mut cursor, mut resume_index) = match get_metadata(database, network_id, MetadataKey::StorageMigrationCursor)?
    {
        Some(bytes) => {
            let (body, index): (Vec<u8>, u64) = bincode::deserialize(&bytes)?;
            (Some(body), index)
        }
        None => (None, 0),
    };

    // A previous run already finished; the runner simply had not recorded the version yet.
    if resume_index == CURSOR_COMPLETE {
        return Ok(());
    }

    let mut progress = Progress::new();
    let mut announced = false;

    loop {
        // The key being resumed is processed again from `resume_index`; otherwise advance.
        let body = match cursor.take() {
            Some(body) if resume_index > 0 => body,
            Some(body) => match next_body(database, &update_context, Some(&body))? {
                Some(next) => {
                    resume_index = 0;
                    next
                }
                None => break,
            },
            None => match next_body(database, &update_context, None)? {
                Some(next) => next,
                None => break,
            },
        };

        if !announced {
            tracing::info!(
                "Repairing the historical mapping schema. The node will not serve requests until \
                 this completes."
            );
            announced = true;
        }

        // The little-endian heights of this key, recorded before anything moves: once a key is
        // partially migrated its entries no longer say what they were, so a resumed run must read
        // its work back rather than re-derive it.
        //
        // The heights and the cursor that points at them are written together. Written separately,
        // an interruption in between leaves a cursor naming one key beside another key's heights,
        // and the resumed run migrates the wrong list under the wrong prefix.
        let heights = match resume_index > 0 {
            true => read_recorded_heights(database, network_id)?,
            false => {
                let classified = classify_entries(database, &update_context, &body)?;
                progress.already_big += classified.big;
                let mut batch = rocksdb::WriteBatch::default();
                batch.put(
                    metadata_key(network_id, MetadataKey::StorageMigrationHeights),
                    bincode::serialize(&classified.little)?,
                );
                batch.put(
                    metadata_key(network_id, MetadataKey::StorageMigrationCursor),
                    bincode::serialize(&(&body, 0u64))?,
                );
                database.write(batch)?;
                classified.little
            }
        };

        let mut prefix = Vec::with_capacity(update_context.len() + body.len());
        prefix.extend_from_slice(&update_context);
        prefix.extend_from_slice(&body);

        migrate_key(database, network_id, &prefix, &body, &heights, resume_index, &mut progress)?;
        resume_index = 0;
        progress.keys += 1;
        progress.report();
        cursor = Some(body);
    }

    // The heights map is not consulted by anything after this point, and keeping it would leave the
    // read-modify-write path that made it grow in the first place.
    drop_map(database, &heights_context)?;
    // Completion and the disposal of the working state commit together. Deleting the heights first
    // would, if interrupted, leave a cursor pointing at a key whose heights are gone -- which the
    // resumed run can only report as an unrecoverable error, on a migration that had in fact
    // finished. The sentinel exists because the runner advances the schema version only after this
    // returns, and a crash in between would otherwise restart a completed migration over
    // big-endian data.
    let mut batch = rocksdb::WriteBatch::default();
    batch.put(
        metadata_key(network_id, MetadataKey::StorageMigrationCursor),
        bincode::serialize(&(Vec::<u8>::new(), CURSOR_COMPLETE))?,
    );
    batch.delete(metadata_key(network_id, MetadataKey::StorageMigrationHeights));
    database.write(batch)?;

    if announced {
        tracing::info!(
            "History repair complete: {} keys, {} entries migrated, {} already big-endian, in {:.1?}",
            progress.keys,
            progress.entries,
            progress.already_big,
            progress.started.elapsed()
        );
    }

    Ok(())
}

/// Returns the next mapping key body strictly after `after`, or the first one when `after` is
/// `None`. Returns `None` once the map is exhausted.
///
/// Bodies are prefix-free — a `Plaintext` is length-delimited, so one complete encoding cannot be a
/// strict prefix of another — so all of a key's entries are contiguous and this never revisits one.
fn next_body(database: &rocksdb::DB, update_context: &[u8], after: Option<&[u8]>) -> Result<Option<Vec<u8>>> {
    let mut iterator = database.raw_iterator();
    match after {
        // Seek past the last possible entry of the previous body.
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
    // The database is one keyspace shared by every map, partitioned only by the context prefix.
    if !key.starts_with(update_context) || key.len() < update_context.len() + 4 {
        return Ok(None);
    }
    Ok(Some(key[update_context.len()..key.len() - 4].to_vec()))
}

/// How an entry's four height bytes should be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    /// Written by snarkOS <= v4.8.0; needs migrating.
    Little,
    /// Written by snarkOS v4.8.1+; already in the target form.
    Big,
}

/// The entries of one mapping key, with the encoding each was written in.
struct KeyEntries {
    /// Heights still in little-endian form, ascending. These are the work.
    little: Vec<u32>,
    /// How many entries were already big-endian, for reporting.
    big: u64,
}

/// Reads every entry of one mapping key and decides how each was written.
///
/// Both encodings can occur under a single key, and this is the *expected* shape rather than an
/// edge case: a key first written during the v4.7.5/v4.8.0 window has no heights row, so when a
/// v4.8.1+ build took over it took the big-endian branch for that key while its earlier entries
/// stayed little-endian.
///
/// The two are told apart by which reading is a plausible height. Where only one is, the answer is
/// certain. Where both are -- roughly one entry in 128, those whose height ends in 0x00 or 0x01 --
/// the tie is broken by *when* each encoding was in use: within a key every little-endian entry
/// predates every big-endian one, so the two occupy disjoint height ranges. `byte_reverse` scatters
/// values across the whole `u32` space, so a genuinely ambiguous entry would have to have both of
/// its readings land inside those narrow ranges, which is rare enough to report rather than guess.
fn classify_entries(database: &rocksdb::DB, update_context: &[u8], body: &[u8]) -> Result<KeyEntries> {
    let mut prefix = Vec::with_capacity(update_context.len() + body.len());
    prefix.extend_from_slice(update_context);
    prefix.extend_from_slice(body);

    // Collect the raw suffixes first; the encoding of the ambiguous ones depends on the rest.
    let mut suffixes = Vec::new();
    let mut iterator = database.raw_iterator();
    iterator.seek(&prefix);
    while iterator.valid() {
        let Some(key) = iterator.key() else { break };
        if !key.starts_with(&prefix) || key.len() != prefix.len() + 4 {
            break;
        }
        suffixes.push(<[u8; 4]>::try_from(&key[prefix.len()..]).expect("checked length"));
        iterator.next();
    }
    iterator.status()?;

    // First pass: everything whose encoding is not in doubt, and the range each encoding covers.
    let mut little = Vec::new();
    let mut big = 0u64;
    let mut ambiguous = Vec::new();
    let (mut little_max, mut big_min) = (None::<u32>, None::<u32>);
    for suffix in &suffixes {
        let as_little = u32::from_le_bytes(*suffix);
        let as_big = u32::from_be_bytes(*suffix);
        match (as_little < IMPLAUSIBLE_HEIGHT, as_big < IMPLAUSIBLE_HEIGHT) {
            (true, false) => {
                little_max = Some(little_max.map_or(as_little, |m: u32| m.max(as_little)));
                little.push(as_little);
            }
            (false, true) => {
                big_min = Some(big_min.map_or(as_big, |m: u32| m.min(as_big)));
                big += 1;
            }
            (true, true) => ambiguous.push((*suffix, as_little, as_big)),
            (false, false) => bail!(
                "Historical mapping entry with height bytes {suffix:?} is not a plausible height \
                 under either encoding; the ledger is corrupt and must be resynced from genesis"
            ),
        }
    }

    // Second pass: place the ambiguous ones.
    //
    // A key that shows no evidence of one encoding was never written in it, which settles most
    // ambiguity outright: a key with no big-endian entry was never touched by a v4.8.1+ build, so
    // everything under it is little-endian however its bytes happen to read. Only a key that
    // genuinely holds both needs the boundary, and there the two encodings occupy disjoint height
    // ranges, because every little-endian entry predates every big-endian one.
    for (suffix, as_little, as_big) in ambiguous {
        let decided = match (little_max, big_min) {
            // No big-endian evidence: the key predates v4.8.1 entirely.
            (_, None) => Encoding::Little,
            // No little-endian evidence: the key was only ever written big-endian.
            (None, Some(_)) => Encoding::Big,
            // Both encodings present, so place it by which side of the boundary it falls on.
            (Some(max), Some(min)) => {
                let fits_little = as_little <= max;
                let fits_big = as_big >= min;
                match (fits_little, fits_big) {
                    (true, false) => Encoding::Little,
                    (false, true) => Encoding::Big,
                    _ => bail!(
                        "Cannot determine the encoding of a historical mapping entry with height \
                         bytes {suffix:?}: it reads as height {as_little} little-endian and \
                         {as_big} big-endian, and this key holds entries in both encodings either \
                         side of it. The ledger must be resynced from genesis."
                    ),
                }
            }
        };
        match decided {
            Encoding::Little => little.push(as_little),
            Encoding::Big => big += 1,
        }
    }

    little.sort_unstable();
    Ok(KeyEntries { little, big })
}

/// Reads back the heights recorded for an interrupted key.
fn read_recorded_heights(database: &rocksdb::DB, network_id: u16) -> Result<Vec<u32>> {
    match get_metadata(database, network_id, MetadataKey::StorageMigrationHeights)? {
        Some(bytes) => Ok(bincode::deserialize(&bytes)?),
        None => bail!("A history repair was interrupted, but the heights it was working on are missing"),
    }
}

/// Groups colliding heights with their partners, so a batch never splits a pair.
///
/// `byte_reverse` is an involution, so the colliding set decomposes into disjoint pairs
/// `{h, byte_reverse(h)}` and palindromic singletons where `h == byte_reverse(h)`.
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
/// the batches already committed by counting entries. Byte-reversal partners share a batch, because
/// their sources and destinations are the same raw keys.
fn work_order(heights: &[u32]) -> Vec<Vec<u32>> {
    let lookup = heights.iter().copied().collect::<HashSet<_>>();
    let (colliding, isolated): (Vec<u32>, Vec<u32>) =
        heights.iter().partition(|height| lookup.contains(&byte_reverse(**height)));
    let mut batches: Vec<Vec<u32>> = isolated.chunks(CHUNK_SIZE).map(<[u32]>::to_vec).collect();
    batches.extend(chunk_groups(&partner_groups(&colliding), CHUNK_SIZE));
    batches
}

/// Migrates one mapping key, skipping the batches a previous run already committed.
fn migrate_key(
    database: &rocksdb::DB,
    network_id: u16,
    prefix: &[u8],
    body: &[u8],
    heights: &[u32],
    start: u64,
    progress: &mut Progress,
) -> Result<()> {
    let mut processed = 0u64;
    for batch in work_order(heights) {
        let next = processed + batch.len() as u64;
        // Batches are atomic, so a committed one is skipped whole.
        if next <= start {
            processed = next;
            continue;
        }
        progress.entries += write_chunk(database, network_id, prefix, &batch, body, next)?;
        processed = next;
        progress.report();
    }
    Ok(())
}

/// Moves a batch of heights to big-endian keys, recording the progress in the same write.
///
/// This is the invariant the whole resume design rests on: the cursor and the data it describes
/// commit together or not at all. Written separately, an interruption between them replays a batch
/// that already committed -- which for byte-reversal partners silently swaps their values back,
/// since each one's source is the other's destination.
///
/// Within the batch every source is read before any write is queued, and deletions precede
/// insertions, so where a source and a destination are the same raw key the migrated value wins.
fn write_chunk(
    database: &rocksdb::DB,
    network_id: u16,
    prefix: &[u8],
    heights: &[u32],
    body: &[u8],
    processed: u64,
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

    let mut batch = rocksdb::WriteBatch::default();
    for source in &sources {
        batch.delete(source);
    }
    let count = moved.len() as u64;
    for (height, value) in moved {
        batch.put(entry_key(prefix, height.to_be_bytes()), value);
    }
    batch.put(metadata_key(network_id, MetadataKey::StorageMigrationCursor), bincode::serialize(&(body, processed))?);
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
    /// This needs all three of: an unambiguous little-endian entry, an unambiguous big-endian one,
    /// and an ambiguous entry whose two readings straddle the boundary between them. Absent any of
    /// those the encoding is decidable, which is why the case is rare in practice.
    #[test]
    fn test_undecidable_entry_is_reported() {
        let (db, _dir) = database();
        // Unambiguously little-endian: its big-endian reading is ~2.18 billion.
        seed(&db, b"eta", 53_441, b"le");
        // Unambiguously big-endian: its little-endian reading is ~3.27 billion.
        seed_big(&db, b"eta", 53_442, b"be");
        // Suffix [0, 1, 0, 0]: reads as 256 little-endian (at or below the little-endian range) and
        // 65_536 big-endian (at or above the big-endian range). Both are consistent.
        seed(&db, b"eta", 256, b"?");

        let error = migrate(&db, NETWORK).unwrap_err().to_string();
        assert!(error.contains("Cannot determine the encoding"), "unexpected error: {error}");
    }

    /// A run interrupted at a batch boundary resumes without re-applying committed work.
    ///
    /// The cursor is written in the same batch as the data it describes, so it always names a batch
    /// boundary. Replaying a committed batch would swap byte-reversal partners back, which is
    /// exactly what this arrangement produces if the skip is wrong: heights 256 and 65_536 are
    /// partners, so they form their own batch after the isolated one.
    #[test]
    fn test_resumes_at_a_batch_boundary() {
        let (db, _dir) = database();
        for (height, value) in [(10u32, b"a".as_slice()), (20, b"b"), (256, b"c"), (65_536, b"d")] {
            seed(&db, b"theta", height, value);
        }
        let heights = vec![10u32, 20, 256, 65_536];
        assert_eq!(work_order(&heights), vec![vec![10, 20], vec![256, 65_536]], "batching assumption");

        // Apply the first batch by hand, exactly as a committed run would have left it.
        let mut prefix = update_ctx();
        prefix.extend_from_slice(b"theta");
        for (height, value) in [(10u32, b"a".as_slice()), (20, b"b")] {
            db.put(entry_key(&prefix, height.to_be_bytes()), value).unwrap();
            db.delete(entry_key(&prefix, height.to_le_bytes())).unwrap();
        }
        db.put(metadata_key(NETWORK, MetadataKey::StorageMigrationHeights), bincode::serialize(&heights).unwrap())
            .unwrap();
        db.put(
            metadata_key(NETWORK, MetadataKey::StorageMigrationCursor),
            bincode::serialize(&(b"theta".to_vec(), 2u64)).unwrap(),
        )
        .unwrap();

        migrate(&db, NETWORK).unwrap();

        for (height, value) in [(10u32, b"a".as_slice()), (20, b"b"), (256, b"c"), (65_536, b"d")] {
            assert_eq!(read_migrated(&db, b"theta", height), Some(value.to_vec()), "height {height}");
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
