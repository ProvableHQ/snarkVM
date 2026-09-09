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
#[cfg(any(feature = "history-staking-rewards", feature = "slipstream-plugins"))]
use console::types::Address;
use console::{
    network::prelude::*,
    program::{Identifier, Plaintext, ProgramID, Value},
    types::Field,
};
use snarkvm_ledger_block::RejectedReason;
use snarkvm_synthesizer_program::{FinalizeOperation, FinalizeStoreTrait};

use aleo_std_storage::StorageMode;
use anyhow::Result;
use core::marker::PhantomData;
use indexmap::IndexSet;
#[cfg(all(feature = "slipstream-plugins", feature = "locktick"))]
use locktick::parking_lot::RwLock;
#[cfg(all(feature = "slipstream-plugins", not(feature = "locktick")))]
use parking_lot::RwLock;
#[cfg(feature = "slipstream-plugins")]
use snarkvm_slipstream_plugin_manager::{BroadcastEvent, BroadcastEventKind, SlipstreamPluginManager};
#[cfg(feature = "slipstream-plugins")]
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, atomic::AtomicU32};
#[cfg(any(feature = "history", feature = "history-staking-rewards", feature = "slipstream-plugins"))]
use std::{borrow::Cow, sync::atomic::Ordering};

/// Serialized form of a mapping replacement, captured before storage consumes the entries.
#[cfg(feature = "slipstream-plugins")]
struct SerializedMappingEntries {
    program_id: Vec<u8>,
    mapping_name: Vec<u8>,
    entries: Vec<(Vec<u8>, Vec<u8>)>,
}

/// The block height component of a [`FinalizeStorage::MappingUpdateMap`] key, stored as 4 raw bytes.
///
/// New entries encode the height in **big-endian** order so that lexicographic key order matches
/// numeric height order, enabling O(log n) floor seeks via `get_floor_confirmed`.
///
/// Legacy entries (written before this schema change) use **little-endian** order (the `bincode`
/// default for `u32`).  They are distinguished at read time by the presence of an entry in
/// `mapping_update_heights_map`; see `get_historical_mapping_value` for details.
#[cfg(feature = "history")]
pub(crate) type HeightBytes = [u8; 4];

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
    /// The mapping of `(program ID, mapping name, key, height)` to `value`.
    ///
    /// The height component is a [`HeightBytes`]: big-endian for new entries, little-endian
    /// for legacy entries (detected via `mapping_update_heights_map`).
    ///
    /// Big-endian encoding lets lexicographic key order match numeric height order, enabling
    /// O(log n) floor lookups via `get_floor_confirmed`.
    #[cfg(feature = "history")]
    type MappingUpdateMap: for<'a> Map<'a, (ProgramID<N>, Identifier<N>, Plaintext<N>, HeightBytes), Value<N>>;
    /// The mapping of `(program ID, mapping name, key)` to `[height]`.
    ///
    /// Present only for keys written before the big-endian schema change. Acts as a
    /// "legacy sentinel": if an entry exists here the key still uses the old LE encoding,
    /// and `get_historical_mapping_value` falls back to the O(n) binary-search path.
    #[cfg(feature = "history")]
    type MappingUpdateHeightsMap: for<'a> Map<'a, (ProgramID<N>, Identifier<N>, Plaintext<N>), Vec<u32>>;
    /// The mapping of `(staker address, height)` to `(validator address, block reward, new stake)`.
    #[cfg(feature = "history-staking-rewards")]
    type StakingRewardsMap: for<'a> Map<'a, (Address<N>, u32), (Address<N>, u64, u64)>;

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
    #[cfg(feature = "history")]
    fn mapping_update_map(&self) -> &Self::MappingUpdateMap;
    /// Returns the historical mapping update heights map (legacy: present only for pre-schema-change keys).
    #[cfg(feature = "history")]
    fn mapping_update_heights_map(&self) -> &Self::MappingUpdateHeightsMap;
    /// Returns the historical staking rewards map.
    #[cfg(feature = "history-staking-rewards")]
    fn staking_rewards_map(&self) -> &Self::StakingRewardsMap;

    /// Returns the storage mode.
    fn storage_mode(&self) -> &StorageMode;

    /// Starts an atomic batch write operation.
    fn start_atomic(&self) {
        self.committee_store().start_atomic();
        self.program_id_map().start_atomic();
        self.key_value_map().start_atomic();
        self.rejected_reason_map().start_atomic();
        #[cfg(feature = "history")]
        {
            self.mapping_update_map().start_atomic();
            self.mapping_update_heights_map().start_atomic();
        }
        #[cfg(feature = "history-staking-rewards")]
        self.staking_rewards_map().start_atomic();
    }

    /// Checks if an atomic batch is in progress.
    fn is_atomic_in_progress(&self) -> bool {
        let ret = self.committee_store().is_atomic_in_progress()
            || self.program_id_map().is_atomic_in_progress()
            || self.key_value_map().is_atomic_in_progress()
            || self.rejected_reason_map().is_atomic_in_progress();
        #[cfg(feature = "history")]
        let ret = ret
            || self.mapping_update_map().is_atomic_in_progress()
            || self.mapping_update_heights_map().is_atomic_in_progress();
        #[cfg(feature = "history-staking-rewards")]
        let ret = ret || self.staking_rewards_map().is_atomic_in_progress();

        ret
    }

    /// Checkpoints the atomic batch.
    fn atomic_checkpoint(&self) {
        self.committee_store().atomic_checkpoint();
        self.program_id_map().atomic_checkpoint();
        self.key_value_map().atomic_checkpoint();
        self.rejected_reason_map().atomic_checkpoint();
        #[cfg(feature = "history")]
        {
            self.mapping_update_map().atomic_checkpoint();
            self.mapping_update_heights_map().atomic_checkpoint();
        }
        #[cfg(feature = "history-staking-rewards")]
        self.staking_rewards_map().atomic_checkpoint();
    }

    /// Clears the latest atomic batch checkpoint.
    fn clear_latest_checkpoint(&self) {
        self.committee_store().clear_latest_checkpoint();
        self.program_id_map().clear_latest_checkpoint();
        self.key_value_map().clear_latest_checkpoint();
        self.rejected_reason_map().clear_latest_checkpoint();
        #[cfg(feature = "history")]
        {
            self.mapping_update_map().clear_latest_checkpoint();
            self.mapping_update_heights_map().clear_latest_checkpoint();
        }
        #[cfg(feature = "history-staking-rewards")]
        self.staking_rewards_map().clear_latest_checkpoint();
    }

    /// Rewinds the atomic batch to the previous checkpoint.
    fn atomic_rewind(&self) {
        self.committee_store().atomic_rewind();
        self.program_id_map().atomic_rewind();
        self.key_value_map().atomic_rewind();
        self.rejected_reason_map().atomic_rewind();
        #[cfg(feature = "history")]
        {
            self.mapping_update_map().atomic_rewind();
            self.mapping_update_heights_map().atomic_rewind();
        }
        #[cfg(feature = "history-staking-rewards")]
        self.staking_rewards_map().atomic_rewind();
    }

    /// Aborts an atomic batch write operation.
    fn abort_atomic(&self) {
        self.committee_store().abort_atomic();
        self.program_id_map().abort_atomic();
        self.key_value_map().abort_atomic();
        self.rejected_reason_map().abort_atomic();
        #[cfg(feature = "history")]
        {
            self.mapping_update_map().abort_atomic();
            self.mapping_update_heights_map().abort_atomic();
        }
        #[cfg(feature = "history-staking-rewards")]
        self.staking_rewards_map().abort_atomic();
    }

    /// Finishes an atomic batch write operation.
    fn finish_atomic(&self) -> Result<()> {
        self.committee_store().finish_atomic()?;
        self.program_id_map().finish_atomic()?;
        self.key_value_map().finish_atomic()?;
        self.rejected_reason_map().finish_atomic()?;
        #[cfg(feature = "history")]
        {
            self.mapping_update_map().finish_atomic()?;
            self.mapping_update_heights_map().finish_atomic()?;
        }
        #[cfg(feature = "history-staking-rewards")]
        self.staking_rewards_map().finish_atomic()?;
        Ok(())
    }

    /// Returns the current block height.
    #[cfg(feature = "history")]
    fn current_block_height(&self) -> &AtomicU32;

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
            // Record the value at the current height in the historical map.
            // The update heights are reconstructed on read by scanning this map's height suffix,
            // so no separate (and ever-growing) per-key heights vector is maintained here.
            #[cfg(feature = "history")]
            {
                let current_height = self.current_block_height().load(Ordering::SeqCst);
                // Record the value at the current height using big-endian encoding.
                self.mapping_update_map()
                    .insert((program_id, mapping_name, key.clone(), current_height.to_be_bytes()), value.clone())?;
            }

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
            // Record the updated value at the current height in the historical map.
            // The update heights are reconstructed on read by scanning this map's height suffix,
            // so no separate (and ever-growing) per-key heights vector is maintained here.
            #[cfg(feature = "history")]
            {
                let current_height = self.current_block_height().load(Ordering::SeqCst);
                let heights_key = (program_id, mapping_name, key.clone());

                // If this key has a legacy heights-map entry it was written before the BE schema
                // change; continue appending to the heights vec and write with the original LE
                // encoding so that reads using the heights-map path remain correct.
                if let Some(heights) = self.mapping_update_heights_map().get_confirmed(&heights_key)? {
                    let mut heights = heights.into_owned();
                    self.mapping_update_map()
                        .insert((program_id, mapping_name, key.clone(), current_height.to_le_bytes()), value.clone())?;
                    heights.push(current_height);
                    self.mapping_update_heights_map().insert(heights_key, heights)?;
                } else {
                    // New key: use the big-endian encoding so floor seeks work correctly.
                    self.mapping_update_map()
                        .insert((program_id, mapping_name, key.clone(), current_height.to_be_bytes()), value.clone())?;
                }
            }

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
            // Remove the existing key-value entries.
            self.key_value_map().remove_map(&(program_id, mapping_name))?;

            // Insert the new key-value entries.
            for (key, value) in entries {
                // Record the updated value at the current height in the historical map.
                // The update heights are reconstructed on read by scanning this map's height suffix,
                // so no separate (and ever-growing) per-key heights vector is maintained here.
                #[cfg(feature = "history")]
                {
                    let current_height = self.current_block_height().load(Ordering::SeqCst);
                    let heights_key = (program_id, mapping_name, key.clone());

                    // Legacy keys (pre-BE schema) continue using LE encoding + heights vec.
                    if let Some(heights) = self.mapping_update_heights_map().get_confirmed(&heights_key)? {
                        let mut heights = heights.into_owned();
                        self.mapping_update_map().insert(
                            (program_id, mapping_name, key.clone(), current_height.to_le_bytes()),
                            value.clone(),
                        )?;
                        heights.push(current_height);
                        self.mapping_update_heights_map().insert(heights_key, heights)?;
                    } else {
                        // New key: big-endian encoding.
                        self.mapping_update_map().insert(
                            (program_id, mapping_name, key.clone(), current_height.to_be_bytes()),
                            value.clone(),
                        )?;
                    }
                }

                // Insert the key-value entry.
                self.key_value_map().insert((program_id, mapping_name), key, value)?;
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
    /// Indicates that canonical finalize is currently in progress.
    /// When `true`, storage writes notify registered Slipstream plugins.
    #[cfg(feature = "slipstream-plugins")]
    is_finalize_mode: Arc<AtomicBool>,
    /// Tracks the current block height.
    /// Updated by the VM at the start of each canonical finalize
    block_height: Arc<AtomicU32>,
    /// Optional plugin manager for streaming canonical mapping and staking updates.
    /// Wrapped in `Arc` so that all clones of `FinalizeStore` share the same instance;
    /// the `RwLock` allows installation from a shared reference after construction.
    #[cfg(feature = "slipstream-plugins")]
    slipstream_plugin_manager: Arc<RwLock<Option<SlipstreamPluginManager>>>,
}

impl<N: Network, P: FinalizeStorage<N>> FinalizeStore<N, P> {
    /// Initializes the finalize store.
    pub fn open<S: Into<StorageMode>>(storage: S) -> Result<Self> {
        Self::from(P::open(storage)?)
    }

    /// Initializes a finalize store from storage.
    pub fn from(storage: P) -> Result<Self> {
        // Return the finalize store.
        Ok(Self {
            storage,
            _phantom: PhantomData,
            #[cfg(feature = "slipstream-plugins")]
            is_finalize_mode: Arc::new(AtomicBool::new(false)),
            block_height: Arc::new(AtomicU32::new(0)),
            #[cfg(feature = "slipstream-plugins")]
            slipstream_plugin_manager: Arc::new(RwLock::new(None)),
        })
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
    #[cfg(feature = "history")]
    pub fn current_block_height(&self) -> &AtomicU32 {
        self.storage.current_block_height()
    }

    /// Returns a reference to the canonical finalize mode flag.
    ///
    /// When `true`, storage writes notify registered Slipstream plugins.
    /// Set to `true` by the VM before canonical finalize runs and reset to `false` afterwards.
    #[cfg(feature = "slipstream-plugins")]
    pub fn is_finalize_mode(&self) -> &Arc<AtomicBool> {
        &self.is_finalize_mode
    }

    /// Returns the current block height.
    pub fn block_height(&self) -> &AtomicU32 {
        &self.block_height
    }

    /// Installs a Slipstream plugin manager to receive canonical mapping and staking updates.
    ///
    /// May be called from a shared reference. Logs a warning if called more than once.
    #[cfg(feature = "slipstream-plugins")]
    pub fn set_slipstream_plugin_manager(&self, manager: SlipstreamPluginManager) {
        let mut guard = self.slipstream_plugin_manager.write();
        if guard.is_some() {
            tracing::warn!("Slipstream plugin manager is already set; ignoring subsequent call.");
            return;
        }
        *guard = Some(manager);
    }

    /// Returns a handle to the Slipstream plugin manager cell.
    ///
    /// The returned `Arc` is a lightweight additional handle to the same shared instance;
    /// acquire a read or write lock on it to inspect or replace the manager.
    #[cfg(feature = "slipstream-plugins")]
    pub fn slipstream_plugin_manager(&self) -> Arc<RwLock<Option<SlipstreamPluginManager>>> {
        Arc::clone(&self.slipstream_plugin_manager)
    }

    /// Notifies all interested plugins of a staking reward, if canonical finalize is active.
    ///
    /// Errors from plugin calls are logged but never propagated.
    #[cfg(feature = "slipstream-plugins")]
    pub fn notify_staking_reward(
        &self,
        staker: &Address<N>,
        validator: &Address<N>,
        reward: u64,
        new_stake: u64,
        block_height: u32,
    ) {
        if !self.is_finalize_mode.load(Ordering::SeqCst) {
            return;
        }

        let spm_guard = self.slipstream_plugin_manager.read();
        if let Some(mgr) = spm_guard.as_ref()
            && mgr.has_subscribers(BroadcastEventKind::StakingReward)
        {
            // Address serializes to a fixed 32-byte array; this cannot fail.
            let staker_bytes = staker.to_bytes_le().expect("Address::to_bytes_le is infallible");
            let validator_bytes = validator.to_bytes_le().expect("Address::to_bytes_le is infallible");
            mgr.broadcast(BroadcastEvent::StakingReward {
                staker: &staker_bytes,
                validator: &validator_bytes,
                reward,
                new_stake,
                block_height,
            });
        }
    }

    /// Returns the historical value of a mapping at or before the given block height.
    ///
    /// **Fast path** (new keys, no `mapping_update_heights_map` entry): single O(log n)
    /// floor seek on `mapping_update_map`, which uses big-endian height encoding.
    ///
    /// **Legacy path** (keys written before the BE schema change, heights-map entry
    /// present): O(n) binary search over the heights `Vec`, then a point lookup using
    /// the original little-endian encoding. Correct but slower; these keys stay on this
    /// path until the node is resynced or an offline migration is performed.
    #[cfg(feature = "history")]
    pub fn get_historical_mapping_value(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        mapping_key: Plaintext<N>,
        height: u32,
    ) -> Result<Option<Cow<'_, Value<N>>>, Error> {
        // Return nothing for future heights, as the mapping value might change by then.
        if height > self.current_block_height().load(Ordering::SeqCst) {
            return Ok(None);
        }

        // Check for a legacy heights-map entry (pre-BE schema change).
        let heights_key = (program_id, mapping_name, mapping_key.clone());
        if let Some(heights) = self.storage.mapping_update_heights_map().get_confirmed(&heights_key)? {
            // Legacy O(n) path: binary search on the heights Vec.
            let heights = heights.into_owned();
            let applicable_height = match heights.binary_search(&height) {
                Ok(_) => height,
                Err(0) => return Ok(None),
                Err(idx) => heights[idx - 1],
            };
            // Look up with the original little-endian encoding.
            return self.storage.mapping_update_map().get_confirmed(&(
                program_id,
                mapping_name,
                mapping_key,
                applicable_height.to_le_bytes(),
            ));
        }

        // New fast path: O(log n) floor seek with big-endian encoding.
        let seek_key = (program_id, mapping_name, mapping_key.clone(), height.to_be_bytes());
        match self.storage.mapping_update_map().get_floor_confirmed(&seek_key)? {
            Some((found_key, found_value)) => {
                let (p, m, k, _h) = found_key.into_owned();
                if p == program_id && m == mapping_name && k == mapping_key { Ok(Some(found_value)) } else { Ok(None) }
            }
            None => Ok(None),
        }
    }

    /// Returns the heights at which past mapping updates occurred, in ascending order.
    ///
    /// For legacy keys (heights-map entry present) the list is read directly from the
    /// heights map. For new keys it is reconstructed by scanning `mapping_update_map`.
    /// Either way this is O(n updates) and is intended for diagnostic / test use only.
    #[cfg(feature = "history")]
    pub fn get_mapping_update_heights(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        mapping_key: Plaintext<N>,
    ) -> Result<Option<Cow<'_, Vec<u32>>>, Error> {
        // Legacy path: heights are stored explicitly in the heights map.
        let heights_key = (program_id, mapping_name, mapping_key.clone());
        if let Some(heights) = self.storage.mapping_update_heights_map().get_confirmed(&heights_key)? {
            return Ok(Some(heights));
        }

        // New path: reconstruct from mapping_update_map keys (big-endian encoded heights).
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

    /// Returns the historical staking rewards map.
    #[cfg(feature = "history-staking-rewards")]
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
        // Serialize before moving, if a plugin notification may be needed.
        #[cfg(feature = "slipstream-plugins")]
        let plugin_data = if self.is_finalize_mode.load(Ordering::SeqCst) {
            let spm_guard = self.slipstream_plugin_manager.read();
            if let Some(mgr) = spm_guard.as_ref() {
                if mgr.has_subscribers(BroadcastEventKind::MappingUpdate) {
                    Some((
                        program_id.to_bytes_le()?,
                        mapping_name.to_bytes_le()?,
                        key.to_bytes_le()?,
                        value.to_bytes_le()?,
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let result = self.storage.update_key_value(program_id, mapping_name, key, value)?;

        // Notify plugins of the update if in canonical finalize mode.
        #[cfg(feature = "slipstream-plugins")]
        if let Some((pid, mname, k, v)) = plugin_data {
            let height = self.block_height().load(Ordering::SeqCst);
            let spm_guard = self.slipstream_plugin_manager.read();
            if let Some(mgr) = spm_guard.as_ref() {
                mgr.broadcast(BroadcastEvent::MappingUpdate {
                    program_id: &pid,
                    mapping_name: &mname,
                    key: &k,
                    value: &v,
                    block_height: height,
                });
            }
        }
        Ok(result)
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
        // Serialize mapping identity and all entries before moving them into storage,
        // so they are available for plugin notification after the storage call.
        #[cfg(feature = "slipstream-plugins")]
        let plugin_data: Option<SerializedMappingEntries> = if self.is_finalize_mode.load(Ordering::SeqCst) {
            let spm_guard = self.slipstream_plugin_manager.read();
            if let Some(mgr) = spm_guard.as_ref() {
                if mgr.has_subscribers(BroadcastEventKind::MappingUpdate) {
                    let mut entries_bytes = Vec::with_capacity(entries.len());
                    for (key, value) in &entries {
                        entries_bytes.push((key.to_bytes_le()?, value.to_bytes_le()?));
                    }
                    Some(SerializedMappingEntries {
                        program_id: program_id.to_bytes_le()?,
                        mapping_name: mapping_name.to_bytes_le()?,
                        entries: entries_bytes,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let result = self.storage.replace_mapping(program_id, mapping_name, entries)?;

        // Notify plugins of each updated key-value pair if in canonical finalize mode.
        #[cfg(feature = "slipstream-plugins")]
        if let Some(data) = plugin_data {
            let height = self.block_height().load(Ordering::SeqCst);
            let spm_guard = self.slipstream_plugin_manager.read();
            if let Some(mgr) = spm_guard.as_ref() {
                for (k, v) in &data.entries {
                    mgr.broadcast(BroadcastEvent::MappingUpdate {
                        program_id: &data.program_id,
                        mapping_name: &data.mapping_name,
                        key: k,
                        value: v,
                        block_height: height,
                    });
                }
            }
        }

        Ok(result)
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
        if cfg!(any(feature = "history", feature = "test")) || consensus_version >= ConsensusVersion::V15 {
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
    #[cfg(feature = "history")]
    fn test_get_historical_mapping_value() {
        use std::sync::atomic::Ordering;

        let program_id = ProgramID::<CurrentNetwork>::from_str("hello.aleo").unwrap();
        let mapping_name = Identifier::from_str("account").unwrap();
        let key = Plaintext::from_str("1field").unwrap();

        let program_memory = FinalizeMemory::open(StorageMode::Test(None)).unwrap();
        let finalize_store = FinalizeStore::from(program_memory).unwrap();

        // Initialize program and mapping.
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
    }

    /// Verifies the legacy (pre-BE schema) read path: keys whose heights are stored in
    /// `mapping_update_heights_map` continue to be found correctly via binary search.
    #[test]
    #[cfg(feature = "history")]
    fn test_get_historical_mapping_value_legacy() {
        use std::sync::atomic::Ordering;

        let program_id = ProgramID::<CurrentNetwork>::from_str("hello.aleo").unwrap();
        let mapping_name = Identifier::from_str("account").unwrap();
        let key = Plaintext::from_str("1field").unwrap();

        let program_memory = FinalizeMemory::open(StorageMode::Test(None)).unwrap();
        let finalize_store = FinalizeStore::from(program_memory).unwrap();

        finalize_store.initialize_mapping(program_id, mapping_name).unwrap();

        // Simulate legacy writes: insert directly into the LE-keyed update map AND into the heights map,
        // exactly as the old code did.
        let v5 = Value::from_str("5u64").unwrap();
        let v10 = Value::from_str("10u64").unwrap();
        finalize_store
            .storage
            .mapping_update_map()
            .insert((program_id, mapping_name, key.clone(), 5u32.to_le_bytes()), v5.clone())
            .unwrap();
        finalize_store
            .storage
            .mapping_update_map()
            .insert((program_id, mapping_name, key.clone(), 10u32.to_le_bytes()), v10.clone())
            .unwrap();
        finalize_store
            .storage
            .mapping_update_heights_map()
            .insert((program_id, mapping_name, key.clone()), vec![5, 10])
            .unwrap();
        finalize_store.storage.current_block_height().store(20, Ordering::SeqCst);

        // Height before first update → None.
        assert!(
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 4).unwrap().is_none()
        );
        // Height 5 (exact) → v5.
        let v = finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 5).unwrap().unwrap();
        assert_eq!(*v, v5);
        // Height 7 (floor → 5) → v5.
        let v = finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 7).unwrap().unwrap();
        assert_eq!(*v, v5);
        // Height 10 (exact) → v10.
        let v =
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 10).unwrap().unwrap();
        assert_eq!(*v, v10);
        // Height 15 (floor → 10) → v10.
        let v =
            finalize_store.get_historical_mapping_value(program_id, mapping_name, key.clone(), 15).unwrap().unwrap();
        assert_eq!(*v, v10);

        // Heights list comes from the heights map.
        let heights =
            finalize_store.get_mapping_update_heights(program_id, mapping_name, key.clone()).unwrap().unwrap();
        assert_eq!(&*heights, &[5, 10]);
    }
}
