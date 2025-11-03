// Copyright (c) 2019-2025 Provable Inc.
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

mod id;
pub use id::*;

mod map;
pub use map::*;

mod nested_map;
pub use nested_map::*;

#[cfg(test)]
mod tests;

use aleo_std_storage::StorageMode;
use anyhow::{Result, bail, ensure};
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
