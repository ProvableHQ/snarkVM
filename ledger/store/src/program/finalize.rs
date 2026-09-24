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

use crate::{
    atomic_batch_scope,
    helpers::{Map, MapRead, NestedMap, NestedMapRead},
    program::{CommitteeStorage, CommitteeStore},
};
use console::{
    network::prelude::*,
    program::{Identifier, Plaintext, ProgramID, Value},
    types::{Address, Field},
};
use snarkvm_ledger_block::RejectedReason;
use snarkvm_synthesizer_program::{FinalizeOperation, FinalizeStoreTrait};

use aleo_std_storage::StorageMode;
use anyhow::Result;
use core::marker::PhantomData;
use indexmap::{IndexMap, IndexSet};
use std::{
    borrow::Cow,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU32, Ordering},
    },
};

/// The block height component of a history-map key, stored as 4 big-endian bytes.
///
/// Big-endian encoding makes lexicographic key order match numeric height order, so a floor seek
/// returns the latest record at or before a height.
pub(crate) type HeightBytes = [u8; 4];

/// A mapping value recorded at one block height.
///
/// `Absent` is a deletion. A later read at or after that height treats the key as missing until a
/// `Present` record overwrites it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(serialize = "N: Network", deserialize = "N: Network"))]
pub enum HistoricalMappingValue<N: Network> {
    /// The key held this value at the recorded height.
    Present(Value<N>),
    /// The key was removed at the recorded height.
    Absent,
}

/// Where a finalize store writes the history of mapping updates and staking rewards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum HistoryRecording {
    /// Nothing is recorded.
    Off = 0,
    /// Records go to the tables that serve historical reads.
    Tables = 1,
    /// Records go to the per-block event log, for another store to import.
    Events = 2,
}

impl HistoryRecording {
    /// Reads the mode stored in `value`.
    fn load(value: &AtomicU8) -> Self {
        match value.load(Ordering::SeqCst) {
            1 => Self::Tables,
            2 => Self::Events,
            _ => Self::Off,
        }
    }
}

/// One history record written while a block was finalized.
///
/// Records for a single height are stored under `(height, sequence)` so a later pass can copy
/// that block's history without scanning every key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(serialize = "N: Network", deserialize = "N: Network"))]
pub enum HistoryEvent<N: Network> {
    /// A mapping key was set or removed.
    Mapping {
        /// Program that owns the mapping.
        program_id: ProgramID<N>,
        /// Mapping name.
        mapping_name: Identifier<N>,
        /// Mapping key.
        key: Plaintext<N>,
        /// Value at this height, or a deletion.
        value: Box<HistoricalMappingValue<N>>,
    },
    /// The block's history was committed. Present even when the block changed no mappings.
    Indexed,
    /// A staking reward paid at this height.
    Staking {
        /// Account that received the reward.
        staker: Address<N>,
        /// Validator the staker was bonded to.
        validator: Address<N>,
        /// Reward paid at this height, in microcredits.
        reward: u64,
        /// Stake after the reward was applied, in microcredits.
        new_stake: u64,
    },
}

/// TODO (howardwu): Remove this.
/// Returns the mapping ID for the given `program ID` and `mapping name`.
fn to_mapping_id<N: Network>(program_id: &ProgramID<N>, mapping_name: &Identifier<N>) -> Result<Field<N>> {
    // Construct the preimage.
    let mut preimage = Vec::new();
    program_id.write_bits_le(&mut preimage);
    false.write_bits_le(&mut preimage); // Separator
    mapping_name.write_bits_le(&mut preimage);
    // Compute the mapping ID.
    N::hash_bhp1024(&preimage)
}

/// Returns the key ID for the given `program ID`, `mapping name`, and `key`.
fn to_key_id<N: Network>(
    program_id: &ProgramID<N>,
    mapping_name: &Identifier<N>,
    key: &Plaintext<N>,
) -> Result<Field<N>> {
    // Construct the preimage.
    let mut preimage = Vec::new();
    program_id.write_bits_le(&mut preimage);
    false.write_bits_le(&mut preimage); // Separator
    mapping_name.write_bits_le(&mut preimage);
    false.write_bits_le(&mut preimage); // Separator
    key.write_bits_le(&mut preimage);
    // Compute the key ID.
    N::hash_bhp1024(&preimage)
}

/// A trait for program state storage. Note: For the program logic, see `DeploymentStorage`.
///
/// We define the `key ID := Hash ( program ID || mapping name || Hash(key) )`
/// and the `value ID := Hash ( key ID || Hash(value) )`.
///
/// `FinalizeStorage` emulates the following data structure:
/// ```text
/// // (program_id => (mapping_name => (key => value)))
/// BTreeMap<ProgramID<N>, BTreeMap<Identifier<N>, BTreeMap<Key, Value>>>
/// ```
pub trait FinalizeStorage<N: Network>: 'static + Clone + Send + Sync {
    /// The committee storage.
    type CommitteeStorage: CommitteeStorage<N>;
    /// The mapping of `program ID` to `[mapping name]`.
    type ProgramIDMap: for<'a> Map<'a, ProgramID<N>, IndexSet<Identifier<N>>>;
    /// The mapping of `(program ID, mapping name)` to `[(key, value)]`.
    type KeyValueMap: for<'a> NestedMap<'a, (ProgramID<N>, Identifier<N>), Plaintext<N>, Value<N>>;
    /// The mapping of `transaction ID` to `rejection reason`.
    type RejectedReasonMap: for<'a> Map<'a, Field<N>, RejectedReason<N>>;
    /// The mapping of `(program ID, mapping name, key, height)` to [`HistoricalMappingValue`].
    ///
    /// The height is big-endian so a floor seek returns the latest record at or before a height.
    type MappingUpdateMap: for<'a> Map<'a, (ProgramID<N>, Identifier<N>, Plaintext<N>, HeightBytes), HistoricalMappingValue<N>>;
    /// The mapping of `(staker address, height)` to `(validator address, block reward, new stake)`.
    type StakingRewardsMap: for<'a> Map<'a, (Address<N>, u32), (Address<N>, u64, u64)>;
    /// The mapping of `(height, sequence)` to the history record written at that position.
    type HistoryEventMap: for<'a> Map<'a, (HeightBytes, HeightBytes), HistoryEvent<N>>;

    /// Initializes the program state storage.
    fn open<S: Into<StorageMode>>(storage: S) -> Result<Self>;

    /// Returns the committee storage.
    fn committee_store(&self) -> &CommitteeStore<N, Self::CommitteeStorage>;
    /// Returns the program ID map.
    fn program_id_map(&self) -> &Self::ProgramIDMap;
    /// Returns the key-value map.
    fn key_value_map(&self) -> &Self::KeyValueMap;
    /// Returns the rejection reason map.
    fn rejected_reason_map(&self) -> &Self::RejectedReasonMap;
    /// Returns the historical mapping value map.
    fn mapping_update_map(&self) -> &Self::MappingUpdateMap;
    /// Returns the historical staking rewards map.
    fn staking_rewards_map(&self) -> &Self::StakingRewardsMap;
    /// Returns the per-block history event log.
    fn history_event_map(&self) -> &Self::HistoryEventMap;

    /// Returns the storage mode.
    fn storage_mode(&self) -> &StorageMode;

    /// Returns where mapping updates and staking rewards are recorded, as a [`HistoryRecording`]
    /// discriminant.
    fn history_recording(&self) -> &AtomicU8;

    /// Returns the next block height history indexing will process.
    ///
    /// A height `h` is indexed when `h` is strictly less than this value.
    fn history_synced_height(&self) -> u32;

    /// Stores the next block height history indexing will process.
    ///
    /// This write is outside the finalize atomic batch.
    fn set_history_synced_height(&self, height: u32) -> Result<()>;

    /// Sequence number of the next history event in the block currently being finalized.
    fn history_event_seq(&self) -> &AtomicU32;

    /// Deletes the history events of every height below `height`.
    ///
    /// The deletion is outside any atomic batch; call it while no batch on this store is open.
    fn prune_history_events_below(&self, height: u32) -> Result<()>;

    /// Starts an atomic batch write operation.
    fn start_atomic(&self) {
        self.committee_store().start_atomic();
        self.program_id_map().start_atomic();
        self.key_value_map().start_atomic();
        self.rejected_reason_map().start_atomic();
        self.mapping_update_map().start_atomic();
        self.staking_rewards_map().start_atomic();
        self.history_event_map().start_atomic();
    }

    /// Checks if an atomic batch is in progress.
    fn is_atomic_in_progress(&self) -> bool {
        self.committee_store().is_atomic_in_progress()
            || self.program_id_map().is_atomic_in_progress()
            || self.key_value_map().is_atomic_in_progress()
            || self.rejected_reason_map().is_atomic_in_progress()
            || self.mapping_update_map().is_atomic_in_progress()
            || self.staking_rewards_map().is_atomic_in_progress()
            || self.history_event_map().is_atomic_in_progress()
    }

    /// Checkpoints the atomic batch.
    fn atomic_checkpoint(&self) {
        self.committee_store().atomic_checkpoint();
        self.program_id_map().atomic_checkpoint();
        self.key_value_map().atomic_checkpoint();
        self.rejected_reason_map().atomic_checkpoint();
        self.mapping_update_map().atomic_checkpoint();
        self.staking_rewards_map().atomic_checkpoint();
        self.history_event_map().atomic_checkpoint();
    }

    /// Clears the latest atomic batch checkpoint.
    fn clear_latest_checkpoint(&self) {
        self.committee_store().clear_latest_checkpoint();
        self.program_id_map().clear_latest_checkpoint();
        self.key_value_map().clear_latest_checkpoint();
        self.rejected_reason_map().clear_latest_checkpoint();
        self.mapping_update_map().clear_latest_checkpoint();
        self.staking_rewards_map().clear_latest_checkpoint();
        self.history_event_map().clear_latest_checkpoint();
    }

    /// Rewinds the atomic batch to the previous checkpoint.
    fn atomic_rewind(&self) {
        self.committee_store().atomic_rewind();
        self.program_id_map().atomic_rewind();
        self.key_value_map().atomic_rewind();
        self.rejected_reason_map().atomic_rewind();
        self.mapping_update_map().atomic_rewind();
        self.staking_rewards_map().atomic_rewind();
        self.history_event_map().atomic_rewind();
    }

    /// Aborts an atomic batch write operation.
    fn abort_atomic(&self) {
        self.committee_store().abort_atomic();
        self.program_id_map().abort_atomic();
        self.key_value_map().abort_atomic();
        self.rejected_reason_map().abort_atomic();
        self.mapping_update_map().abort_atomic();
        self.staking_rewards_map().abort_atomic();
        self.history_event_map().abort_atomic();
    }

    /// Finishes an atomic batch write operation.
    fn finish_atomic(&self) -> Result<()> {
        self.committee_store().finish_atomic()?;
        self.program_id_map().finish_atomic()?;
        self.key_value_map().finish_atomic()?;
        self.rejected_reason_map().finish_atomic()?;
        self.mapping_update_map().finish_atomic()?;
        self.staking_rewards_map().finish_atomic()?;
        self.history_event_map().finish_atomic()?;
        Ok(())
    }

    /// Returns the current block height.
    fn current_block_height(&self) -> &AtomicU32;

    /// Appends `event` to the event log at the current block height.
    fn record_history_event(&self, event: HistoryEvent<N>) -> Result<()> {
        let height = self.current_block_height().load(Ordering::SeqCst);
        let seq = self.history_event_seq().fetch_add(1, Ordering::SeqCst);
        self.history_event_map().insert((height.to_be_bytes(), seq.to_be_bytes()), event)
    }

    /// Records one mapping history entry, as selected by [`Self::history_recording`].
    ///
    /// The write joins the caller's atomic batch, so a speculative finalize that aborts does not
    /// keep it.
    fn record_historical(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: Plaintext<N>,
        value: HistoricalMappingValue<N>,
    ) -> Result<()> {
        match HistoryRecording::load(self.history_recording()) {
            HistoryRecording::Off => Ok(()),
            HistoryRecording::Tables => {
                let height = self.current_block_height().load(Ordering::SeqCst);
                self.mapping_update_map().insert((program_id, mapping_name, key, height.to_be_bytes()), value)
            }
            HistoryRecording::Events => self.record_history_event(HistoryEvent::Mapping {
                program_id,
                mapping_name,
                key,
                value: Box::new(value),
            }),
        }
    }

    /// Records a deletion for every key currently in `mapping_name`.
    fn record_mapping_absences(&self, program_id: ProgramID<N>, mapping_name: Identifier<N>) -> Result<()> {
        if HistoryRecording::load(self.history_recording()) == HistoryRecording::Off {
            return Ok(());
        }
        let entries = self.key_value_map().get_map_speculative(&(program_id, mapping_name))?;
        for (key, _) in entries {
            self.record_historical(program_id, mapping_name, key, HistoricalMappingValue::Absent)?;
        }
        Ok(())
    }

    /// Writes the marker that this block's history was committed, when recording to the event log.
    ///
    /// The marker is the first event at the block height, inside the caller's atomic batch.
    fn record_history_block(&self) -> Result<()> {
        match HistoryRecording::load(self.history_recording()) {
            HistoryRecording::Events => self.record_history_event(HistoryEvent::Indexed),
            HistoryRecording::Off | HistoryRecording::Tables => Ok(()),
        }
    }

    /// Records one staking reward, as selected by [`Self::history_recording`].
    fn record_staking_reward(
        &self,
        staker: Address<N>,
        validator: Address<N>,
        reward: u64,
        new_stake: u64,
    ) -> Result<()> {
        match HistoryRecording::load(self.history_recording()) {
            HistoryRecording::Off => Ok(()),
            HistoryRecording::Tables => {
                let height = self.current_block_height().load(Ordering::SeqCst);
                self.staking_rewards_map().insert((staker, height), (validator, reward, new_stake))
            }
            HistoryRecording::Events => {
                self.record_history_event(HistoryEvent::Staking { staker, validator, reward, new_stake })
            }
        }
    }

    /// Initializes the given `program ID` and `mapping name` in storage.
    /// If the `mapping name` is already initialized, an error is returned.
    fn initialize_mapping(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
    ) -> Result<FinalizeOperation<N>> {
        // Retrieve the mapping names for the program ID. If the program ID does not exist, initialize the mapping names.
        let mut mapping_names =
            self.program_id_map().get_speculative(&program_id)?.map_or(Default::default(), |x| x.into_owned());

        // Ensure the mapping name does not already exist.
        if mapping_names.contains(&mapping_name) {
            bail!("Illegal operation: mapping name '{mapping_name}' already exists in storage - cannot re-initialize.")
        }

        // Insert the new mapping name.
        mapping_names.insert(mapping_name);

        atomic_batch_scope!(self, {
            // Update the program ID map with the new mapping name.
            self.program_id_map().insert(program_id, mapping_names)?;

            Ok(())
        })?;

        // Return the finalize operation.
        Ok(FinalizeOperation::InitializeMapping(to_mapping_id(&program_id, &mapping_name)?))
    }

    /// Stores the given `(key, value)` pair at the given `program ID` and `mapping name` in storage.
    /// If the `mapping name` is not initialized, an error is returned.
    /// If the `key` already exists, the method returns an error.
    fn insert_key_value(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: Plaintext<N>,
        value: Value<N>,
    ) -> Result<FinalizeOperation<N>> {
        // Ensure the mapping name exists.
        if !self.contains_mapping_speculative(&program_id, &mapping_name)? {
            bail!("Illegal operation: '{program_id}/{mapping_name}' is not initialized - cannot insert key-value.")
        }
        // Ensure the key-value does not already exist.
        if self.contains_key_speculative(program_id, mapping_name, &key)? {
            bail!(
                "Illegal operation: '{program_id}/{mapping_name}' key '{key}' already exists in storage - cannot insert key-value"
            );
        }

        // Compute the key ID.
        let key_id = to_key_id(&program_id, &mapping_name, &key)?;
        // Compute the value ID.
        let value_id = N::hash_bhp1024(&(key_id, N::hash_bhp1024(&value.to_bits_le())?).to_bits_le())?;

        atomic_batch_scope!(self, {
            self.record_historical(
                program_id,
                mapping_name,
                key.clone(),
                HistoricalMappingValue::Present(value.clone()),
            )?;

            // Update the key-value map with the new key-value.
            self.key_value_map().insert((program_id, mapping_name), key, value)?;

            Ok(())
        })?;

        // Return the finalize operation.
        Ok(FinalizeOperation::InsertKeyValue(to_mapping_id(&program_id, &mapping_name)?, key_id, value_id))
    }

    /// Stores the given `(key, value)` pair at the given `program ID` and `mapping name` in storage.
    /// If the `mapping name` is not initialized, an error is returned.
    /// If the `key` does not exist, the `(key, value)` pair is initialized.
    /// If the `key` already exists, the `value` is overwritten.
    fn update_key_value(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: Plaintext<N>,
        value: Value<N>,
    ) -> Result<FinalizeOperation<N>> {
        // Ensure the mapping name exists.
        if !self.contains_mapping_speculative(&program_id, &mapping_name)? {
            bail!("Illegal operation: '{program_id}/{mapping_name}' is not initialized - cannot update key-value.")
        }

        // Compute the key ID.
        let key_id = to_key_id(&program_id, &mapping_name, &key)?;
        // Compute the value ID.
        let value_id = N::hash_bhp1024(&(key_id, N::hash_bhp1024(&value.to_bits_le())?).to_bits_le())?;

        atomic_batch_scope!(self, {
            self.record_historical(
                program_id,
                mapping_name,
                key.clone(),
                HistoricalMappingValue::Present(value.clone()),
            )?;

            // Update the key-value map with the new key-value.
            self.key_value_map().insert((program_id, mapping_name), key, value)?;

            Ok(())
        })?;

        // Return the finalize operation.
        Ok(FinalizeOperation::UpdateKeyValue(to_mapping_id(&program_id, &mapping_name)?, key_id, value_id))
    }

    /// Removes the key-value pair for the given `program ID`, `mapping name`, and `key` from storage.
    /// If the `key` does not exist, `None` is returned.
    fn remove_key_value(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<Option<FinalizeOperation<N>>> {
        // Ensure the mapping name exists.
        if !self.contains_mapping_speculative(&program_id, &mapping_name)? {
            bail!("Illegal operation: '{program_id}/{mapping_name}' is not initialized - cannot remove key-value.")
        }
        // Ensure the key-value entry exists.
        if !self.contains_key_speculative(program_id, mapping_name, key)? {
            return Ok(None);
        }

        // Compute the key ID.
        let key_id = to_key_id(&program_id, &mapping_name, key)?;

        atomic_batch_scope!(self, {
            self.record_historical(program_id, mapping_name, key.clone(), HistoricalMappingValue::Absent)?;
            // Update the key-value map with the new key.
            self.key_value_map().remove_key(&(program_id, mapping_name), key)?;

            Ok(())
        })?;

        // Return the finalize operation.
        Ok(Some(FinalizeOperation::RemoveKeyValue(to_mapping_id(&program_id, &mapping_name)?, key_id)))
    }

    /// Replaces the mapping for the given `program ID` and `mapping name` from storage,
    /// with the given `key-value` pairs.
    fn replace_mapping(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        entries: Vec<(Plaintext<N>, Value<N>)>,
    ) -> Result<FinalizeOperation<N>> {
        // Ensure the mapping name exists.
        if !self.contains_mapping_speculative(&program_id, &mapping_name)? {
            bail!("Illegal operation: '{program_id}/{mapping_name}' is not initialized - cannot replace mapping.")
        }

        atomic_batch_scope!(self, {
            // Values before the replacement, read only when history is recorded.
            let mut old_entries: IndexMap<Plaintext<N>, Value<N>> =
                match HistoryRecording::load(self.history_recording()) {
                    HistoryRecording::Off => IndexMap::new(),
                    HistoryRecording::Tables | HistoryRecording::Events => {
                        self.key_value_map().get_map_speculative(&(program_id, mapping_name))?.into_iter().collect()
                    }
                };

            // Remove the existing key-value entries.
            self.key_value_map().remove_map(&(program_id, mapping_name))?;

            // Insert the new key-value entries.
            for (key, value) in entries {
                // A value that did not change keeps its earlier record, which floor reads return.
                if old_entries.swap_remove(&key).as_ref() != Some(&value) {
                    self.record_historical(
                        program_id,
                        mapping_name,
                        key.clone(),
                        HistoricalMappingValue::Present(value.clone()),
                    )?;
                }

                // Insert the key-value entry.
                self.key_value_map().insert((program_id, mapping_name), key, value)?;
            }

            // Keys dropped by the replacement stay absent at this height.
            for key in old_entries.into_keys() {
                self.record_historical(program_id, mapping_name, key, HistoricalMappingValue::Absent)?;
            }

            Ok(())
        })?;

        // Return the finalize operation.
        Ok(FinalizeOperation::ReplaceMapping(to_mapping_id(&program_id, &mapping_name)?))
    }

    /// Removes the mapping for the given `program ID` and `mapping name` from storage,
    /// along with all associated key-value pairs in storage.
    fn remove_mapping(&self, program_id: ProgramID<N>, mapping_name: Identifier<N>) -> Result<FinalizeOperation<N>> {
        // Retrieve the mapping names.
        let Some(mut mapping_names) = self.program_id_map().get_speculative(&program_id)?.map(|x| x.into_owned())
        else {
            bail!("Illegal operation: program ID '{program_id}' is not initialized - cannot remove mapping.");
        };
        // Remove the mapping name.
        if !mapping_names.shift_remove(&mapping_name) {
            bail!("Illegal operation: mapping '{mapping_name}' does not exist in storage - cannot remove mapping.");
        }

        atomic_batch_scope!(self, {
            self.record_mapping_absences(program_id, mapping_name)?;
            // Update the mapping names.
            self.program_id_map().insert(program_id, mapping_names)?;
            // Remove the mapping.
            self.key_value_map().remove_map(&(program_id, mapping_name))?;

            Ok(())
        })?;

        // Return the finalize operation.
        Ok(FinalizeOperation::RemoveMapping(to_mapping_id(&program_id, &mapping_name)?))
    }

    /// Removes the program for the given `program ID` from storage,
    /// along with all associated mappings and key-value pairs in storage.
    fn remove_program(&self, program_id: &ProgramID<N>) -> Result<()> {
        // Retrieve the mapping names.
        let Some(mapping_names) = self.program_id_map().get_speculative(program_id)? else {
            bail!("Illegal operation: program ID '{program_id}' is not initialized - cannot remove mapping.")
        };

        atomic_batch_scope!(self, {
            // Update the mapping names.
            self.program_id_map().remove(program_id)?;

            // Remove each mapping.
            for mapping_name in mapping_names.iter() {
                self.record_mapping_absences(*program_id, *mapping_name)?;
                // Remove the mapping.
                self.key_value_map().remove_map(&(*program_id, *mapping_name))?;
            }
            Ok(())
        })
    }

    /// Returns `true` if the given `program ID` exist.
    fn contains_program_confirmed(&self, program_id: &ProgramID<N>) -> Result<bool> {
        self.program_id_map().contains_key_confirmed(program_id)
    }

    /// Returns `true` if the given `program ID` and `mapping name` exist.
    fn contains_mapping_confirmed(&self, program_id: &ProgramID<N>, mapping_name: &Identifier<N>) -> Result<bool> {
        Ok(self.program_id_map().get_confirmed(program_id)?.is_some_and(|m| m.contains(mapping_name)))
    }

    /// Returns `true` if the given `program ID` and `mapping name` exist.
    fn contains_mapping_speculative(&self, program_id: &ProgramID<N>, mapping_name: &Identifier<N>) -> Result<bool> {
        Ok(self.program_id_map().get_speculative(program_id)?.is_some_and(|m| m.contains(mapping_name)))
    }

    /// Returns `true` if the given `program ID`, `mapping name`, and `key` exist.
    fn contains_key_confirmed(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<bool> {
        self.key_value_map().contains_key_confirmed(&(program_id, mapping_name), key)
    }

    /// Returns `true` if the given `program ID`, `mapping name`, and `key` exist.
    fn contains_key_speculative(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<bool> {
        self.key_value_map().contains_key_speculative(&(program_id, mapping_name), key)
    }

    /// Returns the confirmed mapping names for the given `program ID`.
    fn get_mapping_names_confirmed(&self, program_id: &ProgramID<N>) -> Result<Option<IndexSet<Identifier<N>>>> {
        Ok(self.program_id_map().get_confirmed(program_id)?.map(|names| names.into_owned()))
    }

    /// Returns the speculative mapping names for the given `program ID`.
    fn get_mapping_names_speculative(&self, program_id: &ProgramID<N>) -> Result<Option<IndexSet<Identifier<N>>>> {
        Ok(self.program_id_map().get_speculative(program_id)?.map(|names| names.into_owned()))
    }

    /// Returns the confirmed mapping entries for the given `program ID` and `mapping name`.
    fn get_mapping_confirmed(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
    ) -> Result<Vec<(Plaintext<N>, Value<N>)>> {
        // Ensure the mapping name exists.
        if !self.contains_mapping_confirmed(&program_id, &mapping_name)? {
            bail!("Illegal operation: '{program_id}/{mapping_name}' is not initialized - cannot get mapping (C).")
        }
        // Retrieve the key-values for the mapping.
        self.key_value_map().get_map_confirmed(&(program_id, mapping_name))
    }

    /// Returns the speculative mapping entries for the given `program ID` and `mapping name`.
    fn get_mapping_speculative(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
    ) -> Result<Vec<(Plaintext<N>, Value<N>)>> {
        // Ensure the mapping name exists.
        if !self.contains_mapping_speculative(&program_id, &mapping_name)? {
            bail!("Illegal operation: '{program_id}/{mapping_name}' is not initialized - cannot get mapping (S).")
        }
        // Retrieve the key-values for the mapping.
        self.key_value_map().get_map_speculative(&(program_id, mapping_name))
    }

    /// Returns the confirmed value for the given `program ID`, `mapping name`, and `key`.
    fn get_value_confirmed(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<Option<Value<N>>> {
        Ok(self.key_value_map().get_value_confirmed(&(program_id, mapping_name), key)?.map(|x| x.into_owned()))
    }

    /// Returns the speculative value for the given `program ID`, `mapping name`, and `key`.
    fn get_value_speculative(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<Option<Value<N>>> {
        Ok(self.key_value_map().get_value_speculative(&(program_id, mapping_name), key)?.map(|x| x.into_owned()))
    }

    /// Returns the confirmed checksum of the finalize storage.
    fn get_checksum_confirmed(&self) -> Result<Field<N>> {
        // Compute all mapping checksums.
        let preimage: std::collections::BTreeMap<_, _> = self
            .key_value_map()
            .iter_confirmed()
            .map(|(m, k, v)| {
                let m = *m;
                let k = k.into_owned();
                let v = v.into_owned();

                let mut preimage = Vec::new();
                m.write_bits_le(&mut preimage);
                false.write_bits_le(&mut preimage); // Separator.
                k.write_bits_le(&mut preimage);
                false.write_bits_le(&mut preimage); // Separator.

                // Compute the mapping checksum as `Hash( m || k )`.
                let mapping_checksum = N::hash_bhp1024(&preimage)?;

                v.write_bits_le(&mut preimage);
                false.write_bits_le(&mut preimage); // Separator.

                // Compute the entry checksum as `Hash( m || k || v )`.
                let entry_checksum = N::hash_bhp1024(&preimage)?;
                // Return the mapping checksum and entry checksum.
                Ok::<_, Error>((mapping_checksum, entry_checksum.to_bits_le()))
            })
            .try_collect()?;
        // Compute the checksum as `Hash( all mapping checksums )`.
        N::hash_bhp1024(&preimage.into_values().flatten().collect::<Vec<_>>())
    }

    /// Returns the pending checksum of the finalize storage.
    fn get_checksum_pending(&self) -> Result<Field<N>> {
        // Compute all mapping checksums.
        let preimage: std::collections::BTreeMap<_, _> = self
            .key_value_map()
            .iter_pending()
            .map(|(m, k, v)| {
                let m = *m;

                let mut preimage = Vec::new();
                m.write_bits_le(&mut preimage);
                false.write_bits_le(&mut preimage); // Separator.
                if let Some(k) = k {
                    k.into_owned().write_bits_le(&mut preimage);
                }
                false.write_bits_le(&mut preimage); // Separator.

                // Compute the mapping checksum as `Hash( m || k )`.
                let mapping_checksum = N::hash_bhp1024(&preimage)?;

                if let Some(v) = v {
                    v.into_owned().write_bits_le(&mut preimage);
                }
                false.write_bits_le(&mut preimage); // Separator.

                // Compute the entry checksum as `Hash( m || k || v )`.
                let entry_checksum = N::hash_bhp1024(&preimage)?;
                // Return the mapping checksum and entry checksum.
                Ok::<_, Error>((mapping_checksum, entry_checksum.to_bits_le()))
            })
            .try_collect()?;
        // Compute the checksum as `Hash( all mapping checksums )`.
        N::hash_bhp1024(&preimage.into_values().flatten().collect::<Vec<_>>())
    }
}

/// The finalize store.
#[derive(Clone)]
pub struct FinalizeStore<N: Network, P: FinalizeStorage<N>> {
    /// The finalize storage.
    storage: P,
    /// PhantomData.
    _phantom: PhantomData<N>,
    /// Tracks the current block height.
    /// Updated by the VM at the start of each canonical finalize
    block_height: Arc<AtomicU32>,
}

impl<N: Network, P: FinalizeStorage<N>> FinalizeStore<N, P> {
    /// Initializes the finalize store.
    pub fn open<S: Into<StorageMode>>(storage: S) -> Result<Self> {
        Self::from(P::open(storage)?)
    }

    /// Initializes a finalize store from storage.
    pub fn from(storage: P) -> Result<Self> {
        // Return the finalize store.
        Ok(Self { storage, _phantom: PhantomData, block_height: Arc::new(AtomicU32::new(0)) })
    }

    /// Starts an atomic batch write operation.
    pub fn start_atomic(&self) {
        self.storage.start_atomic();
    }

    /// Checks if an atomic batch is in progress.
    pub fn is_atomic_in_progress(&self) -> bool {
        self.storage.is_atomic_in_progress()
    }

    /// Checkpoints the atomic batch.
    pub fn atomic_checkpoint(&self) {
        self.storage.atomic_checkpoint();
    }

    /// Clears the latest atomic batch checkpoint.
    pub fn clear_latest_checkpoint(&self) {
        self.storage.clear_latest_checkpoint();
    }

    /// Rewinds the atomic batch to the previous checkpoint.
    pub fn atomic_rewind(&self) {
        self.storage.atomic_rewind();
    }

    /// Aborts an atomic batch write operation.
    pub fn abort_atomic(&self) {
        self.storage.abort_atomic();
    }

    /// Finishes an atomic batch write operation.
    pub fn finish_atomic(&self) -> Result<()> {
        self.storage.finish_atomic()
    }

    /// Returns the storage mode.
    pub fn storage_mode(&self) -> &StorageMode {
        self.storage.storage_mode()
    }

    /// Returns the rejection reason map.
    pub fn rejected_reason_map(&self) -> &P::RejectedReasonMap {
        self.storage.rejected_reason_map()
    }

    /// Returns the current block height.
    pub fn current_block_height(&self) -> &AtomicU32 {
        self.storage.current_block_height()
    }

    /// Enables or disables history recording into this store's history tables.
    pub fn set_record_history(&self, enabled: bool) {
        self.set_history_recording(if enabled { HistoryRecording::Tables } else { HistoryRecording::Off });
    }

    /// Sets where mapping updates and staking rewards are recorded.
    ///
    /// Recording is off unless a caller turns it on. Canonical finalize and speculative finalize
    /// share this setting; speculative batches abort, so only a committed block keeps the records.
    pub fn set_history_recording(&self, recording: HistoryRecording) {
        self.storage.history_recording().store(recording as u8, Ordering::SeqCst);
    }

    /// Returns where mapping updates and staking rewards are recorded.
    pub fn history_recording(&self) -> HistoryRecording {
        HistoryRecording::load(self.storage.history_recording())
    }

    /// Returns whether history is recorded into this store's history tables.
    pub fn record_history(&self) -> bool {
        self.history_recording() == HistoryRecording::Tables
    }

    /// Returns the next block height history indexing will process.
    pub fn history_synced_height(&self) -> u32 {
        self.storage.history_synced_height()
    }

    /// Stores the next block height history indexing will process.
    pub fn set_history_synced_height(&self, height: u32) -> Result<()> {
        self.storage.set_history_synced_height(height)
    }

    /// Resets the per-block history event sequence to zero.
    pub fn reset_history_event_seq(&self) {
        self.storage.history_event_seq().store(0, Ordering::SeqCst);
    }

    /// Writes the marker that this block's history was committed, when recording to the event log.
    pub fn record_history_block(&self) -> Result<()> {
        self.storage.record_history_block()
    }

    /// Records a staking reward when history recording is enabled.
    pub fn record_staking_reward(
        &self,
        staker: Address<N>,
        validator: Address<N>,
        reward: u64,
        new_stake: u64,
    ) -> Result<()> {
        self.storage.record_staking_reward(staker, validator, reward, new_stake)
    }

    /// Returns the current block height.
    pub fn block_height(&self) -> &AtomicU32 {
        &self.block_height
    }

    /// Returns the historical value of a mapping at or before the given block height.
    ///
    /// The lookup is a floor seek on `mapping_update_map`. A deletion (`Absent`) at or before
    /// `height` means the key has no value. `height` must be strictly below
    /// [`Self::history_synced_height`]; a later height is not indexed yet.
    pub fn get_historical_mapping_value(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        mapping_key: Plaintext<N>,
        height: u32,
    ) -> Result<Option<Cow<'_, Value<N>>>, Error> {
        let synced = self.history_synced_height();
        if height >= synced {
            bail!("Block {height} is not in the history index (history is indexed before height {synced})");
        }

        let seek_key = (program_id, mapping_name, mapping_key.clone(), height.to_be_bytes());
        match self.storage.mapping_update_map().get_floor_confirmed(&seek_key)? {
            Some((found_key, found_value)) => {
                let (p, m, k, _h) = found_key.into_owned();
                if p == program_id && m == mapping_name && k == mapping_key {
                    match found_value.into_owned() {
                        HistoricalMappingValue::Present(value) => Ok(Some(Cow::Owned(value))),
                        HistoricalMappingValue::Absent => Ok(None),
                    }
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Returns the heights at which past mapping updates occurred, in ascending order.
    ///
    /// Deletions are included. This scans the history map and is intended for tests.
    pub fn get_mapping_update_heights(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        mapping_key: Plaintext<N>,
    ) -> Result<Option<Cow<'_, Vec<u32>>>, Error> {
        let mut heights: Vec<u32> = self
            .storage
            .mapping_update_map()
            .iter_confirmed()
            .filter_map(|(k, _v)| {
                let (p, m, key, h_be) = k.into_owned();
                if p == program_id && m == mapping_name && key == mapping_key {
                    Some(u32::from_be_bytes(h_be))
                } else {
                    None
                }
            })
            .collect();

        if heights.is_empty() {
            return Ok(None);
        }

        heights.sort_unstable();
        Ok(Some(Cow::Owned(heights)))
    }

    /// Returns the history events written for `height`, in sequence order.
    pub fn history_events(&self, height: u32) -> Result<Vec<HistoryEvent<N>>> {
        let mut events = Vec::new();
        let mut seq = 0u32;
        while let Some(event) =
            self.storage.history_event_map().get_confirmed(&(height.to_be_bytes(), seq.to_be_bytes()))?
        {
            events.push(event.into_owned());
            seq = seq.saturating_add(1);
        }
        Ok(events)
    }

    /// Writes the history events of each given height into this store's history tables, in one
    /// write batch.
    ///
    /// Used to copy blocks' history from the replay into the ledger that serves reads. Existing
    /// records at the same keys are overwritten.
    pub fn import_history_events(&self, blocks: Vec<(u32, Vec<HistoryEvent<N>>)>) -> Result<()> {
        atomic_batch_scope!(self.storage, {
            for (height, events) in blocks {
                for event in events {
                    match event {
                        HistoryEvent::Indexed => {}
                        HistoryEvent::Mapping { program_id, mapping_name, key, value } => {
                            self.storage
                                .mapping_update_map()
                                .insert((program_id, mapping_name, key, height.to_be_bytes()), *value)?;
                        }
                        HistoryEvent::Staking { staker, validator, reward, new_stake } => {
                            self.storage
                                .staking_rewards_map()
                                .insert((staker, height), (validator, reward, new_stake))?;
                        }
                    }
                }
            }
            Ok(())
        })
    }

    /// Deletes the history events of every height below `height`.
    ///
    /// The deletion is outside any atomic batch; call it while no batch on this store is open.
    pub fn prune_history_events_below(&self, height: u32) -> Result<()> {
        self.storage.prune_history_events_below(height)
    }

    /// Returns the historical staking rewards map.
    pub fn staking_rewards_map(&self) -> &P::StakingRewardsMap {
        self.storage.staking_rewards_map()
    }
}

impl<N: Network, P: FinalizeStorage<N>> FinalizeStore<N, P> {
    /// Returns the committee store.
    pub fn committee_store(&self) -> &CommitteeStore<N, P::CommitteeStorage> {
        self.storage.committee_store()
    }
}

impl<N: Network, P: FinalizeStorage<N>> FinalizeStoreTrait<N> for FinalizeStore<N, P> {
    /// Returns `true` if the given `program ID` and `mapping name` is confirmed to exist.
    fn contains_mapping_confirmed(&self, program_id: &ProgramID<N>, mapping_name: &Identifier<N>) -> Result<bool> {
        self.storage.contains_mapping_confirmed(program_id, mapping_name)
    }

    /// Returns `true` if the given `program ID` and `mapping name` exist.
    fn contains_mapping_speculative(&self, program_id: &ProgramID<N>, mapping_name: &Identifier<N>) -> Result<bool> {
        self.storage.contains_mapping_speculative(program_id, mapping_name)
    }

    /// Returns `true` if the given `program ID`, `mapping name`, and `key` exist.
    fn contains_key_speculative(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<bool> {
        self.storage.contains_key_speculative(program_id, mapping_name, key)
    }

    /// Returns the speculative value for the given `program ID`, `mapping name`, and `key`.
    fn get_value_speculative(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<Option<Value<N>>> {
        self.storage.get_value_speculative(program_id, mapping_name, key)
    }

    /// Stores the given `(key, value)` pair at the given `program ID` and `mapping name` in storage.
    /// If the `mapping name` is not initialized, an error is returned.
    /// If the `key` already exists, the method returns an error.
    fn insert_key_value(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: Plaintext<N>,
        value: Value<N>,
    ) -> Result<FinalizeOperation<N>> {
        self.storage.insert_key_value(program_id, mapping_name, key, value)
    }

    /// Stores the given `(key, value)` pair at the given `program ID` and `mapping name` in storage.
    /// If the `mapping name` is not initialized, an error is returned.
    /// If the `key` does not exist, the `(key, value)` pair is initialized.
    /// If the `key` already exists, the `value` is overwritten.
    fn update_key_value(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: Plaintext<N>,
        value: Value<N>,
    ) -> Result<FinalizeOperation<N>> {
        self.storage.update_key_value(program_id, mapping_name, key, value)
    }

    /// Removes the key-value pair for the given `program ID`, `mapping name`, and `key` from storage.
    fn remove_key_value(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<Option<FinalizeOperation<N>>> {
        self.storage.remove_key_value(program_id, mapping_name, key)
    }
}

impl<N: Network, P: FinalizeStorage<N>> FinalizeStore<N, P> {
    /// Initializes the given `program ID` and `mapping name` in storage.
    /// If the `mapping name` is already initialized, an error is returned.
    pub fn initialize_mapping(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
    ) -> Result<FinalizeOperation<N>> {
        self.storage.initialize_mapping(program_id, mapping_name)
    }

    /// Replaces the mapping for the given `program ID` and `mapping name` from storage,
    /// with the given `key-value` pairs.
    pub fn replace_mapping(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        entries: Vec<(Plaintext<N>, Value<N>)>,
    ) -> Result<FinalizeOperation<N>> {
        self.storage.replace_mapping(program_id, mapping_name, entries)
    }

    /// Removes the mapping for the given `program ID` and `mapping name` from storage,
    /// along with all associated key-value pairs in storage.
    pub fn remove_mapping(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
    ) -> Result<FinalizeOperation<N>> {
        self.storage.remove_mapping(program_id, mapping_name)
    }

    /// Removes the program for the given `program ID` from storage,
    /// along with all associated mappings and key-value pairs in storage.
    pub fn remove_program(&self, program_id: &ProgramID<N>) -> Result<()> {
        self.storage.remove_program(program_id)
    }
}

impl<N: Network, P: FinalizeStorage<N>> FinalizeStore<N, P> {
    /// Returns `true` if the given `program ID` exist.
    pub fn contains_program_confirmed(&self, program_id: &ProgramID<N>) -> Result<bool> {
        self.storage.contains_program_confirmed(program_id)
    }

    /// Returns `true` if the given `program ID`, `mapping name`, and `key` exist.
    pub fn contains_key_confirmed(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<bool> {
        self.storage.contains_key_confirmed(program_id, mapping_name, key)
    }
}

impl<N: Network, P: FinalizeStorage<N>> FinalizeStore<N, P> {
    /// Returns the confirmed mapping names for the given `program ID`.
    pub fn get_mapping_names_confirmed(&self, program_id: &ProgramID<N>) -> Result<Option<IndexSet<Identifier<N>>>> {
        self.storage.get_mapping_names_confirmed(program_id)
    }

    /// Returns the confirmed mapping entries for the given `program ID` and `mapping name`.
    pub fn get_mapping_confirmed(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
    ) -> Result<Vec<(Plaintext<N>, Value<N>)>> {
        self.storage.get_mapping_confirmed(program_id, mapping_name)
    }

    /// Returns the speculative mapping entries for the given `program ID` and `mapping name`.
    pub fn get_mapping_speculative(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
    ) -> Result<Vec<(Plaintext<N>, Value<N>)>> {
        self.storage.get_mapping_speculative(program_id, mapping_name)
    }

    /// Returns the confirmed value for the given `program ID`, `mapping name`, and `key`.
    pub fn get_value_confirmed(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<Option<Value<N>>> {
        self.storage.get_value_confirmed(program_id, mapping_name, key)
    }

    /// Returns the speculative value for the given `program ID`, `mapping name`, and `key`.
    pub fn get_value_speculative(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<Option<Value<N>>> {
        self.storage.get_value_speculative(program_id, mapping_name, key)
    }

    /// Returns the confirmed checksum of the finalize store.
    pub fn get_checksum_confirmed(&self) -> Result<Field<N>> {
        self.storage.get_checksum_confirmed()
    }
}

impl<N: Network, P: FinalizeStorage<N>> FinalizeStore<N, P> {
    /// Stores the rejection reason for the given transaction ID.
    pub fn insert_rejected_reason(&self, transaction_id: Field<N>, reason: RejectedReason<N>) -> Result<()> {
        let height = self.block_height.load(std::sync::atomic::Ordering::SeqCst);
        let consensus_version = N::CONSENSUS_VERSION(height)?;
        if cfg!(feature = "test") || consensus_version >= ConsensusVersion::V15 {
            self.storage.rejected_reason_map().insert(transaction_id, reason)
        } else {
            Ok(())
        }
    }

    /// Returns the rejection reason for the given transaction ID.
    pub fn get_rejected_reason(&self, transaction_id: &Field<N>) -> Result<Option<RejectedReason<N>>> {
        match self.storage.rejected_reason_map().get_speculative(transaction_id)? {
            Some(reason) => Ok(Some(reason.into_owned())),
            None => Ok(None),
        }
    }

    /// Returns `true` if a rejection reason exists for the given transaction ID.
    pub fn contains_rejected_reason(&self, transaction_id: &Field<N>) -> Result<bool> {
        self.storage.rejected_reason_map().contains_key_speculative(transaction_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::memory::FinalizeMemory;
    use console::network::MainnetV0;

    use aleo_std::StorageMode;

    use console::{program::Literal, types::U64};

    type CurrentNetwork = MainnetV0;

    /// Checks `initialize_mapping`, `insert_key_value`, `remove_key_value`, and `remove_mapping`.
    fn check_initialize_insert_remove<N: Network>(
        finalize_store: &FinalizeStore<N, FinalizeMemory<N>>,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
    ) {
        // Prepare a key and value.
        let key = Plaintext::from_str("123456789field").unwrap();
        let value = Value::from_str("987654321u128").unwrap();

        // Ensure the program ID does not exist.
        assert!(!finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name does not exist.
        assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure removing an un-initialized mapping fails.
        assert!(finalize_store.remove_mapping(program_id, mapping_name).is_err());

        // Now, initialize the mapping.
        finalize_store.initialize_mapping(program_id, mapping_name).unwrap();
        // Ensure the program ID got initialized.
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name got initialized.
        assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure the key did not get initialized.
        assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        // Ensure the value returns None.
        assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());

        // Insert a (key, value) pair.
        finalize_store.insert_key_value(program_id, mapping_name, key.clone(), value.clone()).unwrap();
        // Ensure the program ID is still initialized.
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name is still initialized.
        assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure the key got initialized.
        assert!(finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        // Ensure the value returns Some(value).
        assert_eq!(value, finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().unwrap());

        // Ensure removing the key succeeds.
        assert!(finalize_store.remove_key_value(program_id, mapping_name, &key).unwrap().is_some());
        // Ensure the program ID is still initialized.
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name is still initialized.
        assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure the key got removed.
        assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        // Ensure the value returns None.
        assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());

        // Ensure removing the mapping succeeds.
        finalize_store.remove_mapping(program_id, mapping_name).unwrap();
        // Ensure the program ID is still initialized.
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name is no longer initialized.
        assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure the key is still removed.
        assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        // Ensure the value still returns None.
        assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());

        // Ensure removing the program succeeds.
        finalize_store.remove_program(&program_id).unwrap();
        // Ensure the program ID is no longer initialized.
        assert!(!finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name is still no longer initialized.
        assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure the key is still removed.
        assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        // Ensure the value still returns None.
        assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());
    }

    /// Checks `initialize_mapping`, `update_key_value`, `remove_key_value`, and `remove_mapping`.
    fn check_initialize_update_remove<N: Network>(
        finalize_store: &FinalizeStore<N, FinalizeMemory<N>>,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
    ) {
        // Prepare a key and value.
        let key = Plaintext::from_str("123456789field").unwrap();
        let value = Value::from_str("987654321u128").unwrap();

        // Ensure the program ID does not exist.
        assert!(!finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name does not exist.
        assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure removing an un-initialized mapping fails.
        assert!(finalize_store.remove_mapping(program_id, mapping_name).is_err());

        // Now, initialize the mapping.
        finalize_store.initialize_mapping(program_id, mapping_name).unwrap();
        // Ensure the program ID got initialized.
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name got initialized.
        assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure the key did not get initialized.
        assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        // Ensure the value returns None.
        assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());

        // Update a (key, value) pair.
        finalize_store.update_key_value(program_id, mapping_name, key.clone(), value.clone()).unwrap();
        // Ensure the program ID is still initialized.
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name is still initialized.
        assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure the key got initialized.
        assert!(finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        // Ensure the value returns Some(value).
        assert_eq!(value, finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().unwrap());

        // Ensure calling `insert_key_value` with the same key and value fails.
        assert!(finalize_store.insert_key_value(program_id, mapping_name, key.clone(), value.clone()).is_err());
        // Ensure the key is still initialized.
        assert!(finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        // Ensure the value still returns Some(value).
        assert_eq!(value, finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().unwrap());

        // Ensure calling `update_key_value` with the same key and value succeeds.
        finalize_store.update_key_value(program_id, mapping_name, key.clone(), value.clone()).unwrap();
        // Ensure the key is still initialized.
        assert!(finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        // Ensure the value still returns Some(value).
        assert_eq!(value, finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().unwrap());

        {
            // Prepare the same key and different value.
            let new_value = Value::from_str("123456789u128").unwrap();

            // Ensure calling `insert_key_value` with a different key and value fails.
            assert!(finalize_store.insert_key_value(program_id, mapping_name, key.clone(), new_value.clone()).is_err());
            // Ensure the key is still initialized.
            assert!(finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value still returns Some(value).
            assert_eq!(value, finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().unwrap());

            // Ensure calling `update_key_value` with a different key and value succeeds.
            finalize_store.update_key_value(program_id, mapping_name, key.clone(), new_value.clone()).unwrap();
            // Ensure the key is still initialized.
            assert!(finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns Some(new_value).
            assert_eq!(
                new_value,
                finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().unwrap()
            );

            // Ensure calling `update_key_value` with the same key and original value succeeds.
            finalize_store.update_key_value(program_id, mapping_name, key.clone(), value.clone()).unwrap();
            // Ensure the key is still initialized.
            assert!(finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns Some(value).
            assert_eq!(value, finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().unwrap());
        }

        // Ensure removing the key succeeds.
        assert!(finalize_store.remove_key_value(program_id, mapping_name, &key).unwrap().is_some());
        // Ensure the program ID is still initialized.
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name is still initialized.
        assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure the key got removed.
        assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        // Ensure the value returns None.
        assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());

        // Ensure removing the mapping succeeds.
        finalize_store.remove_mapping(program_id, mapping_name).unwrap();
        // Ensure the program ID is still initialized.
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name is no longer initialized.
        assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure the key is still removed.
        assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        // Ensure the value still returns None.
        assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());

        // Ensure removing the program succeeds.
        finalize_store.remove_program(&program_id).unwrap();
        // Ensure the program ID is no longer initialized.
        assert!(!finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name is still no longer initialized.
        assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure the key is still removed.
        assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        // Ensure the value still returns None.
        assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());
    }

    #[test]
    fn test_initialize_insert_remove() {
        // Initialize a program ID and mapping name.
        let program_id = ProgramID::<CurrentNetwork>::from_str("hello.aleo").unwrap();
        let mapping_name = Identifier::from_str("account").unwrap();

        // Initialize a new finalize store.
        let program_memory = FinalizeMemory::open(StorageMode::Test(None)).unwrap();
        let finalize_store = FinalizeStore::from(program_memory).unwrap();
        // Check the operations.
        check_initialize_insert_remove(&finalize_store, program_id, mapping_name);
    }

    #[test]
    fn test_initialize_update_remove() {
        // Initialize a program ID and mapping name.
        let program_id = ProgramID::<CurrentNetwork>::from_str("hello.aleo").unwrap();
        let mapping_name = Identifier::from_str("account").unwrap();

        // Initialize a new finalize store.
        let program_memory = FinalizeMemory::open(StorageMode::Test(None)).unwrap();
        let finalize_store = FinalizeStore::from(program_memory).unwrap();
        // Check the operations.
        check_initialize_update_remove(&finalize_store, program_id, mapping_name);
    }

    #[test]
    fn test_remove_key_value() {
        // Initialize a program ID and mapping name.
        let program_id = ProgramID::<CurrentNetwork>::from_str("hello.aleo").unwrap();
        let mapping_name = Identifier::from_str("account").unwrap();

        // Initialize a new finalize store.
        let program_memory = FinalizeMemory::open(StorageMode::Test(None)).unwrap();
        let finalize_store = FinalizeStore::from(program_memory).unwrap();
        // Ensure the program ID does not exist.
        assert!(!finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name does not exist.
        assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure removing an un-initialized mapping fails.
        assert!(finalize_store.remove_mapping(program_id, mapping_name).is_err());

        // Now, initialize the mapping.
        finalize_store.initialize_mapping(program_id, mapping_name).unwrap();
        // Ensure the program ID got initialized.
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name got initialized.
        assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());

        // Attempt to remove a key-value pairs that do not exist.
        for item in 0..1000 {
            // Prepare the key.
            let key = Plaintext::from_str(&format!("{item}field")).unwrap();
            // Ensure the key did not get initialized.
            assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns None.
            assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());

            // Remove the key-value pair.
            assert!(finalize_store.remove_key_value(program_id, mapping_name, &key).unwrap().is_none());
            // Ensure the program ID is still initialized.
            assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
            // Ensure the mapping name is still initialized.
            assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
            // Ensure the key did not get initialized.
            assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns None.
            assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());
        }

        // Insert the list of keys and values.
        for item in 0..1000 {
            // Prepare the key and value.
            let key = Plaintext::from_str(&format!("{item}field")).unwrap();
            let value = Value::from_str(&format!("{item}u64")).unwrap();
            // Ensure the key did not get initialized.
            assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns None.
            assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());

            // Insert the key and value.
            finalize_store.insert_key_value(program_id, mapping_name, key.clone(), value.clone()).unwrap();
            // Ensure the program ID is still initialized.
            assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
            // Ensure the mapping name is still initialized.
            assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
            // Ensure the key got initialized.
            assert!(finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns Some(value).
            assert_eq!(value, finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().unwrap());
        }

        // Remove the list of keys and values.
        for item in 0..1000 {
            // Prepare the key and value.
            let key = Plaintext::from_str(&format!("{item}field")).unwrap();
            let value = Value::from_str(&format!("{item}u64")).unwrap();
            // Ensure the key is still initialized.
            assert!(finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns Some(value).
            assert_eq!(value, finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().unwrap());

            // Remove the key-value pair.
            assert!(finalize_store.remove_key_value(program_id, mapping_name, &key).unwrap().is_some());
            // Ensure the program ID is still initialized.
            assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
            // Ensure the mapping name is still initialized.
            assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
            // Ensure the key is no longer initialized.
            assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns None.
            assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());
        }
    }

    #[test]
    fn test_remove_mapping() {
        // Initialize a program ID and mapping name.
        let program_id = ProgramID::<CurrentNetwork>::from_str("hello.aleo").unwrap();
        let mapping_name = Identifier::from_str("account").unwrap();

        // Initialize a new finalize store.
        let program_memory = FinalizeMemory::open(StorageMode::Test(None)).unwrap();
        let finalize_store = FinalizeStore::from(program_memory).unwrap();
        // Ensure the program ID does not exist.
        assert!(!finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name does not exist.
        assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure removing an un-initialized mapping fails.
        assert!(finalize_store.remove_mapping(program_id, mapping_name).is_err());

        // Now, initialize the mapping.
        finalize_store.initialize_mapping(program_id, mapping_name).unwrap();
        // Ensure the program ID got initialized.
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name got initialized.
        assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());

        // Insert the list of keys and values.
        for item in 0..1000 {
            // Prepare the key and value.
            let key = Plaintext::from_str(&format!("{item}field")).unwrap();
            let value = Value::from_str(&format!("{item}u64")).unwrap();
            // Ensure the key did not get initialized.
            assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns None.
            assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());

            // Insert the key and value.
            finalize_store.insert_key_value(program_id, mapping_name, key.clone(), value.clone()).unwrap();
            // Ensure the program ID is still initialized.
            assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
            // Ensure the mapping name is still initialized.
            assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
            // Ensure the key got initialized.
            assert!(finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns Some(value).
            assert_eq!(value, finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().unwrap());
        }

        // Remove the mapping.
        finalize_store.remove_mapping(program_id, mapping_name).unwrap();
        // Ensure the program ID is still initialized.
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name is no longer initialized.
        assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());

        // Check the list of keys and values.
        for item in 0..1000 {
            // Prepare the key.
            let key = Plaintext::from_str(&format!("{item}field")).unwrap();

            // Ensure the key is no longer initialized.
            assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns None.
            assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());
        }
    }

    #[test]
    fn test_remove_program() {
        // Initialize a program ID and mapping name.
        let program_id = ProgramID::<CurrentNetwork>::from_str("hello.aleo").unwrap();
        let mapping_name = Identifier::from_str("account").unwrap();

        // Initialize a new finalize store.
        let program_memory = FinalizeMemory::open(StorageMode::Test(None)).unwrap();
        let finalize_store = FinalizeStore::from(program_memory).unwrap();
        // Ensure the program ID does not exist.
        assert!(!finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name does not exist.
        assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure removing an un-initialized mapping fails.
        assert!(finalize_store.remove_mapping(program_id, mapping_name).is_err());

        // Now, initialize the mapping.
        finalize_store.initialize_mapping(program_id, mapping_name).unwrap();
        // Ensure the program ID got initialized.
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name got initialized.
        assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());

        // Insert the list of keys and values.
        for item in 0..1000 {
            // Prepare the key and value.
            let key = Plaintext::from_str(&format!("{item}field")).unwrap();
            let value = Value::from_str(&format!("{item}u64")).unwrap();
            // Ensure the key did not get initialized.
            assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns None.
            assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());

            // Insert the key and value.
            finalize_store.insert_key_value(program_id, mapping_name, key.clone(), value.clone()).unwrap();
            // Ensure the program ID is still initialized.
            assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
            // Ensure the mapping name is still initialized.
            assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
            // Ensure the key got initialized.
            assert!(finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns Some(value).
            assert_eq!(value, finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().unwrap());
        }

        // Remove the program.
        finalize_store.remove_program(&program_id).unwrap();
        // Ensure the program ID is no longer initialized.
        assert!(!finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name is no longer initialized.
        assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());

        // Check the list of keys and values.
        for item in 0..1000 {
            // Prepare the key.
            let key = Plaintext::from_str(&format!("{item}field")).unwrap();

            // Ensure the key is no longer initialized.
            assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns None.
            assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());
        }
    }

    #[test]
    fn test_must_initialize_first() {
        // Initialize a program ID and mapping name.
        let program_id = ProgramID::<CurrentNetwork>::from_str("hello.aleo").unwrap();
        let mapping_name = Identifier::from_str("account").unwrap();

        // Initialize a new finalize store.
        let program_memory = FinalizeMemory::open(StorageMode::Test(None)).unwrap();
        let finalize_store = FinalizeStore::from(program_memory).unwrap();
        // Ensure the program ID does not exist.
        assert!(!finalize_store.contains_program_confirmed(&program_id).unwrap());
        // Ensure the mapping name does not exist.
        assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        // Ensure removing an un-initialized mapping fails.
        assert!(finalize_store.remove_mapping(program_id, mapping_name).is_err());

        {
            // Ensure inserting a (key, value) before initializing the mapping fails.
            let key = Plaintext::from_str("123456789field").unwrap();
            let value = Value::from_str("987654321u128").unwrap();
            assert!(finalize_store.insert_key_value(program_id, mapping_name, key.clone(), value).is_err());

            // Ensure the program ID did not get initialized.
            assert!(!finalize_store.contains_program_confirmed(&program_id).unwrap());
            // Ensure the mapping name did not get initialized.
            assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
            // Ensure the key did not get initialized.
            assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns None.
            assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());
            // Ensure removing an un-initialized key fails.
            assert!(finalize_store.remove_key_value(program_id, mapping_name, &key).is_err());
            // Ensure removing an un-initialized mapping fails.
            assert!(finalize_store.remove_mapping(program_id, mapping_name).is_err());
        }
        {
            // Ensure updating a (key, value) before initializing the mapping fails.
            let key = Plaintext::from_str("987654321field").unwrap();
            let value = Value::from_str("123456789u128").unwrap();
            assert!(finalize_store.update_key_value(program_id, mapping_name, key.clone(), value).is_err());

            // Ensure the program ID did not get initialized.
            assert!(!finalize_store.contains_program_confirmed(&program_id).unwrap());
            // Ensure the mapping name did not get initialized.
            assert!(!finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
            // Ensure the key did not get initialized.
            assert!(!finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
            // Ensure the value returns None.
            assert!(finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().is_none());
            // Ensure removing an un-initialized key fails.
            assert!(finalize_store.remove_key_value(program_id, mapping_name, &key).is_err());
            // Ensure removing an un-initialized mapping fails.
            assert!(finalize_store.remove_mapping(program_id, mapping_name).is_err());
        }

        // Ensure finalize storage still behaves correctly after the above operations.
        check_initialize_insert_remove(&finalize_store, program_id, mapping_name);
        check_initialize_update_remove(&finalize_store, program_id, mapping_name);
    }

    /// If you want to customize the DB size, run:
    /// ```ignore
    /// NUM_ITEMS=100000 cargo test test_finalize_timings -- --nocapture
    /// ```
    /// If you want to run the test with RocksDB, run:
    /// ```ignore
    /// NUM_ITEMS=100000 cargo test test_finalize_timings --features rocks -- --nocapture
    /// ```
    #[test]
    #[ignore]
    fn test_finalize_timings() {
        let rng = &mut TestRng::default();

        // Default to "100000" if the environment variable doesn't exist or is invalid.
        let num_items: u128 = std::env::var("NUM_ITEMS")
            .unwrap_or_else(|_| "100000".to_string())
            .parse()
            .expect("Failed to parse NUM_ITEMS as u128");

        // Initialize a program ID and mapping name.
        let program_id = ProgramID::<CurrentNetwork>::from_str("hello.aleo").unwrap();
        let mapping_name = Identifier::from_str("account").unwrap();

        // Initialize a new finalize store.
        #[cfg(not(feature = "rocks"))]
        let finalize_store = {
            let program_memory = FinalizeMemory::open(StorageMode::Test(None)).unwrap();
            FinalizeStore::from(program_memory).unwrap()
        };

        // Initialize a new finalize store.
        #[cfg(feature = "rocks")]
        let finalize_store = {
            let temp_dir = std::sync::Arc::new(tempfile::tempdir().expect("Failed to open temporary directory"));
            let program_rocksdb = crate::helpers::rocksdb::FinalizeDB::open(temp_dir).unwrap();
            FinalizeStore::from(program_rocksdb).unwrap()
        };

        // Now, initialize the mapping.
        let timer = std::time::Instant::now();
        finalize_store.initialize_mapping(program_id, mapping_name).unwrap();
        println!("FinalizeStore::initialize_mapping - {} μs", timer.elapsed().as_micros());

        // Prepare the key and value.
        let item: u64 = 100u64;
        let key = Plaintext::from(Literal::Field(Field::from_u64(item)));
        let value = Value::from(Literal::U64(U64::new(item)));

        // Insert the key and value.
        let timer = std::time::Instant::now();
        finalize_store.insert_key_value(program_id, mapping_name, key.clone(), value).unwrap();
        println!("FinalizeStore::insert_key_value - {} μs", timer.elapsed().as_micros());

        // Insert the list of keys and values.
        let mut elapsed = 0u128;
        // Start an atomic transaction.
        finalize_store.start_atomic();
        for i in 0..num_items {
            if i != 0 && i % 10_000 == 0 {
                // Finish the atomic transaction.
                if finalize_store.is_atomic_in_progress() {
                    finalize_store.finish_atomic().unwrap();
                }
                println!("FinalizeStore::insert_key_value - {} μs (average over {i} items)", elapsed / i);
                // Start a new atomic transaction.
                finalize_store.start_atomic();
            }

            // Prepare the key and value.
            let item: u64 = rng.random();
            let key = Plaintext::from(Literal::Field(Field::from_u64(item)));
            let value = Value::from(Literal::U64(U64::new(item)));

            // Insert the key and value.
            let timer = std::time::Instant::now();
            finalize_store.insert_key_value(program_id, mapping_name, key, value).unwrap();
            elapsed = elapsed.checked_add(timer.elapsed().as_micros()).unwrap();
        }
        // Finish the atomic transaction.
        if finalize_store.is_atomic_in_progress() {
            finalize_store.finish_atomic().unwrap();
        }
        println!("FinalizeStore::insert_key_value - {} μs (average over {num_items} items)", elapsed / num_items);

        // Retrieve the checksum.
        let timer = std::time::Instant::now();
        finalize_store.get_checksum_confirmed().unwrap();
        println!("FinalizeStore::get_checksum_confirmed - {} μs", timer.elapsed().as_micros());

        // Ensure the program ID is still initialized.
        let timer = std::time::Instant::now();
        assert!(finalize_store.contains_program_confirmed(&program_id).unwrap());
        println!("FinalizeStore::contains_program_confirmed - {} μs", timer.elapsed().as_micros());

        // Ensure the mapping name is still initialized.
        let timer = std::time::Instant::now();
        assert!(finalize_store.contains_mapping_confirmed(&program_id, &mapping_name).unwrap());
        println!("FinalizeStore::contains_mapping_confirmed - {} μs", timer.elapsed().as_micros());

        // Ensure the key got initialized.
        let timer = std::time::Instant::now();
        assert!(finalize_store.contains_key_confirmed(program_id, mapping_name, &key).unwrap());
        println!("FinalizeStore::contains_key_confirmed - {} μs", timer.elapsed().as_micros());

        // Retrieve the value.
        let timer = std::time::Instant::now();
        finalize_store.get_value_speculative(program_id, mapping_name, &key).unwrap().unwrap();
        println!("FinalizeStore::get_value_speculative - {} μs", timer.elapsed().as_micros());

        // Remove the key-value pair.
        let timer = std::time::Instant::now();
        assert!(finalize_store.remove_key_value(program_id, mapping_name, &key).unwrap().is_some());
        println!("FinalizeStore::remove_key_value - {} μs", timer.elapsed().as_micros());

        // Ensure removing the mapping succeeds.
        let timer = std::time::Instant::now();
        finalize_store.remove_mapping(program_id, mapping_name).unwrap();
        println!("FinalizeStore::remove_mapping - {} μs", timer.elapsed().as_micros());

        // Ensure removing the program succeeds.
        let timer = std::time::Instant::now();
        finalize_store.remove_program(&program_id).unwrap();
        println!("FinalizeStore::remove_program - {} μs", timer.elapsed().as_micros());
    }

    /// Verifies `get_historical_mapping_value` returns the floor value for the requested height.
    #[test]
    fn test_get_historical_mapping_value() {
        use std::sync::atomic::Ordering;

        let program_id = ProgramID::<CurrentNetwork>::from_str("hello.aleo").unwrap();
        let mapping_name = Identifier::from_str("account").unwrap();
        let key = Plaintext::from_str("1field").unwrap();

        let program_memory = FinalizeMemory::open(StorageMode::Test(None)).unwrap();
        let finalize_store = FinalizeStore::from(program_memory).unwrap();

        // Initialize program and mapping. The cursor covers every height this test queries.
        finalize_store.set_record_history(true);
        finalize_store.set_history_synced_height(201).unwrap();
        finalize_store.initialize_mapping(program_id, mapping_name).unwrap();

        // Insert at block height 10.
        finalize_store.storage.current_block_height().store(10, Ordering::SeqCst);
        let value_10 = Value::from_str("10u64").unwrap();
        finalize_store.insert_key_value(program_id, mapping_name, key.clone(), value_10.clone()).unwrap();

        // Update at block height 20.
        finalize_store.storage.current_block_height().store(20, Ordering::SeqCst);
        let value_20 = Value::from_str("20u64").unwrap();
        finalize_store.update_key_value(program_id, mapping_name, key.clone(), value_20.clone()).unwrap();

        // Update at block height 50.
        finalize_store.storage.current_block_height().store(50, Ordering::SeqCst);
        let value_50 = Value::from_str("50u64").unwrap();
        finalize_store.update_key_value(program_id, mapping_name, key.clone(), value_50.clone()).unwrap();

        // Update at block height 100.
        finalize_store.storage.current_block_height().store(100, Ordering::SeqCst);
        let value_100 = Value::from_str("100u64").unwrap();
        finalize_store.update_key_value(program_id, mapping_name, key.clone(), value_100.clone()).unwrap();

        // Height 0 (before first insert) => None.
        assert!(
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 0).unwrap().is_none()
        );

        // Height 9 (just before first insert) => None.
        assert!(
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 9).unwrap().is_none()
        );

        // Height 10 (exact match) => value_10.
        let v =
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 10).unwrap().unwrap();
        assert_eq!(*v, value_10);

        // Height 15 (floor → 10) => value_10.
        let v =
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 15).unwrap().unwrap();
        assert_eq!(*v, value_10);

        // Height 20 (exact match) => value_20.
        let v =
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 20).unwrap().unwrap();
        assert_eq!(*v, value_20);

        // Height 49 (floor → 20) => value_20.
        let v =
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 49).unwrap().unwrap();
        assert_eq!(*v, value_20);

        // Height 50 (exact match) => value_50.
        let v =
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 50).unwrap().unwrap();
        assert_eq!(*v, value_50);

        // Height 75 (floor → 50) => value_50.
        let v =
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 75).unwrap().unwrap();
        assert_eq!(*v, value_50);

        // Height 100 (exact match) => value_100.
        let v =
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 100).unwrap().unwrap();
        assert_eq!(*v, value_100);

        // Advance chain past last update height; querying height 150 should floor to 100.
        finalize_store.storage.current_block_height().store(200, Ordering::SeqCst);
        let v =
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 150).unwrap().unwrap();
        assert_eq!(*v, value_100);

        // get_mapping_update_heights returns all heights sorted ascending.
        let heights =
            finalize_store.get_mapping_update_heights(program_id, mapping_name, key.clone()).unwrap().unwrap();
        assert_eq!(&*heights, &[10, 20, 50, 100]);

        // A deletion at height 120 hides the key from that height on, and earlier heights keep value_100.
        finalize_store.storage.current_block_height().store(120, Ordering::SeqCst);
        finalize_store.remove_key_value(program_id, mapping_name, &key).unwrap();
        assert!(
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 120).unwrap().is_none()
        );
        let v =
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 119).unwrap().unwrap();
        assert_eq!(*v, value_100);
        let heights =
            finalize_store.get_mapping_update_heights(program_id, mapping_name, key.clone()).unwrap().unwrap();
        assert_eq!(&*heights, &[10, 20, 50, 100, 120]);
    }

    /// Writes events at heights 1 to 3, then checks that pruning below 3 keeps only height 3 and
    /// leaves the history tables alone.
    fn check_prune_history_events_below<P: FinalizeStorage<CurrentNetwork>>(
        finalize_store: FinalizeStore<CurrentNetwork, P>,
    ) {
        use std::sync::atomic::Ordering;

        let staker = Address::<CurrentNetwork>::zero();
        finalize_store.set_record_history(true);
        finalize_store.current_block_height().store(1, Ordering::SeqCst);
        finalize_store.record_staking_reward(staker, staker, 9, 9).unwrap();

        finalize_store.set_history_recording(HistoryRecording::Events);
        for height in 1..=3u32 {
            finalize_store.current_block_height().store(height, Ordering::SeqCst);
            finalize_store.reset_history_event_seq();
            finalize_store.record_history_block().unwrap();
            finalize_store.record_staking_reward(staker, staker, u64::from(height), 0).unwrap();
        }
        let height_3 =
            vec![HistoryEvent::Indexed, HistoryEvent::Staking { staker, validator: staker, reward: 3, new_stake: 0 }];

        finalize_store.prune_history_events_below(0).unwrap();
        assert_eq!(finalize_store.history_events(1).unwrap().len(), 2);

        finalize_store.prune_history_events_below(3).unwrap();
        assert!(finalize_store.history_events(1).unwrap().is_empty());
        assert!(finalize_store.history_events(2).unwrap().is_empty());
        assert_eq!(finalize_store.history_events(3).unwrap(), height_3);
        assert!(finalize_store.staking_rewards_map().get_confirmed(&(staker, 1)).unwrap().is_some());
    }

    #[test]
    fn test_prune_history_events_below() {
        check_prune_history_events_below(
            FinalizeStore::from(FinalizeMemory::open(StorageMode::Test(None)).unwrap()).unwrap(),
        );
    }

    #[cfg(feature = "rocks")]
    #[test]
    fn test_prune_history_events_below_rocks() {
        check_prune_history_events_below(
            FinalizeStore::<CurrentNetwork, crate::helpers::rocksdb::FinalizeDB<CurrentNetwork>>::open(
                StorageMode::new_test(None),
            )
            .unwrap(),
        );
    }

    /// Verifies `replace_mapping` records only changed and removed keys, and only where recording
    /// is directed.
    #[test]
    fn test_replace_mapping_records_changes_only() {
        use std::sync::atomic::Ordering;

        let program_id = ProgramID::<CurrentNetwork>::from_str("hello.aleo").unwrap();
        let mapping_name = Identifier::from_str("account").unwrap();
        let [k1, k2, k3] = ["1field", "2field", "3field"].map(|key| Plaintext::from_str(key).unwrap());
        let value = |value: &str| Value::from_str(value).unwrap();

        let finalize_store = FinalizeStore::from(FinalizeMemory::open(StorageMode::Test(None)).unwrap()).unwrap();
        finalize_store.set_record_history(true);
        finalize_store.set_history_synced_height(10).unwrap();
        finalize_store.initialize_mapping(program_id, mapping_name).unwrap();
        let replace_at = |height: u32, entries: Vec<(Plaintext<CurrentNetwork>, Value<CurrentNetwork>)>| {
            finalize_store.storage.current_block_height().store(height, Ordering::SeqCst);
            finalize_store.replace_mapping(program_id, mapping_name, entries).unwrap();
        };
        let heights = |key: &Plaintext<CurrentNetwork>| {
            finalize_store
                .get_mapping_update_heights(program_id, mapping_name, key.clone())
                .unwrap()
                .map(|heights| heights.into_owned())
        };
        let at = |key: &Plaintext<CurrentNetwork>, height: u32| {
            finalize_store
                .get_historical_mapping_value(program_id, mapping_name, key.clone(), height)
                .unwrap()
                .map(|value| value.into_owned())
        };

        replace_at(1, vec![(k1.clone(), value("1u64")), (k2.clone(), value("2u64"))]);
        replace_at(2, vec![(k1.clone(), value("1u64")), (k2.clone(), value("3u64")), (k3.clone(), value("4u64"))]);
        replace_at(3, vec![(k2.clone(), value("3u64"))]);

        // An unchanged value keeps its earlier record. A key left out of the replacement is absent.
        assert_eq!(heights(&k1), Some(vec![1, 3]));
        assert_eq!(heights(&k2), Some(vec![1, 2]));
        assert_eq!(heights(&k3), Some(vec![2, 3]));
        assert_eq!(at(&k1, 2), Some(value("1u64")));
        assert_eq!(at(&k1, 3), None);
        assert_eq!(at(&k2, 3), Some(value("3u64")));
        assert_eq!(at(&k3, 2), Some(value("4u64")));

        // With recording off, a replacement writes no history.
        finalize_store.set_record_history(false);
        replace_at(4, vec![(k2.clone(), value("5u64"))]);
        assert_eq!(heights(&k2), Some(vec![1, 2]));

        // Recording to the event log leaves the history tables unchanged.
        let staker = Address::<CurrentNetwork>::zero();
        finalize_store.set_history_recording(HistoryRecording::Events);
        finalize_store.reset_history_event_seq();
        replace_at(5, vec![(k2.clone(), value("6u64"))]);
        finalize_store.record_staking_reward(staker, staker, 7, 8).unwrap();
        assert_eq!(heights(&k2), Some(vec![1, 2]));
        assert!(finalize_store.staking_rewards_map().get_confirmed(&(staker, 5)).unwrap().is_none());
        assert_eq!(finalize_store.history_events(5).unwrap(), vec![
            HistoryEvent::Mapping {
                program_id,
                mapping_name,
                key: k2.clone(),
                value: Box::new(HistoricalMappingValue::Present(value("6u64"))),
            },
            HistoryEvent::Staking { staker, validator: staker, reward: 7, new_stake: 8 },
        ]);
    }
}
