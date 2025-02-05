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

use crate::{
    atomic_batch_scope,
    cow_to_cloned,
    cow_to_copied,
    helpers::{Map, MapRead, NestedMap, NestedMapRead},
    program::{CommitteeStorage, CommitteeStore},
};
use console::{
    network::prelude::*,
    program::{Identifier, Plaintext, ProgramID, Value},
    types::Field,
};
use synthesizer_program::{FinalizeOperation, FinalizeStoreTrait};

use aleo_std_storage::StorageMode;
use anyhow::Result;
use core::marker::PhantomData;
use indexmap::IndexSet;

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

    /// Initializes the program state storage.
    fn open<S: Clone + Into<StorageMode>>(storage: S) -> Result<Self>;

    /// Returns the committee storage.
    fn committee_store(&self) -> &CommitteeStore<N, Self::CommitteeStorage>;
    /// Returns the program ID map.
    fn program_id_map(&self) -> &Self::ProgramIDMap;
    /// Returns the key-value map.
    fn key_value_map(&self) -> &Self::KeyValueMap;

    /// Returns the storage mode.
    fn storage_mode(&self) -> &StorageMode;

    /// Starts an atomic batch write operation.
    fn start_atomic(&self) {
        self.committee_store().start_atomic();
        self.program_id_map().start_atomic();
        self.key_value_map().start_atomic();
    }

    /// Checks if an atomic batch is in progress.
    fn is_atomic_in_progress(&self) -> bool {
        self.committee_store().is_atomic_in_progress()
            || self.program_id_map().is_atomic_in_progress()
            || self.key_value_map().is_atomic_in_progress()
    }

    /// Checkpoints the atomic batch.
    fn atomic_checkpoint(&self) {
        self.committee_store().atomic_checkpoint();
        self.program_id_map().atomic_checkpoint();
        self.key_value_map().atomic_checkpoint();
    }

    /// Clears the latest atomic batch checkpoint.
    fn clear_latest_checkpoint(&self) {
        self.committee_store().clear_latest_checkpoint();
        self.program_id_map().clear_latest_checkpoint();
        self.key_value_map().clear_latest_checkpoint();
    }

    /// Rewinds the atomic batch to the previous checkpoint.
    fn atomic_rewind(&self) {
        self.committee_store().atomic_rewind();
        self.program_id_map().atomic_rewind();
        self.key_value_map().atomic_rewind();
    }

    /// Aborts an atomic batch write operation.
    fn abort_atomic(&self) {
        self.committee_store().abort_atomic();
        self.program_id_map().abort_atomic();
        self.key_value_map().abort_atomic();
    }

    /// Finishes an atomic batch write operation.
    fn finish_atomic(&self) -> Result<()> {
        self.committee_store().finish_atomic()?;
        self.program_id_map().finish_atomic()?;
        self.key_value_map().finish_atomic()
    }

    /// Initializes the given `program ID` and `mapping name` in storage.
    /// If the `mapping name` is already initialized, an error is returned.
    fn initialize_mapping(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
    ) -> Result<FinalizeOperation<N>> {
        // Retrieve the mapping names for the program ID.
        let mut mapping_names = match self.program_id_map().get_speculative(&program_id)? {
            // If the program ID already exists, retrieve the mapping names.
            Some(mapping_names) => cow_to_cloned!(mapping_names),
            // If the program ID does not exist, initialize the mapping names.
            None => IndexSet::new(),
        };

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
        let mut mapping_names = match self.program_id_map().get_speculative(&program_id)? {
            Some(mapping_names) => cow_to_cloned!(mapping_names),
            None => bail!("Illegal operation: program ID '{program_id}' is not initialized - cannot remove mapping."),
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
        Ok(self.program_id_map().get_confirmed(program_id)?.map_or(false, |m| m.contains(mapping_name)))
    }

    /// Returns `true` if the given `program ID` and `mapping name` exist.
    fn contains_mapping_speculative(&self, program_id: &ProgramID<N>, mapping_name: &Identifier<N>) -> Result<bool> {
        Ok(self.program_id_map().get_speculative(program_id)?.map_or(false, |m| m.contains(mapping_name)))
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
        Ok(self.program_id_map().get_confirmed(program_id)?.map(|names| cow_to_cloned!(names)))
    }

    /// Returns the speculative mapping names for the given `program ID`.
    fn get_mapping_names_speculative(&self, program_id: &ProgramID<N>) -> Result<Option<IndexSet<Identifier<N>>>> {
        Ok(self.program_id_map().get_speculative(program_id)?.map(|names| cow_to_cloned!(names)))
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
        match self.key_value_map().get_value_confirmed(&(program_id, mapping_name), key)? {
            Some(value) => Ok(Some(cow_to_cloned!(value))),
            None => Ok(None),
        }
    }

    /// Returns the speculative value for the given `program ID`, `mapping name`, and `key`.
    fn get_value_speculative(
        &self,
        program_id: ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &Plaintext<N>,
    ) -> Result<Option<Value<N>>> {
        match self.key_value_map().get_value_speculative(&(program_id, mapping_name), key)? {
            Some(value) => Ok(Some(cow_to_cloned!(value))),
            None => Ok(None),
        }
    }

    /// Returns the confirmed checksum of the finalize storage.
    fn get_checksum_confirmed(&self) -> Result<Field<N>> {
        // Compute all mapping checksums.
        let preimage: std::collections::BTreeMap<_, _> = self
            .key_value_map()
            .iter_confirmed()
            .map(|(m, k, v)| {
                let m = cow_to_copied!(m);
                let k = cow_to_cloned!(k);
                let v = cow_to_cloned!(v);

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
                let m = cow_to_copied!(m);

                let mut preimage = Vec::new();
                m.write_bits_le(&mut preimage);
                false.write_bits_le(&mut preimage); // Separator.
                if let Some(k) = k {
                    cow_to_cloned!(k).write_bits_le(&mut preimage);
                }
                false.write_bits_le(&mut preimage); // Separator.

                // Compute the mapping checksum as `Hash( m || k )`.
                let mapping_checksum = N::hash_bhp1024(&preimage)?;

                if let Some(v) = v {
                    cow_to_cloned!(v).write_bits_le(&mut preimage);
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
}

impl<N: Network, P: FinalizeStorage<N>> FinalizeStore<N, P> {
    /// Initializes the finalize store.
    pub fn open<S: Clone + Into<StorageMode>>(storage: S) -> Result<Self> {
        Self::from(P::open(storage)?)
    }

    /// Initializes a finalize store from storage.
    pub fn from(storage: P) -> Result<Self> {
        // Return the finalize store.
        Ok(Self { storage, _phantom: PhantomData })
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
}

impl<N: Network, P: FinalizeStorage<N>> FinalizeStore<N, P> {
    /// Returns the committee store.
    pub fn committee_store(&self) -> &CommitteeStore<N, P::CommitteeStorage> {
        self.storage.committee_store()
    }
}

impl<N: Network, P: FinalizeStorage<N>> FinalizeStoreTrait<N> for FinalizeStore<N, P> {
    /// Returns `true` if the given `program ID` and `mapping name` exist.
    fn contains_mapping_confirmed(&self, program_id: &ProgramID<N>, mapping_name: &Identifier<N>) -> Result<bool> {
        self.storage.contains_mapping_confirmed(program_id, mapping_name)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::memory::FinalizeMemory;
    #[cfg(feature = "rocks")]
    use crate::{ConsensusStore, consensus::ConsensusStorage};
    use console::{network::MainnetV0, program::Literal, types::U64};

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
        let program_memory = FinalizeMemory::open(None).unwrap();
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
        let program_memory = FinalizeMemory::open(None).unwrap();
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
        let program_memory = FinalizeMemory::open(None).unwrap();
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
        let program_memory = FinalizeMemory::open(None).unwrap();
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
        let program_memory = FinalizeMemory::open(None).unwrap();
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
        let program_memory = FinalizeMemory::open(None).unwrap();
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
            let program_memory = FinalizeMemory::open(None).unwrap();
            FinalizeStore::from(program_memory).unwrap()
        };

        // Initialize a new finalize store; it is done via the consensus store, as that one
        // is the primary storage object, and it has a dedicated test version.
        #[cfg(feature = "rocks")]
        let consensus_store = {
            let consensus_db = crate::helpers::rocksdb::ConsensusDB::open(None).unwrap();
            ConsensusStore::from(consensus_db)
        };
        #[cfg(feature = "rocks")]
        let finalize_store = consensus_store.finalize_store();

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
            let item: u64 = rng.gen();
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
