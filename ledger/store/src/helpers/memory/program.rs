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
    helpers::memory::{MemoryMap, NestedMemoryMap},
};
#[cfg(feature = "history-staking-rewards")]
use console::types::Address;
use console::{
    prelude::*,
    program::{Identifier, Plaintext, ProgramID, Value},
};
use snarkvm_ledger_committee::Committee;

use aleo_std_storage::StorageMode;
use indexmap::IndexSet;
#[cfg(feature = "history")]
use std::sync::{Arc, atomic::AtomicU32};

/// An in-memory finalize storage.
#[derive(Clone)]
pub struct FinalizeMemory<N: Network> {
    /// The committee store.
    committee_store: CommitteeStore<N, CommitteeMemory<N>>,
    /// The program ID map.
    program_id_map: MemoryMap<ProgramID<N>, IndexSet<Identifier<N>>>,
    /// The key-value map.
    key_value_map: NestedMemoryMap<(ProgramID<N>, Identifier<N>), Plaintext<N>, Value<N>>,
    /// The historical mapping value update map.
    #[cfg(feature = "history")]
    mapping_update_map: MemoryMap<(ProgramID<N>, Identifier<N>, Plaintext<N>, u32), Value<N>>,
    /// The historical mapping update heights map.
    #[cfg(feature = "history")]
    mapping_update_heights_map: MemoryMap<(ProgramID<N>, Identifier<N>, Plaintext<N>), Vec<u32>>,
    /// The current block height.
    #[cfg(feature = "history")]
    block_height: Arc<AtomicU32>,
    /// The historical staking rewards map.
    #[cfg(feature = "history-staking-rewards")]
    staking_rewards_map: MemoryMap<(Address<N>, u32), (Address<N>, u64, u64)>,
    /// The storage mode.
    storage_mode: StorageMode,
}

#[rustfmt::skip]
impl<N: Network> FinalizeStorage<N> for FinalizeMemory<N> {
    type CommitteeStorage = CommitteeMemory<N>;
    type ProgramIDMap = MemoryMap<ProgramID<N>, IndexSet<Identifier<N>>>;
    type KeyValueMap = NestedMemoryMap<(ProgramID<N>, Identifier<N>), Plaintext<N>, Value<N>>;
    #[cfg(feature = "history")]
    type MappingUpdateMap = MemoryMap<(ProgramID<N>, Identifier<N>, Plaintext<N>, u32), Value<N>>;
    #[cfg(feature = "history")]
    type MappingUpdateHeightsMap = MemoryMap<(ProgramID<N>, Identifier<N>, Plaintext<N>), Vec<u32>>;
    #[cfg(feature = "history-staking-rewards")]
    type StakingRewardsMap = MemoryMap<(Address<N>, u32), (Address<N>, u64, u64)>;

    /// Initializes the finalize storage.
    fn open<S: Into<StorageMode>>(storage: S) -> Result<Self> {
        let storage = storage.into();
        // Initialize the committee store.
        let committee_store = CommitteeStore::<N, CommitteeMemory<N>>::open(storage.clone())?;
        // Return the finalize store.
        Ok(Self {
            committee_store,
            program_id_map: MemoryMap::default(),
            key_value_map: NestedMemoryMap::default(),
            #[cfg(feature = "history")]
            mapping_update_map: MemoryMap::default(),
            #[cfg(feature = "history")]
            mapping_update_heights_map: MemoryMap::default(),
            #[cfg(feature = "history")]
            block_height: Default::default(),
            #[cfg(feature = "history-staking-rewards")]
            staking_rewards_map: MemoryMap::default(),
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

    /// Returns the historical value map.
    #[cfg(feature = "history")]
    fn mapping_update_map(&self) -> &Self::MappingUpdateMap {
        &self.mapping_update_map
    }

    #[cfg(feature = "history")]
    fn mapping_update_heights_map(&self) -> &Self::MappingUpdateHeightsMap {
        &self.mapping_update_heights_map
    }

    /// Returns the historical staking rewards map.
    #[cfg(feature = "history-staking-rewards")]
    fn staking_rewards_map(&self) -> &Self::StakingRewardsMap {
        &self.staking_rewards_map
    }

    /// Returns the storage mode.
    fn storage_mode(&self) -> &StorageMode {
        &self.storage_mode
    }

    /// Returns the current block height.
    #[cfg(feature = "history")]
    fn current_block_height(&self) -> &AtomicU32 {
        &self.block_height
    }
}

/// An in-memory committee storage.
#[derive(Clone)]
pub struct CommitteeMemory<N: Network> {
    /// The current round map.
    current_round_map: MemoryMap<u8, u64>,
    /// The round to height map.
    round_to_height_map: MemoryMap<u64, u32>,
    /// The committee map.
    committee_map: MemoryMap<u32, Committee<N>>,
    /// The storage mode.
    storage_mode: StorageMode,
}

#[rustfmt::skip]
impl<N: Network> CommitteeStorage<N> for CommitteeMemory<N> {
    type CurrentRoundMap = MemoryMap<u8, u64>;
    type RoundToHeightMap = MemoryMap<u64, u32>;
    type CommitteeMap = MemoryMap<u32, Committee<N>>;

    /// Initializes the committee storage.
    fn open<S: Into<StorageMode>>(storage: S) -> Result<Self> {
        Ok(Self {
            current_round_map: MemoryMap::default(),
            round_to_height_map: MemoryMap::default(),
            committee_map: MemoryMap::default(),
            storage_mode: storage.into(),
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
