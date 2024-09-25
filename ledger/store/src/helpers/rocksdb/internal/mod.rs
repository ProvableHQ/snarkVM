// Copyright 2024 Aleo Network Foundation
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
use anyhow::{bail, ensure, Result};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    borrow::Borrow,
    marker::PhantomData,
    mem,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

pub const PREFIX_LEN: usize = 4; // N::ID (u16) + DataID (u16)

pub trait Database {
    /// Opens the database.
    fn open<S: Clone + Into<StorageMode>>(network_id: u16, storage: S) -> Result<Self>
    where
        Self: Sized;

    /// Opens the map with the given `network_id`, `storage mode`, and `map_id` from storage.
    fn open_map<
        S: Clone + Into<StorageMode>,
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
        T: Into<u16>,
    >(
        network_id: u16,
        storage: S,
        map_id: T,
    ) -> Result<DataMap<K, V>>;

    /// Opens the nested map with the given `network_id`, `storage mode`, and `map_id` from storage.
    fn open_nested_map<
        S: Clone + Into<StorageMode>,
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
    fn open<S: Clone + Into<StorageMode>>(network_id: u16, storage: S) -> Result<Self> {
        static DB: OnceCell<RocksDB> = OnceCell::new();

        // Retrieve the database.
        let database = DB
            .get_or_try_init(|| {
                // Customize database options.
                let mut options = rocksdb::Options::default();
                options.set_compression_type(rocksdb::DBCompressionType::Lz4);

                // Register the prefix length.
                let prefix_extractor = rocksdb::SliceTransform::create_fixed_prefix(PREFIX_LEN);
                options.set_prefix_extractor(prefix_extractor);

                let primary = aleo_std_storage::aleo_ledger_dir(network_id, storage.clone().into());
                let rocksdb = {
                    options.increase_parallelism(2);
                    options.set_max_background_jobs(4);
                    options.create_if_missing(true);

                    Arc::new(rocksdb::DB::open(&options, primary)?)
                };

                Ok::<_, anyhow::Error>(RocksDB {
                    rocksdb,
                    network_id,
                    storage_mode: storage.clone().into(),
                    atomic_batch: Default::default(),
                    atomic_depth: Default::default(),
                    atomic_writes_paused: Default::default(),
                    default_readopts: Default::default(),
                })
            })?
            .clone();

        // Ensure the database network ID and storage mode match.
        match database.network_id == network_id && database.storage_mode == storage.into() {
            true => Ok(database),
            false => bail!("Mismatching network ID or storage mode in the database"),
        }
    }

    /// Opens the map with the given `network_id`, `storage mode`, and `map_id` from storage.
    fn open_map<
        S: Clone + Into<StorageMode>,
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
        S: Clone + Into<StorageMode>,
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

    /// Opens the test database.
    #[cfg(any(test, feature = "test"))]
    pub fn open_testing(temp_dir: std::path::PathBuf, dev: Option<u16>) -> Result<Self> {
        use console::prelude::{Rng, TestRng};

        // Ensure the `temp_dir` is unique.
        let temp_dir = temp_dir.join(Rng::gen::<u64>(&mut TestRng::default()).to_string());

        // Construct the directory for the test database.
        let primary = match dev {
            Some(dev) => temp_dir.join(dev.to_string()),
            None => temp_dir,
        };

        // Prepare the storage mode.
        let storage_mode = StorageMode::from(primary.clone());

        let database = {
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

                // Keep these around as options for configuration testing.

                // options.set_max_subcompactions(4);
                // options.set_use_direct_io_for_flush_and_compaction(true);
                // options.set_bytes_per_sync(1 << 28);
                // options.set_compaction_readahead_size(1 << 28);
                // options.set_max_write_buffer_number(16);
                // options.set_min_write_buffer_number_to_merge(8);
                // options.set_compression_type(rocksdb::DBCompressionType::None);
                // options.set_bottommost_compression_type(rocksdb::DBCompressionType::None);
                // options.set_write_buffer_size(1 << 28);

                Arc::new(rocksdb::DB::open(&options, primary)?)
            };

            Ok::<_, anyhow::Error>(RocksDB {
                rocksdb,
                network_id: u16::MAX,
                storage_mode: storage_mode.clone(),
                atomic_batch: Default::default(),
                atomic_depth: Default::default(),
                atomic_writes_paused: Default::default(),
                default_readopts: Default::default(),
            })
        }?;

        // Ensure the database storage mode match.
        match database.storage_mode == storage_mode {
            true => Ok(database),
            false => bail!("Mismatching storage mode in the test database"),
        }
    }

    /// Opens the test map.
    #[cfg(any(test, feature = "test"))]
    pub fn open_map_testing<K: Serialize + DeserializeOwned, V: Serialize + DeserializeOwned, T: Into<u16>>(
        temp_dir: std::path::PathBuf,
        dev: Option<u16>,
        map_id: T,
    ) -> Result<DataMap<K, V>> {
        // Open the RocksDB test database.
        let database = Self::open_testing(temp_dir, dev)?;

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

    /// Opens the test nested map.
    #[cfg(any(test, feature = "test"))]
    pub fn open_nested_map_testing<
        M: Serialize + DeserializeOwned,
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
        T: Into<u16>,
    >(
        temp_dir: std::path::PathBuf,
        dev: Option<u16>,
        map_id: T,
    ) -> Result<NestedDataMap<M, K, V>> {
        // Open the RocksDB test database.
        let database = Self::open_testing(temp_dir, dev)?;

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
