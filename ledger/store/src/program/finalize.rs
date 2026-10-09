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
    types::Field,
};
use snarkvm_ledger_block::RejectedReason;
use snarkvm_synthesizer_program::{FinalizeOperation, FinalizeStoreTrait};

use aleo_std_storage::StorageMode;
use anyhow::Result;
use core::marker::PhantomData;
use indexmap::{IndexMap, IndexSet};
#[cfg(all(not(target_arch = "wasm32"), feature = "locktick"))]
use locktick::parking_lot::RwLock;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "locktick")))]
use parking_lot::RwLock;
#[cfg(not(target_arch = "wasm32"))]
use snarkvm_slipstream_plugin_manager::{BroadcastEvent, BroadcastEventKind, SlipstreamPluginManager};

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

/// Serialized form of one mapping update: program ID, mapping name, key, and value.
#[cfg(not(target_arch = "wasm32"))]
type SerializedMappingUpdate = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

/// Serialized form of a mapping replacement, captured before storage consumes the entries.
#[cfg(not(target_arch = "wasm32"))]
struct SerializedMappingEntries {
    program_id: Vec<u8>,
    mapping_name: Vec<u8>,
    entries: Vec<(Vec<u8>, Vec<u8>)>,
}

/// Serialized mapping removal.
///
/// `keys` is `None` when the mapping itself is removed. Otherwise it lists removed keys.
#[cfg(not(target_arch = "wasm32"))]
struct SerializedMappingRemovals {
    program_id: Vec<u8>,
    mapping_name: Vec<u8>,
    keys: Option<Vec<Vec<u8>>>,
}

/// Serializes `value` for a slipstream event.
///
/// A serialization failure logs a warning and returns `None`.
#[cfg(not(target_arch = "wasm32"))]
fn serialize_slipstream_bytes(value: &impl ToBytes, label: &str) -> Option<Vec<u8>> {
    match value.to_bytes_le() {
        Ok(bytes) => Some(bytes),
        Err(_) => {
            tracing::warn!("Failed to serialize a {label} for slipstream");
            None
        }
    }
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

    /// Returns the storage mode.
    fn storage_mode(&self) -> &StorageMode;

    /// Starts an atomic batch write operation.
    fn start_atomic(&self) {
        self.committee_store().start_atomic();
        self.program_id_map().start_atomic();
        self.key_value_map().start_atomic();
        self.rejected_reason_map().start_atomic();
    }

    /// Checks if an atomic batch is in progress.
    fn is_atomic_in_progress(&self) -> bool {
        self.committee_store().is_atomic_in_progress()
            || self.program_id_map().is_atomic_in_progress()
            || self.key_value_map().is_atomic_in_progress()
            || self.rejected_reason_map().is_atomic_in_progress()
    }

    /// Checkpoints the atomic batch.
    fn atomic_checkpoint(&self) {
        self.committee_store().atomic_checkpoint();
        self.program_id_map().atomic_checkpoint();
        self.key_value_map().atomic_checkpoint();
        self.rejected_reason_map().atomic_checkpoint();
    }

    /// Clears the latest atomic batch checkpoint.
    fn clear_latest_checkpoint(&self) {
        self.committee_store().clear_latest_checkpoint();
        self.program_id_map().clear_latest_checkpoint();
        self.key_value_map().clear_latest_checkpoint();
        self.rejected_reason_map().clear_latest_checkpoint();
    }

    /// Rewinds the atomic batch to the previous checkpoint.
    fn atomic_rewind(&self) {
        self.committee_store().atomic_rewind();
        self.program_id_map().atomic_rewind();
        self.key_value_map().atomic_rewind();
        self.rejected_reason_map().atomic_rewind();
    }

    /// Aborts an atomic batch write operation.
    fn abort_atomic(&self) {
        self.committee_store().abort_atomic();
        self.program_id_map().abort_atomic();
        self.key_value_map().abort_atomic();
        self.rejected_reason_map().abort_atomic();
    }

    /// Finishes an atomic batch write operation.
    fn finish_atomic(&self) -> Result<()> {
        self.committee_store().finish_atomic()?;
        self.program_id_map().finish_atomic()?;
        self.key_value_map().finish_atomic()?;
        self.rejected_reason_map().finish_atomic()?;

        Ok(())
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
    /// Tracks the current block height.
    /// Updated by the VM at the start of each canonical finalize
    block_height: Arc<AtomicU32>,
    /// When set, canonical finalize writes credits.aleo history as JSON.
    record_history_json: Arc<AtomicBool>,
    /// When set, canonical finalize emits mapping updates, mapping removals, staking rewards, and committed blocks.
    slipstream: Arc<AtomicBool>,
    /// Set for the duration of canonical finalize.
    /// Mapping and staking events are emitted only while this is set.
    #[cfg(not(target_arch = "wasm32"))]
    is_finalize_mode: Arc<AtomicBool>,
    /// Plugins that receive slipstream events. Shared by every clone of this store.
    #[cfg(not(target_arch = "wasm32"))]
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
            block_height: Arc::new(AtomicU32::new(0)),
            record_history_json: Arc::new(AtomicBool::new(false)),
            slipstream: Arc::new(AtomicBool::new(false)),
            #[cfg(not(target_arch = "wasm32"))]
            is_finalize_mode: Arc::new(AtomicBool::new(false)),
            #[cfg(not(target_arch = "wasm32"))]
            slipstream_plugin_manager: Arc::new(RwLock::new(None)),
        })
    }

    /// Enables or disables JSON history for later canonical finalizes.
    pub fn set_record_history_json(&self, enabled: bool) {
        self.record_history_json.store(enabled, Ordering::SeqCst);
    }

    /// Returns whether canonical finalize writes JSON history.
    pub fn record_history_json(&self) -> bool {
        self.record_history_json.load(Ordering::SeqCst)
    }

    /// Enables or disables slipstream for later canonical finalizes and committed blocks.
    pub fn set_slipstream(&self, enabled: bool) {
        self.slipstream.store(enabled, Ordering::SeqCst);
    }

    /// Returns whether slipstream emits mapping updates, mapping removals, staking rewards, and committed blocks.
    pub fn slipstream(&self) -> bool {
        self.slipstream.load(Ordering::SeqCst)
    }

    /// Returns the flag that is set for the duration of canonical finalize.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn is_finalize_mode(&self) -> &Arc<AtomicBool> {
        &self.is_finalize_mode
    }

    /// Installs the plugin manager that receives slipstream events.
    ///
    /// A later call leaves the installed manager in place.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_slipstream_plugin_manager(&self, manager: SlipstreamPluginManager) {
        let mut guard = self.slipstream_plugin_manager.write();
        if guard.is_some() {
            tracing::warn!("Slipstream plugin manager is already set; ignoring subsequent call.");
            return;
        }
        *guard = Some(manager);
    }

    /// Returns a handle to the installed plugin manager.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn slipstream_plugin_manager(&self) -> Arc<RwLock<Option<SlipstreamPluginManager>>> {
        Arc::clone(&self.slipstream_plugin_manager)
    }

    /// Returns `true` when slipstream is enabled and a loaded plugin subscribes to `kind`.
    ///
    /// Mapping and staking events also require canonical finalize. Block events do not.
    #[cfg(not(target_arch = "wasm32"))]
    fn slipstream_wants(&self, kind: BroadcastEventKind) -> bool {
        if !self.slipstream.load(Ordering::SeqCst) {
            return false;
        }
        if kind != BroadcastEventKind::Block && !self.is_finalize_mode.load(Ordering::SeqCst) {
            return false;
        }
        self.slipstream_plugin_manager.read().as_ref().is_some_and(|manager| manager.has_subscribers(kind))
    }

    /// Emits one staking reward per current staker when a plugin subscribes to staking rewards.
    ///
    /// A staker absent from `next_stakers` is emitted with reward `0` and the current stake.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn notify_staking_rewards(
        &self,
        current_stakers: &IndexMap<console::types::Address<N>, (console::types::Address<N>, u64)>,
        next_stakers: &IndexMap<console::types::Address<N>, (console::types::Address<N>, u64)>,
        block_height: u32,
    ) {
        if !self.slipstream_wants(BroadcastEventKind::StakingReward) {
            return;
        }
        let guard = self.slipstream_plugin_manager.read();
        let Some(manager) = guard.as_ref() else {
            return;
        };
        for (staker, (validator, stake)) in current_stakers {
            let (next_validator, new_stake) = next_stakers.get(staker).copied().unwrap_or((*validator, *stake));
            let reward = new_stake.saturating_sub(*stake);
            // Address encodes to 32 bytes. The conversion does not fail.
            let staker_bytes = staker.to_bytes_le().expect("Address::to_bytes_le is infallible");
            let validator_bytes = next_validator.to_bytes_le().expect("Address::to_bytes_le is infallible");
            manager.broadcast(BroadcastEvent::StakingReward {
                staker: &staker_bytes,
                validator: &validator_bytes,
                reward,
                new_stake,
                block_height,
            });
        }
    }

    /// Emits a committed block when a plugin subscribes to blocks.
    ///
    /// A serialization failure skips the event.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn notify_block(&self, block: &impl ToBytes, block_height: u32) {
        if !self.slipstream_wants(BroadcastEventKind::Block) {
            return;
        }
        let Ok(bytes) = block.to_bytes_le() else {
            tracing::warn!("Failed to serialize block {block_height} for slipstream");
            return;
        };
        let guard = self.slipstream_plugin_manager.read();
        if let Some(manager) = guard.as_ref() {
            manager.broadcast(BroadcastEvent::Block { block: &bytes, block_height });
        }
    }

    /// Serializes a mapping update when a plugin subscribes to mapping updates.
    #[cfg(not(target_arch = "wasm32"))]
    fn capture_mapping_update(
        &self,
        program_id: &ProgramID<N>,
        mapping_name: &Identifier<N>,
        key: &Plaintext<N>,
        value: &Value<N>,
    ) -> Option<SerializedMappingUpdate> {
        if !self.slipstream_wants(BroadcastEventKind::MappingUpdate) {
            return None;
        }
        let Ok(program_id) = program_id.to_bytes_le() else {
            tracing::warn!("Failed to serialize a program ID for slipstream");
            return None;
        };
        let Ok(mapping_name) = mapping_name.to_bytes_le() else {
            tracing::warn!("Failed to serialize a mapping name for slipstream");
            return None;
        };
        let Ok(key) = key.to_bytes_le() else {
            tracing::warn!("Failed to serialize a mapping key for slipstream");
            return None;
        };
        let Ok(value) = value.to_bytes_le() else {
            tracing::warn!("Failed to serialize a mapping value for slipstream");
            return None;
        };
        Some((program_id, mapping_name, key, value))
    }

    /// Delivers a previously serialized mapping update.
    #[cfg(not(target_arch = "wasm32"))]
    fn emit_mapping_update(&self, plugin_data: Option<SerializedMappingUpdate>) {
        let Some((program_id, mapping_name, key, value)) = plugin_data else {
            return;
        };
        let height = self.block_height().load(Ordering::SeqCst);
        let guard = self.slipstream_plugin_manager.read();
        if let Some(manager) = guard.as_ref() {
            manager.broadcast(BroadcastEvent::MappingUpdate {
                program_id: &program_id,
                mapping_name: &mapping_name,
                key: &key,
                value: &value,
                block_height: height,
            });
        }
    }

    /// Serializes each entry of a mapping replacement when a plugin subscribes to mapping updates.
    #[cfg(not(target_arch = "wasm32"))]
    fn capture_mapping_entries(
        &self,
        program_id: &ProgramID<N>,
        mapping_name: &Identifier<N>,
        entries: &[(Plaintext<N>, Value<N>)],
    ) -> Option<SerializedMappingEntries> {
        if !self.slipstream_wants(BroadcastEventKind::MappingUpdate) {
            return None;
        }
        let Ok(program_id) = program_id.to_bytes_le() else {
            tracing::warn!("Failed to serialize a program ID for slipstream");
            return None;
        };
        let Ok(mapping_name) = mapping_name.to_bytes_le() else {
            tracing::warn!("Failed to serialize a mapping name for slipstream");
            return None;
        };
        let mut entries_bytes = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            match (key.to_bytes_le(), value.to_bytes_le()) {
                (Ok(key), Ok(value)) => entries_bytes.push((key, value)),
                (Err(_), _) | (_, Err(_)) => {
                    tracing::warn!("Failed to serialize a mapping entry for slipstream");
                }
            }
        }
        Some(SerializedMappingEntries { program_id, mapping_name, entries: entries_bytes })
    }

    /// Serializes one mapping key when a plugin subscribes to mapping removals.
    #[cfg(not(target_arch = "wasm32"))]
    fn capture_mapping_key(
        &self,
        program_id: &ProgramID<N>,
        mapping_name: &Identifier<N>,
        key: &Plaintext<N>,
    ) -> Option<SerializedMappingRemovals> {
        if !self.slipstream_wants(BroadcastEventKind::MappingRemoval) {
            return None;
        }
        let (Some(program_id), Some(mapping_name), Some(key)) = (
            serialize_slipstream_bytes(program_id, "program ID"),
            serialize_slipstream_bytes(mapping_name, "mapping name"),
            serialize_slipstream_bytes(key, "mapping key"),
        ) else {
            return None;
        };
        Some(SerializedMappingRemovals { program_id, mapping_name, keys: Some(vec![key]) })
    }

    /// Serializes keys that `entries` does not keep when a plugin subscribes to mapping removals.
    #[cfg(not(target_arch = "wasm32"))]
    fn capture_removed_mapping_keys(
        &self,
        program_id: &ProgramID<N>,
        mapping_name: &Identifier<N>,
        entries: &[(Plaintext<N>, Value<N>)],
    ) -> Option<SerializedMappingRemovals> {
        if !self.slipstream_wants(BroadcastEventKind::MappingRemoval) {
            return None;
        }
        let (Some(program_id_bytes), Some(mapping_name_bytes)) = (
            serialize_slipstream_bytes(program_id, "program ID"),
            serialize_slipstream_bytes(mapping_name, "mapping name"),
        ) else {
            return None;
        };
        let old_entries = match self.get_mapping_speculative(*program_id, *mapping_name) {
            Ok(entries) => entries,
            Err(_) => {
                tracing::warn!("Failed to read a mapping before emitting slipstream removals");
                return None;
            }
        };
        let mut retained = std::collections::HashSet::with_capacity(entries.len());
        for (key, _) in entries {
            let bytes = serialize_slipstream_bytes(key, "mapping key")?;
            retained.insert(bytes);
        }
        let mut removed = Vec::new();
        for (key, _) in &old_entries {
            if let Some(bytes) = serialize_slipstream_bytes(key, "mapping key")
                && !retained.contains(&bytes)
            {
                removed.push(bytes);
            }
        }
        if removed.is_empty() {
            return None;
        }
        Some(SerializedMappingRemovals {
            program_id: program_id_bytes,
            mapping_name: mapping_name_bytes,
            keys: Some(removed),
        })
    }

    /// Serializes a mapping identity when a plugin subscribes to mapping removals.
    #[cfg(not(target_arch = "wasm32"))]
    fn capture_removed_mapping(
        &self,
        program_id: &ProgramID<N>,
        mapping_name: &Identifier<N>,
    ) -> Option<SerializedMappingRemovals> {
        if !self.slipstream_wants(BroadcastEventKind::MappingRemoval) {
            return None;
        }
        let (Some(program_id), Some(mapping_name)) = (
            serialize_slipstream_bytes(program_id, "program ID"),
            serialize_slipstream_bytes(mapping_name, "mapping name"),
        ) else {
            return None;
        };
        Some(SerializedMappingRemovals { program_id, mapping_name, keys: None })
    }

    /// Delivers previously serialized mapping removals.
    #[cfg(not(target_arch = "wasm32"))]
    fn emit_mapping_removals(&self, plugin_data: Option<SerializedMappingRemovals>) {
        let Some(data) = plugin_data else {
            return;
        };
        let height = self.block_height().load(Ordering::SeqCst);
        let guard = self.slipstream_plugin_manager.read();
        let Some(manager) = guard.as_ref() else {
            return;
        };
        match &data.keys {
            None => manager.broadcast(BroadcastEvent::MappingRemoval {
                program_id: &data.program_id,
                mapping_name: &data.mapping_name,
                key: None,
                block_height: height,
            }),
            Some(keys) => {
                for key in keys {
                    manager.broadcast(BroadcastEvent::MappingRemoval {
                        program_id: &data.program_id,
                        mapping_name: &data.mapping_name,
                        key: Some(key),
                        block_height: height,
                    });
                }
            }
        }
    }

    /// Delivers each entry of a previously serialized mapping replacement.
    #[cfg(not(target_arch = "wasm32"))]
    fn emit_mapping_entries(&self, plugin_data: Option<SerializedMappingEntries>) {
        let Some(data) = plugin_data else {
            return;
        };
        let height = self.block_height().load(Ordering::SeqCst);
        let guard = self.slipstream_plugin_manager.read();
        if let Some(manager) = guard.as_ref() {
            for (key, value) in &data.entries {
                manager.broadcast(BroadcastEvent::MappingUpdate {
                    program_id: &data.program_id,
                    mapping_name: &data.mapping_name,
                    key,
                    value,
                    block_height: height,
                });
            }
        }
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
    pub fn block_height(&self) -> &AtomicU32 {
        &self.block_height
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
        #[cfg(not(target_arch = "wasm32"))]
        let plugin_data = self.capture_mapping_update(&program_id, &mapping_name, &key, &value);
        let result = self.storage.update_key_value(program_id, mapping_name, key, value)?;
        #[cfg(not(target_arch = "wasm32"))]
        self.emit_mapping_update(plugin_data);
        Ok(result)
    }

    /// Removes the key-value pair for the given `program ID`, `mapping name`, and `key` from storage.
    fn remove_key_value(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<Option<FinalizeOperation<N>>> {
        #[cfg(not(target_arch = "wasm32"))]
        let plugin_data = self.capture_mapping_key(&program_id, &mapping_name, key);
        let result = self.storage.remove_key_value(program_id, mapping_name, key)?;
        #[cfg(not(target_arch = "wasm32"))]
        if result.is_some() {
            self.emit_mapping_removals(plugin_data);
        }
        Ok(result)
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
        #[cfg(not(target_arch = "wasm32"))]
        let removed_keys = self.capture_removed_mapping_keys(&program_id, &mapping_name, &entries);
        #[cfg(not(target_arch = "wasm32"))]
        let plugin_data = self.capture_mapping_entries(&program_id, &mapping_name, &entries);
        let result = self.storage.replace_mapping(program_id, mapping_name, entries)?;
        #[cfg(not(target_arch = "wasm32"))]
        self.emit_mapping_removals(removed_keys);
        #[cfg(not(target_arch = "wasm32"))]
        self.emit_mapping_entries(plugin_data);
        Ok(result)
    }

    /// Removes the mapping for the given `program ID` and `mapping name` from storage,
    /// along with all associated key-value pairs in storage.
    pub fn remove_mapping(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
    ) -> Result<FinalizeOperation<N>> {
        #[cfg(not(target_arch = "wasm32"))]
        let plugin_data = self.capture_removed_mapping(&program_id, &mapping_name);
        let result = self.storage.remove_mapping(program_id, mapping_name)?;
        #[cfg(not(target_arch = "wasm32"))]
        self.emit_mapping_removals(plugin_data);
        Ok(result)
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

    /// Mapping updates and staking rewards require the runtime flag and canonical finalize.
    /// A committed block requires only the runtime flag.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_slipstream_streams_mappings_rewards_and_blocks() {
        use console::types::Address;
        use snarkvm_slipstream_plugin_manager::{BroadcastEvent, BroadcastEventKind, SlipstreamPlugin};

        #[derive(Debug)]
        struct RecordingPlugin {
            events: std::sync::Arc<std::sync::Mutex<Vec<BroadcastEventKind>>>,
            /// `true` when a mapping removal names a key. `false` when the mapping itself is removed.
            removal_has_key: std::sync::Arc<std::sync::Mutex<Vec<bool>>>,
        }

        impl SlipstreamPlugin for RecordingPlugin {
            fn name(&self) -> &'static str {
                "recording"
            }

            fn subscribed_events(&self) -> &[BroadcastEventKind] {
                const EVENTS: &[BroadcastEventKind] = &[
                    BroadcastEventKind::MappingUpdate,
                    BroadcastEventKind::MappingRemoval,
                    BroadcastEventKind::StakingReward,
                    BroadcastEventKind::Block,
                ];
                EVENTS
            }

            fn on_broadcast(&self, event: BroadcastEvent<'_>) -> anyhow::Result<()> {
                // The test is the only other lock user, and it does not panic while holding the lock.
                self.events.lock().expect("recording plugin lock is not poisoned").push(event.kind());
                if let BroadcastEvent::MappingRemoval { key, .. } = event {
                    self.removal_has_key.lock().expect("recording plugin lock is not poisoned").push(key.is_some());
                }
                Ok(())
            }
        }

        let program_memory = FinalizeMemory::open(StorageMode::Test(None)).unwrap();
        let finalize_store = FinalizeStore::from(program_memory).unwrap();
        assert!(!finalize_store.slipstream());

        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let removal_has_key = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut manager = SlipstreamPluginManager::new();
        manager
            .install(RecordingPlugin {
                events: std::sync::Arc::clone(&events),
                removal_has_key: std::sync::Arc::clone(&removal_has_key),
            })
            .unwrap();
        finalize_store.set_slipstream_plugin_manager(manager);
        let flush = || finalize_store.slipstream_plugin_manager().read().as_ref().unwrap().flush();

        let program_id = ProgramID::<CurrentNetwork>::from_str("hello.aleo").unwrap();
        let mapping_name = Identifier::from_str("account").unwrap();
        let key = Plaintext::from_str("123456789field").unwrap();
        let value = Value::from_str("1u64").unwrap();
        finalize_store.initialize_mapping(program_id, mapping_name).unwrap();

        // The flag is off, so a mapping write emits nothing.
        finalize_store.update_key_value(program_id, mapping_name, key.clone(), value.clone()).unwrap();
        flush();
        assert!(events.lock().unwrap().is_empty());

        // The flag is on, but this write is outside canonical finalize.
        finalize_store.set_slipstream(true);
        finalize_store.update_key_value(program_id, mapping_name, key.clone(), value.clone()).unwrap();
        flush();
        assert!(events.lock().unwrap().is_empty());

        finalize_store.is_finalize_mode().store(true, Ordering::SeqCst);
        finalize_store.block_height().store(4, Ordering::SeqCst);
        finalize_store.update_key_value(program_id, mapping_name, key.clone(), value).unwrap();

        let replacement = vec![
            (Plaintext::from_str("1field").unwrap(), Value::from_str("2u64").unwrap()),
            (Plaintext::from_str("2field").unwrap(), Value::from_str("3u64").unwrap()),
        ];
        finalize_store.replace_mapping(program_id, mapping_name, replacement).unwrap();
        assert!(
            finalize_store
                .remove_key_value(program_id, mapping_name, &Plaintext::from_str("1field").unwrap())
                .unwrap()
                .is_some()
        );
        assert!(finalize_store.remove_key_value(program_id, mapping_name, &key).unwrap().is_none());
        finalize_store.remove_mapping(program_id, mapping_name).unwrap();

        let staker = Address::<CurrentNetwork>::zero();
        let validator = Address::<CurrentNetwork>::zero();
        let mut current_stakers = IndexMap::new();
        current_stakers.insert(staker, (validator, 100));
        let mut next_stakers = IndexMap::new();
        next_stakers.insert(staker, (validator, 140));
        finalize_store.notify_staking_rewards(&current_stakers, &next_stakers, 4);

        finalize_store.is_finalize_mode().store(false, Ordering::SeqCst);
        finalize_store.notify_block(&7u32, 4);

        // Further writes stay local once the flag is cleared.
        finalize_store.set_slipstream(false);
        finalize_store.is_finalize_mode().store(true, Ordering::SeqCst);
        finalize_store.initialize_mapping(program_id, mapping_name).unwrap();
        finalize_store
            .update_key_value(
                program_id,
                mapping_name,
                Plaintext::from_str("9field").unwrap(),
                Value::from_str("9u64").unwrap(),
            )
            .unwrap();

        flush();
        let recorded = events.lock().unwrap();
        assert_eq!(recorded.as_slice(), &[
            BroadcastEventKind::MappingUpdate,
            BroadcastEventKind::MappingRemoval,
            BroadcastEventKind::MappingUpdate,
            BroadcastEventKind::MappingUpdate,
            BroadcastEventKind::MappingRemoval,
            BroadcastEventKind::MappingRemoval,
            BroadcastEventKind::StakingReward,
            BroadcastEventKind::Block,
        ]);
        assert_eq!(removal_has_key.lock().unwrap().as_slice(), &[true, true, false]);
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
}
