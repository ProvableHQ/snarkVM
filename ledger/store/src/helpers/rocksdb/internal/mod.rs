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

mod history_rebuild;
pub use history_rebuild::{
    allow_downlevel_open,
    clear_rebuilt_state,
    disallow_downlevel_open,
    has_history,
    has_staking_rewards,
    is_rebuilding,
    open_for_rebuild,
    required_features,
    schema_version,
    set_rebuilding,
    set_schema_version,
};

mod id;
pub use id::*;

mod map;
pub use map::*;

mod nested_map;
pub use nested_map::*;

#[cfg(test)]
mod tests;

use aleo_std_storage::StorageMode;
use anyhow::{Result, anyhow, bail, ensure};
#[cfg(feature = "locktick")]
use locktick::parking_lot::Mutex;
#[cfg(not(feature = "locktick"))]
use parking_lot::Mutex;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    borrow::Borrow,
    collections::HashMap,
    marker::PhantomData,
    mem,
    ops::Deref,
    path::PathBuf,
    sync::{
        Arc,
        LazyLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

pub const PREFIX_LEN: usize = 4; // N::ID (u16) + DataID (u16)

/// The storage schema version this build writes and understands.
///
/// Bump this whenever the on-disk layout changes in a way that a build expecting the previous
/// version would read incorrectly. A database records the version it was last written under, so a
/// build can refuse a database from the future instead of silently misreading it.
///
/// Note this only protects forward from the release that introduced it: builds older than that do
/// not consult the record at all. Three incompatible historical-mapping layouts shipped within
/// three weeks with nothing on disk to distinguish them, which is the situation this exists to
/// prevent recurring.
pub const STORAGE_VERSION: u32 = 1;

/// The well-known keys of the storage metadata map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum MetadataKey {
    /// The storage schema version the database was last written under.
    StorageVersion = 0,
    /// Whether the finalize state has been discarded and is awaiting replay.
    ///
    /// Distinct from the version, which only advances once a rebuild completes. This says the
    /// ledger is mid-rebuild, and an empty finalize store cannot say that for itself: a fresh
    /// ledger looks identical, and only one of the two must refuse to serve reads.
    RebuildInProgress = 1,
    /// Which optional maps the discarded data occupied, so a resumed rebuild can still tell.
    RebuildRequiredFeatures = 2,
}

/// Returns the 4-byte `[network_id, map_id]` prefix a map's keys sit behind.
///
/// The layout is a persisted on-disk invariant, so it is built in one place rather than repeated
/// wherever a raw key is needed.
pub(crate) fn map_context(network_id: u16, map_id: MapID) -> Vec<u8> {
    let mut raw = Vec::with_capacity(PREFIX_LEN);
    raw.extend_from_slice(&network_id.to_le_bytes());
    raw.extend_from_slice(&u16::from(map_id).to_le_bytes());
    raw
}

/// Returns the raw database key for a metadata entry.
pub(crate) fn metadata_key(network_id: u16, key: MetadataKey) -> Vec<u8> {
    let mut raw = map_context(network_id, MapID::Metadata(MetadataMap::Metadata));
    raw.push(key as u8);
    raw
}

/// Reads a metadata entry, or `None` if the database has never recorded it.
pub(crate) fn get_metadata(database: &rocksdb::DB, network_id: u16, key: MetadataKey) -> Result<Option<Vec<u8>>> {
    Ok(database.get(metadata_key(network_id, key))?)
}

/// Writes a metadata entry.
pub(crate) fn put_metadata(database: &rocksdb::DB, network_id: u16, key: MetadataKey, value: &[u8]) -> Result<()> {
    Ok(database.put(metadata_key(network_id, key), value)?)
}

/// Reads a `u32` metadata entry, treating an absent record as zero.
///
/// Zero is the right default: every database written before this record existed is, by definition,
/// at the version that preceded it.
pub(crate) fn get_metadata_u32(database: &rocksdb::DB, network_id: u16, key: MetadataKey) -> Result<u32> {
    match get_metadata(database, network_id, key)? {
        Some(bytes) => {
            let bytes: [u8; 4] = bytes.as_slice().try_into().map_err(|_| anyhow!("Malformed metadata for {key:?}"))?;
            Ok(u32::from_le_bytes(bytes))
        }
        None => Ok(0),
    }
}

/// Returns whether the migration from `version` to `version + 1` has anything to do here.
///
/// Lets the version be stamped in passing when every outstanding migration is a no-op, without that
/// shortcut being tied to what any one of them happens to be about.
fn has_work(database: &rocksdb::DB, network_id: u16, version: u32) -> Result<bool> {
    match version {
        0 => history_rebuild::has_history(database, network_id),
        other => bail!("No storage migration is defined for schema v{other}"),
    }
}

/// The remedy printed when a ledger needs rebuilding.
const REBUILD_REMEDY: &str = "Stop the node and rebuild its finalize state with snarkVM's \
                              `rebuild_db` tool, built from this release:\n\n    cargo build \
                              --release --bin rebuild_db --features rebuild,history\n    \
                              rebuild_db <ledger-dir>\n\nIt replays every block from local \
                              storage and can take hours on an archive node. It reports progress, \
                              and can be interrupted and resumed. Pass --check first to see what it \
                              would do.";

/// Verifies the ledger's storage schema is one this build understands.
///
/// Deliberately does **not** rebuild. A rebuild replays the whole chain and can take hours, which
/// is not something a node should do as a side effect of starting: an operator wants to run it when
/// they choose, watching it, able to stop it. So this is a point lookup that refuses to proceed and
/// says what to run, in the manner of every other system that separates schema migration from
/// application startup.
///
/// A database with no history to rebuild is stamped in passing, so a fresh node -- or one that never
/// enabled the `history` feature -- never sees a rebuild prompt for work that does not exist.
fn check_storage_version(database: &rocksdb::DB, network_id: u16) -> Result<()> {
    // The rebuild must open the very database this gate exists to keep a node out of.
    if history_rebuild::downlevel_open_allowed() {
        return Ok(());
    }

    // Checked before the version, which is still at the old one throughout a rebuild and so cannot
    // distinguish "not started" from "half done". It matters because the finalize state has already
    // been discarded by this point: were this ordered after the stamp-in-passing shortcut below,
    // that shortcut would find no history, conclude there was nothing to do, and start a node on an
    // empty finalize store.
    ensure!(
        !history_rebuild::is_rebuilding(database, network_id)?,
        "This ledger's finalize state was discarded by a rebuild that has not finished. It cannot \
         be read until the rebuild completes.\n\n{REBUILD_REMEDY}"
    );

    let found = get_metadata_u32(database, network_id, MetadataKey::StorageVersion)?;

    // A database from the future cannot be read safely, and the failure would otherwise be silent
    // and data-dependent rather than immediate.
    ensure!(
        found <= STORAGE_VERSION,
        "This ledger was written by a newer version of snarkVM (storage schema v{found}; this \
         build understands v{STORAGE_VERSION}). Upgrade snarkVM, or resync from genesis."
    );
    if found == STORAGE_VERSION {
        return Ok(());
    }

    // Nothing recorded, and nothing to record: stamp it and carry on.
    //
    // The gate is on the *data*, not on which features this build was compiled with, and that is a
    // correctness requirement rather than a convenience. A build that does not read history could
    // stamp the version without rebuilding, since it would never notice the difference -- but the
    // stamp is what a later history-enabled build consults, and it would then skip the rebuild and
    // read little-endian entries as big-endian. Blocking a non-history node that carries unrebuilt
    // history is the price of the version meaning what it says.
    //
    // Asked per outstanding migration rather than as one hardcoded probe: a future v1 -> v2
    // migration concerning some other map would otherwise be skipped on any ledger without
    // history, stamping a database as being in a layout it is not in.
    if !(found..STORAGE_VERSION)
        .map(|v| has_work(database, network_id, v))
        .collect::<Result<Vec<_>>>()?
        .iter()
        .any(|has| *has)
    {
        put_metadata(database, network_id, MetadataKey::StorageVersion, &STORAGE_VERSION.to_le_bytes())?;
        return Ok(());
    }

    bail!("This ledger is at storage schema v{found}, and this build requires v{STORAGE_VERSION}.\n\n{REBUILD_REMEDY}");
}

// A static map of database paths to their objects; it's needed in order to facilitate concurrent
// tests involving persistent storage, but it only ever has a single member outside of them.
// TODO: remove the static in favor of improved `open` methods.
// note: this object can't utilize locktick for lock accounting, but it is only ever accessed
//       when first creating the database(s), so this is acceptable; this will also no longer
//       be an issue once the above TODO is complete.
static DATABASES: LazyLock<parking_lot::Mutex<HashMap<PathBuf, RocksDB>>> =
    LazyLock::new(|| parking_lot::Mutex::new(HashMap::new()));

pub trait Database {
    /// Opens the database.
    fn open<S: Into<StorageMode>>(network_id: u16, storage: S) -> Result<Self>
    where
        Self: Sized;

    /// Opens the map with the given `network_id`, `storage mode`, and `map_id` from storage.
    fn open_map<S: Into<StorageMode>, K: Serialize + DeserializeOwned, V: Serialize + DeserializeOwned, T: Into<u16>>(
        network_id: u16,
        storage: S,
        map_id: T,
    ) -> Result<DataMap<K, V>>;

    /// Opens the nested map with the given `network_id`, `storage mode`, and `map_id` from storage.
    fn open_nested_map<
        S: Into<StorageMode>,
        M: Serialize + DeserializeOwned,
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
        T: Into<u16>,
    >(
        network_id: u16,
        storage: S,
        map_id: T,
    ) -> Result<NestedDataMap<M, K, V>>;
}

/// An instance of a RocksDB database.
pub struct RocksDB {
    /// The RocksDB instance.
    rocksdb: Arc<rocksdb::DB>,
    /// The network ID.
    network_id: u16,
    /// The storage mode.
    storage_mode: StorageMode,
    /// The low-level database transaction that gets executed atomically at the end
    /// of a real-run `atomic_finalize` or the outermost `atomic_batch_scope`.
    pub(super) atomic_batch: Arc<Mutex<rocksdb::WriteBatch>>,
    /// The depth of the current atomic write batch; it gets incremented with every call
    /// to `start_atomic` and decremented with each call to `finish_atomic`.
    pub(super) atomic_depth: Arc<AtomicUsize>,
    /// A flag indicating whether the atomic writes are currently paused.
    pub(super) atomic_writes_paused: Arc<AtomicBool>,
    /// This is an optimization that avoids some allocations when querying the database.
    pub(super) default_readopts: rocksdb::ReadOptions,
}

impl Clone for RocksDB {
    fn clone(&self) -> Self {
        Self {
            rocksdb: self.rocksdb.clone(),
            network_id: self.network_id,
            storage_mode: self.storage_mode.clone(),
            atomic_batch: self.atomic_batch.clone(),
            atomic_depth: self.atomic_depth.clone(),
            atomic_writes_paused: self.atomic_writes_paused.clone(),
            default_readopts: Default::default(),
        }
    }
}

impl Deref for RocksDB {
    type Target = Arc<rocksdb::DB>;

    fn deref(&self) -> &Self::Target {
        &self.rocksdb
    }
}

impl Database for RocksDB {
    /// Opens the database.
    ///
    /// In production mode, the database opens directory `~/.aleo/storage/ledger-{network}`.
    /// In development mode, the database opens directory `/path/to/repo/.ledger-{network}-{id}`.
    /// In tests, the database opens an ephemeral directory in the OS temporary folder.
    /// The default storage location can be changed by using `StorageMode::Custom`.
    fn open<S: Into<StorageMode>>(network_id: u16, storage: S) -> Result<Self> {
        let storage = storage.into();

        // Retrieve the database.
        let db_path = aleo_std_storage::aleo_ledger_dir(network_id, &storage);
        let mut databases = DATABASES.lock();
        let database = if let Some(db) = databases.get(&db_path) {
            db.clone()
        } else {
            // Customize database options.
            let mut options = rocksdb::Options::default();
            options.set_compression_type(rocksdb::DBCompressionType::Lz4);

            // Register the prefix length.
            let prefix_extractor = rocksdb::SliceTransform::create_fixed_prefix(PREFIX_LEN);
            options.set_prefix_extractor(prefix_extractor);

            let rocksdb = {
                options.increase_parallelism(2);
                options.set_max_background_jobs(4);
                options.create_if_missing(true);
                options.set_max_open_files(8192);

                Arc::new(rocksdb::DB::open(&options, &db_path)?)
            };

            let db = RocksDB {
                rocksdb,
                network_id,
                storage_mode: storage.clone(),
                atomic_batch: Default::default(),
                atomic_depth: Default::default(),
                atomic_writes_paused: Default::default(),
                default_readopts: Default::default(),
            };

            databases.insert(db_path.clone(), db.clone());

            db
        };

        // Ensure that multiple database instances are possible only when using the test storage
        // mode, and that in such scenarios, all of the instances are only using the test mode.
        if matches!(storage, StorageMode::Test(_)) {
            ensure!(databases.values().all(|db| matches!(&db.storage_mode, StorageMode::Test(_))));
        } else {
            ensure!(databases.len() == 1, "There can only be one active rocksDB database when not in test mode.");
        }

        // Refuse a schema this build does not understand, before anything reads it.
        check_storage_version(&database.rocksdb, network_id)?;

        // Ensure the database network ID and storage mode match.
        match database.network_id == network_id && database.storage_mode == storage {
            true => Ok(database),
            false => bail!("Mismatching network ID or storage mode in the database"),
        }
    }

    /// Opens the map with the given `network_id`, `storage mode`, and `map_id` from storage.
    fn open_map<
        S: Into<StorageMode>,
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
        T: Into<u16>,
    >(
        network_id: u16,
        storage: S,
        map_id: T,
    ) -> Result<DataMap<K, V>> {
        // Open the RocksDB database.
        let database = Self::open(network_id, storage)?;

        // Combine contexts to create a new scope.
        let mut context = database.network_id.to_le_bytes().to_vec();
        context.extend_from_slice(&(map_id.into()).to_le_bytes());

        // Return the DataMap.
        Ok(DataMap(Arc::new(InnerDataMap {
            database,
            context,
            batch_in_progress: Default::default(),
            atomic_batch: Default::default(),
            checkpoints: Default::default(),
        })))
    }

    /// Opens the nested map with the given `network_id`, `storage mode`, and `map_id` from storage.
    fn open_nested_map<
        S: Into<StorageMode>,
        M: Serialize + DeserializeOwned,
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
        T: Into<u16>,
    >(
        network_id: u16,
        storage: S,
        map_id: T,
    ) -> Result<NestedDataMap<M, K, V>> {
        // Open the RocksDB database.
        let database = Self::open(network_id, storage)?;

        // Combine contexts to create a new scope.
        let mut context = database.network_id.to_le_bytes().to_vec();
        context.extend_from_slice(&(map_id.into()).to_le_bytes());

        // Return the DataMap.
        Ok(NestedDataMap {
            database,
            context,
            batch_in_progress: Default::default(),
            atomic_batch: Default::default(),
            checkpoints: Default::default(),
        })
    }
}

impl RocksDB {
    /// Pause the execution of atomic writes for the entire database.
    fn pause_atomic_writes(&self) -> Result<()> {
        // This operation is only intended to be performed before or after
        // atomic batches - never in the middle of them.
        assert_eq!(self.atomic_depth.load(Ordering::SeqCst), 0);

        // Set the flag indicating that the pause is in effect.
        let already_paused = self.atomic_writes_paused.swap(true, Ordering::SeqCst);
        // Make sure that we haven't already paused atomic writes (which would
        // indicate a logic bug).
        assert!(!already_paused);

        Ok(())
    }

    /// Unpause the execution of atomic writes for the entire database; this
    /// executes all the writes that have been queued since they were paused.
    fn unpause_atomic_writes<const DISCARD_BATCH: bool>(&self) -> Result<()> {
        // Ensure the call to unpause is only performed before or after an atomic batch scope
        // - and never in the middle of one (otherwise there is a fundamental logic bug).
        // Note: In production, this `ensure` is a safety-critical invariant that never fails.
        ensure!(self.atomic_depth.load(Ordering::SeqCst) == 0, "Atomic depth must be 0 to unpause atomic writes");

        // https://github.com/rust-lang/rust/issues/98485
        let currently_paused = self.atomic_writes_paused.load(Ordering::SeqCst);
        // Ensure the database is paused (otherwise there is a fundamental logic bug).
        // Note: In production, this `ensure` is a safety-critical invariant that never fails.
        ensure!(currently_paused, "Atomic writes must be paused to unpause them");

        // In order to ensure that all the operations that are intended
        // to be atomic via the usual macro approach are still performed
        // atomically (just as a part of a larger batch), every atomic
        // storage operation that has accumulated from the moment the
        // writes have been paused becomes executed as a single atomic batch.
        let batch = mem::take(&mut *self.atomic_batch.lock());
        if !DISCARD_BATCH {
            self.rocksdb.write(batch)?;
        }

        // Unset the flag indicating that the pause is in effect.
        self.atomic_writes_paused.store(false, Ordering::SeqCst);

        Ok(())
    }

    /// Checks whether the atomic writes are currently paused.
    fn are_atomic_writes_paused(&self) -> bool {
        self.atomic_writes_paused.load(Ordering::SeqCst)
    }

    /// Reads key RocksDB internal properties and publishes them to the metrics registry.
    ///
    /// This is a lightweight, synchronous call — properties are read from RocksDB's in-memory
    /// counters with no disk I/O.  Call it from an existing background loop (e.g. the
    /// auto-checkpoint polling loop in snarkOS); there is no need to spawn a dedicated thread.
    #[cfg(feature = "metrics")]
    pub fn export_rocksdb_metrics(&self) {
        use snarkvm_metrics::rocksdb as names;

        /// Read a single integer property; silently skip on error (DB may be closing).
        fn prop(db: &rocksdb::DB, key: &str) -> Option<u64> {
            db.property_int_value(key).ok().flatten()
        }

        let db = &self.rocksdb;

        // Compaction pressure
        if let Some(v) = prop(db, "rocksdb.compaction-pending") {
            snarkvm_metrics::gauge(names::COMPACTION_PENDING, v as f64);
        }
        if let Some(v) = prop(db, "rocksdb.estimate-pending-compaction-bytes") {
            snarkvm_metrics::gauge(names::ESTIMATE_PENDING_COMPACTION_BYTES, v as f64);
        }
        if let Some(v) = prop(db, "rocksdb.num-running-compactions") {
            snarkvm_metrics::gauge(names::NUM_RUNNING_COMPACTIONS, v as f64);
        }
        if let Some(v) = prop(db, "rocksdb.num-running-flushes") {
            snarkvm_metrics::gauge(names::NUM_RUNNING_FLUSHES, v as f64);
        }
        if let Some(v) = prop(db, "rocksdb.mem-table-flush-pending") {
            snarkvm_metrics::gauge(names::MEM_TABLE_FLUSH_PENDING, v as f64);
        }

        // Disk footprint
        if let Some(v) = prop(db, "rocksdb.total-sst-files-size") {
            snarkvm_metrics::gauge(names::TOTAL_SST_FILES_SIZE, v as f64);
        }
        if let Some(v) = prop(db, "rocksdb.live-sst-files-size") {
            snarkvm_metrics::gauge(names::LIVE_SST_FILES_SIZE, v as f64);
        }

        // General state
        if let Some(v) = prop(db, "rocksdb.estimate-num-keys") {
            snarkvm_metrics::gauge(names::ESTIMATE_NUM_KEYS, v as f64);
        }
        if let Some(v) = prop(db, "rocksdb.num-snapshots") {
            snarkvm_metrics::gauge(names::NUM_SNAPSHOTS, v as f64);
        }

        // Per-level SST file counts (levels 0–6)
        for (level, &name) in names::NUM_FILES_AT_LEVEL.iter().enumerate() {
            let key = format!("rocksdb.num-files-at-level{level}");
            if let Some(v) = prop(db, &key) {
                snarkvm_metrics::gauge(name, v as f64);
            }
        }
    }
}

// impl RocksDB {
//     /// Imports a file with the given path to reconstruct storage.
//     fn import<P: AsRef<Path>>(&self, path: P) -> Result<()> {
//         let file = File::open(path)?;
//         let mut reader = BufReader::new(file);
//
//         let len = reader.seek(SeekFrom::End(0))?;
//         reader.rewind()?;
//
//         let mut buf = vec![0u8; 16 * 1024];
//
//         while reader.stream_position()? < len {
//             reader.read_exact(&mut buf[..4])?;
//             let key_len = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
//
//             if key_len + 4 > buf.len() {
//                 buf.resize(key_len + 4, 0);
//             }
//
//             reader.read_exact(&mut buf[..key_len + 4])?;
//             let value_len = u32::from_le_bytes(buf[key_len..][..4].try_into().unwrap()) as usize;
//
//             if key_len + value_len > buf.len() {
//                 buf.resize(key_len + value_len, 0);
//             }
//
//             reader.read_exact(&mut buf[key_len..][..value_len])?;
//
//             self.rocksdb.put(&buf[..key_len], &buf[key_len..][..value_len])?;
//         }
//
//         Ok(())
//     }
//
//     /// Exports the current state of storage to a single file at the specified location.
//     fn export<P: AsRef<Path>>(&self, path: P) -> Result<()> {
//         let file = File::create(path)?;
//         let mut writer = BufWriter::new(file);
//
//         let mut iterator = self.rocksdb.raw_iterator();
//         iterator.seek_to_first();
//
//         while iterator.valid() {
//             if let (Some(key), Some(value)) = (iterator.key(), iterator.value()) {
//                 writer.write_all(&(key.len() as u32).to_le_bytes())?;
//                 writer.write_all(key)?;
//
//                 writer.write_all(&(value.len() as u32).to_le_bytes())?;
//                 writer.write_all(value)?;
//             }
//             iterator.next();
//         }
//
//         Ok(())
//     }
// }
