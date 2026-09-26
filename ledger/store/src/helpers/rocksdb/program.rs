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

#![allow(clippy::type_complexity)]

use crate::{
    CommitteeStorage,
    CommitteeStore,
    FinalizeStorage,
    HeightBytes,
    HistoricalMappingValue,
    HistoryEvent,
    HistoryRecording,
    HistoryRow,
    HistoryTable,
    helpers::rocksdb::{self, CommitteeMap, DataMap, Database, MapID, NestedDataMap, ProgramMap},
};
use console::{
    prelude::*,
    program::{Identifier, Plaintext, ProgramID, Value},
    types::{Address, Field},
};
use snarkvm_ledger_block::RejectedReason;
use snarkvm_ledger_committee::Committee;

use aleo_std_storage::StorageMode;
use indexmap::IndexSet;
#[cfg(feature = "locktick")]
use locktick::parking_lot::RwLock;
#[cfg(not(feature = "locktick"))]
use parking_lot::RwLock;
use snarkvm_utilities::bytes::unchecked_deserialize;
use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicU32, Ordering},
};

/// A RocksDB finalize storage.
#[derive(Clone)]
pub struct FinalizeDB<N: Network> {
    /// The committee store.
    committee_store: CommitteeStore<N, CommitteeDB<N>>,
    /// The program ID map.
    program_id_map: DataMap<ProgramID<N>, IndexSet<Identifier<N>>>,
    /// The key-value map.
    key_value_map: NestedDataMap<(ProgramID<N>, Identifier<N>), Plaintext<N>, Value<N>>,
    /// The rejection reason map.
    rejected_reason_map: DataMap<Field<N>, RejectedReason<N>>,
    /// The historical mapping value map (keyed by big-endian block height).
    mapping_update_map: DataMap<(ProgramID<N>, Identifier<N>, Plaintext<N>, HeightBytes), HistoricalMappingValue<N>>,
    /// The historical staking rewards map (keyed by big-endian block height).
    staking_rewards_map: DataMap<(Address<N>, HeightBytes), (Address<N>, u64, u64)>,
    /// The per-block history event log.
    history_event_map: DataMap<(HeightBytes, HeightBytes), HistoryEvent<N>>,
    /// The current block height.
    block_height: Arc<AtomicU32>,
    /// Where mapping updates and staking rewards are recorded.
    history_recording: Arc<AtomicU8>,
    /// The programs whose mapping history is recorded, or `None` for every program.
    history_programs: Arc<RwLock<Option<IndexSet<ProgramID<N>>>>>,
    /// Sequence number of the next history event in the current block.
    history_event_seq: Arc<AtomicU32>,
    /// The next block height history indexing will process.
    history_synced_height: Arc<AtomicU32>,
    /// The database that stores the history sync cursor.
    database: rocksdb::RocksDB,
    /// The storage mode.
    storage_mode: StorageMode,
}

#[rustfmt::skip]
impl<N: Network> FinalizeStorage<N> for FinalizeDB<N> {
    type CommitteeStorage = CommitteeDB<N>;
    type ProgramIDMap = DataMap<ProgramID<N>, IndexSet<Identifier<N>>>;
    type KeyValueMap = NestedDataMap<(ProgramID<N>, Identifier<N>), Plaintext<N>, Value<N>>;
    type RejectedReasonMap = DataMap<Field<N>, RejectedReason<N>>;
    type MappingUpdateMap =
        DataMap<(ProgramID<N>, Identifier<N>, Plaintext<N>, HeightBytes), HistoricalMappingValue<N>>;
    type StakingRewardsMap = DataMap<(Address<N>, HeightBytes), (Address<N>, u64, u64)>;
    type HistoryEventMap = DataMap<(HeightBytes, HeightBytes), HistoryEvent<N>>;

    /// Initializes the finalize storage.
    fn open<S: Into<StorageMode>>(storage: S) -> Result<Self> {
        let storage = storage.into();
        // Open the database first so the schema migration has stored the history cursor.
        let database = rocksdb::RocksDB::open(N::ID, storage.clone())?;
        let history_synced_height = Arc::new(AtomicU32::new(database.history_synced_height()?));
        // Initialize the committee store.
        let committee_store = CommitteeStore::<N, CommitteeDB<N>>::open(storage.clone())?;
        // Seed the history height guard from the last committed block height so that
        // historical REST queries succeed immediately after node startup (before the
        // first new block arrives and re-seeds the value via `atomic_finalize`).
        // Returns 0 for a fresh database that has no committee data yet.
        let initial_height = committee_store.current_height().unwrap_or(0);
        // Return the finalize storage.
        Ok(Self {
            committee_store,
            program_id_map: rocksdb::RocksDB::open_map(N::ID, storage.clone(), MapID::Program(ProgramMap::ProgramID))?,
            key_value_map: rocksdb::RocksDB::open_nested_map(N::ID, storage.clone(), MapID::Program(ProgramMap::KeyValueID))?,
            rejected_reason_map: rocksdb::RocksDB::open_map(N::ID, storage.clone(), MapID::Program(ProgramMap::RejectedReason))?,
            mapping_update_map: rocksdb::RocksDB::open_map(N::ID, storage.clone(), MapID::Program(ProgramMap::MappingUpdate))?,
            staking_rewards_map: rocksdb::RocksDB::open_map(N::ID, storage.clone(), MapID::Program(ProgramMap::StakingRewards))?,
            history_event_map: rocksdb::RocksDB::open_map(N::ID, storage.clone(), MapID::Program(ProgramMap::HistoryEvent))?,
            block_height: Arc::new(AtomicU32::new(initial_height)),
            history_recording: Arc::new(AtomicU8::new(HistoryRecording::Off as u8)),
            history_programs: Default::default(),
            history_event_seq: Arc::new(AtomicU32::new(0)),
            history_synced_height,
            database,
            storage_mode: storage,
        })
    }

    /// Returns the committee store.
    fn committee_store(&self) -> &CommitteeStore<N, Self::CommitteeStorage> {
        &self.committee_store
    }

    /// Returns the program ID map.
    fn program_id_map(&self) -> &Self::ProgramIDMap {
        &self.program_id_map
    }

    /// Returns the key-value map.
    fn key_value_map(&self) -> &Self::KeyValueMap {
        &self.key_value_map
    }

    /// Returns the rejection reason map.
    fn rejected_reason_map(&self) -> &Self::RejectedReasonMap {
        &self.rejected_reason_map
    }

    /// Returns the historical value map.
    fn mapping_update_map(&self) -> &Self::MappingUpdateMap {
        &self.mapping_update_map
    }

    /// Returns the historical staking rewards map.
    fn staking_rewards_map(&self) -> &Self::StakingRewardsMap {
        &self.staking_rewards_map
    }

    /// Returns the per-block history event log.
    fn history_event_map(&self) -> &Self::HistoryEventMap {
        &self.history_event_map
    }

    /// Returns the storage mode.
    fn storage_mode(&self) -> &StorageMode {
        &self.storage_mode
    }

    /// Returns where history is recorded.
    fn history_recording(&self) -> &AtomicU8 {
        &self.history_recording
    }

    /// Returns the programs whose mapping history is recorded.
    fn history_programs(&self) -> &RwLock<Option<IndexSet<ProgramID<N>>>> {
        &self.history_programs
    }

    /// Returns the program list stored with this store's history.
    fn stored_history_programs(&self) -> Result<Option<IndexSet<ProgramID<N>>>> {
        match self.database.history_programs()? {
            Some(bytes) => Ok(Some(unchecked_deserialize(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Stores the program list this store's history is recorded for.
    fn store_history_programs(&self, programs: &IndexSet<ProgramID<N>>) -> Result<()> {
        self.database.set_history_programs(&bincode::serialize(programs)?)
    }

    /// Deletes the history tables, the event log, and the stored program list, and sets the
    /// history cursor to 0.
    fn reset_history(&self) -> Result<()> {
        for map in [ProgramMap::MappingUpdate, ProgramMap::StakingRewards, ProgramMap::HistoryEvent] {
            self.database.delete_map(MapID::Program(map))?;
        }
        self.database.delete_history_programs()?;
        self.set_history_synced_height(0)
    }

    /// Returns the per-block history event sequence.
    fn history_event_seq(&self) -> &AtomicU32 {
        &self.history_event_seq
    }

    /// Deletes the history events of every height below `height`, with one range deletion.
    fn prune_history_events_below(&self, height: u32) -> Result<()> {
        // Event keys serialize as the big-endian height followed by the big-endian sequence.
        let end = [height.to_be_bytes(), [0u8; 4]].concat();
        self.database.delete_map_range(MapID::Program(ProgramMap::HistoryEvent), &[], &end)
    }

    /// Writes serialized history-table records in one write batch.
    fn put_history_rows(&self, rows: Vec<HistoryRow>) -> Result<()> {
        self.database.put_map_rows(rows.into_iter().map(|(table, key, value)| {
            let map = match table {
                HistoryTable::MappingUpdates => ProgramMap::MappingUpdate,
                HistoryTable::StakingRewards => ProgramMap::StakingRewards,
            };
            (MapID::Program(map), key, value)
        }))
    }

    /// Returns the next block height history indexing will process.
    fn history_synced_height(&self) -> u32 {
        self.history_synced_height.load(Ordering::SeqCst)
    }

    /// Stores the next block height history indexing will process.
    fn set_history_synced_height(&self, height: u32) -> Result<()> {
        self.database.set_history_synced_height(height)?;
        self.history_synced_height.store(height, Ordering::SeqCst);
        Ok(())
    }

    /// Returns the current block height.
    fn current_block_height(&self) -> &AtomicU32 {
        &self.block_height
    }
}

/// A RocksDB committee storage.
#[derive(Clone)]
pub struct CommitteeDB<N: Network> {
    /// The current round map.
    current_round_map: DataMap<u8, u64>,
    /// The round to height map.
    round_to_height_map: DataMap<u64, u32>,
    /// The committee map.
    committee_map: DataMap<u32, Committee<N>>,
    /// The storage mode.
    storage_mode: StorageMode,
}

#[rustfmt::skip]
impl<N: Network> CommitteeStorage<N> for CommitteeDB<N> {
    type CurrentRoundMap = DataMap<u8, u64>;
    type RoundToHeightMap = DataMap<u64, u32>;
    type CommitteeMap = DataMap<u32, Committee<N>>;

    /// Initializes the committee storage.
    fn open<S: Into<StorageMode>>(storage: S) -> Result<Self> {
        let storage = storage.into();
        Ok(Self {
            current_round_map: rocksdb::RocksDB::open_map(N::ID, storage.clone(), MapID::Committee(CommitteeMap::CurrentRound))?,
            round_to_height_map: rocksdb::RocksDB::open_map(N::ID, storage.clone(), MapID::Committee(CommitteeMap::RoundToHeight))?,
            committee_map: rocksdb::RocksDB::open_map(N::ID, storage.clone(), MapID::Committee(CommitteeMap::Committee))?,
            storage_mode: storage,
        })
    }

    /// Returns the current round map.
    fn current_round_map(&self) -> &Self::CurrentRoundMap {
        &self.current_round_map
    }

    /// Returns the round to height map.
    fn round_to_height_map(&self) -> &Self::RoundToHeightMap {
        &self.round_to_height_map
    }

    /// Returns the committee map.
    fn committee_map(&self) -> &Self::CommitteeMap {
        &self.committee_map
    }

    /// Returns the storage mode.
    fn storage_mode(&self) -> &StorageMode {
        &self.storage_mode
    }
}
