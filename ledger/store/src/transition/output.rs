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
    helpers::{Map, MapRead},
};
use console::{
    network::prelude::*,
    program::{Ciphertext, Future, Plaintext, Record},
    types::{Field, Group},
};
use ledger_block::Output;

use aleo_std_storage::StorageMode;
use anyhow::Result;
use std::borrow::Cow;

/// A trait for transition output storage.
pub trait OutputStorage<N: Network>: Clone + Send + Sync {
    /// The mapping of `transition ID` to `output IDs`.
    type IDMap: for<'a> Map<'a, N::TransitionID, Vec<Field<N>>>;
    /// The mapping of `output ID` to `transition ID`.
    type ReverseIDMap: for<'a> Map<'a, Field<N>, N::TransitionID>;
    /// The mapping of `plaintext hash` to `(optional) plaintext`.
    type ConstantMap: for<'a> Map<'a, Field<N>, Option<Plaintext<N>>>;
    /// The mapping of `plaintext hash` to `(optional) plaintext`.
    type PublicMap: for<'a> Map<'a, Field<N>, Option<Plaintext<N>>>;
    /// The mapping of `ciphertext hash` to `(optional) ciphertext`.
    type PrivateMap: for<'a> Map<'a, Field<N>, Option<Ciphertext<N>>>;
    /// The mapping of `commitment` to `(checksum, (optional) record ciphertext)`.
    type RecordMap: for<'a> Map<'a, Field<N>, (Field<N>, Option<Record<N, Ciphertext<N>>>)>;
    /// The mapping of `record nonce` to `commitment`.
    type RecordNonceMap: for<'a> Map<'a, Group<N>, Field<N>>;
    /// The mapping of `external hash` to `()`. Note: This is **not** the record commitment.
    type ExternalRecordMap: for<'a> Map<'a, Field<N>, ()>;
    /// The mapping of `future hash` to `(optional) future`.
    type FutureMap: for<'a> Map<'a, Field<N>, Option<Future<N>>>;

    /// Initializes the transition output storage.
    fn open<S: Clone + Into<StorageMode>>(storage: S) -> Result<Self>;

    /// Returns the ID map.
    fn id_map(&self) -> &Self::IDMap;
    /// Returns the reverse ID map.
    fn reverse_id_map(&self) -> &Self::ReverseIDMap;
    /// Returns the constant map.
    fn constant_map(&self) -> &Self::ConstantMap;
    /// Returns the public map.
    fn public_map(&self) -> &Self::PublicMap;
    /// Returns the private map.
    fn private_map(&self) -> &Self::PrivateMap;
    /// Returns the record map.
    fn record_map(&self) -> &Self::RecordMap;
    /// Returns the record nonce map.
    fn record_nonce_map(&self) -> &Self::RecordNonceMap;
    /// Returns the external record map.
    fn external_record_map(&self) -> &Self::ExternalRecordMap;
    /// Returns the future map.
    fn future_map(&self) -> &Self::FutureMap;

    /// Returns the storage mode.
    fn storage_mode(&self) -> &StorageMode;

    /// Starts an atomic batch write operation.
    fn start_atomic(&self) {
        self.id_map().start_atomic();
        self.reverse_id_map().start_atomic();
        self.constant_map().start_atomic();
        self.public_map().start_atomic();
        self.private_map().start_atomic();
        self.record_map().start_atomic();
        self.record_nonce_map().start_atomic();
        self.external_record_map().start_atomic();
        self.future_map().start_atomic();
    }

    /// Checks if an atomic batch is in progress.
    fn is_atomic_in_progress(&self) -> bool {
        self.id_map().is_atomic_in_progress()
            || self.reverse_id_map().is_atomic_in_progress()
            || self.constant_map().is_atomic_in_progress()
            || self.public_map().is_atomic_in_progress()
            || self.private_map().is_atomic_in_progress()
            || self.record_map().is_atomic_in_progress()
            || self.record_nonce_map().is_atomic_in_progress()
            || self.external_record_map().is_atomic_in_progress()
            || self.future_map().is_atomic_in_progress()
    }

    /// Checkpoints the atomic batch.
    fn atomic_checkpoint(&self) {
        self.id_map().atomic_checkpoint();
        self.reverse_id_map().atomic_checkpoint();
        self.constant_map().atomic_checkpoint();
        self.public_map().atomic_checkpoint();
        self.private_map().atomic_checkpoint();
        self.record_map().atomic_checkpoint();
        self.record_nonce_map().atomic_checkpoint();
        self.external_record_map().atomic_checkpoint();
        self.future_map().atomic_checkpoint();
    }

    /// Clears the latest atomic batch checkpoint.
    fn clear_latest_checkpoint(&self) {
        self.id_map().clear_latest_checkpoint();
        self.reverse_id_map().clear_latest_checkpoint();
        self.constant_map().clear_latest_checkpoint();
        self.public_map().clear_latest_checkpoint();
        self.private_map().clear_latest_checkpoint();
        self.record_map().clear_latest_checkpoint();
        self.record_nonce_map().clear_latest_checkpoint();
        self.external_record_map().clear_latest_checkpoint();
        self.future_map().clear_latest_checkpoint();
    }

    /// Rewinds the atomic batch to the previous checkpoint.
    fn atomic_rewind(&self) {
        self.id_map().atomic_rewind();
        self.reverse_id_map().atomic_rewind();
        self.constant_map().atomic_rewind();
        self.public_map().atomic_rewind();
        self.private_map().atomic_rewind();
        self.record_map().atomic_rewind();
        self.record_nonce_map().atomic_rewind();
        self.external_record_map().atomic_rewind();
        self.future_map().atomic_rewind();
    }

    /// Aborts an atomic batch write operation.
    fn abort_atomic(&self) {
        self.id_map().abort_atomic();
        self.reverse_id_map().abort_atomic();
        self.constant_map().abort_atomic();
        self.public_map().abort_atomic();
        self.private_map().abort_atomic();
        self.record_map().abort_atomic();
        self.record_nonce_map().abort_atomic();
        self.external_record_map().abort_atomic();
        self.future_map().abort_atomic();
    }

    /// Finishes an atomic batch write operation.
    fn finish_atomic(&self) -> Result<()> {
        self.id_map().finish_atomic()?;
        self.reverse_id_map().finish_atomic()?;
        self.constant_map().finish_atomic()?;
        self.public_map().finish_atomic()?;
        self.private_map().finish_atomic()?;
        self.record_map().finish_atomic()?;
        self.record_nonce_map().finish_atomic()?;
        self.external_record_map().finish_atomic()?;
        self.future_map().finish_atomic()
    }

    /// Stores the given `(transition ID, output)` pair into storage.
    fn insert(&self, transition_id: N::TransitionID, outputs: &[Output<N>]) -> Result<()> {
        atomic_batch_scope!(self, {
            // Store the output IDs.
            self.id_map().insert(transition_id, outputs.iter().map(Output::id).copied().collect())?;

            // Store the outputs.
            for output in outputs {
                // Store the reverse output ID.
                self.reverse_id_map().insert(*output.id(), transition_id)?;
                // Store the output.
                match output.clone() {
                    Output::Constant(output_id, constant) => self.constant_map().insert(output_id, constant)?,
                    Output::Public(output_id, public) => self.public_map().insert(output_id, public)?,
                    Output::Private(output_id, private) => self.private_map().insert(output_id, private)?,
                    Output::Record(commitment, checksum, optional_record) => {
                        // If the optional record exists, insert the record nonce.
                        if let Some(record) = &optional_record {
                            self.record_nonce_map().insert(*record.nonce(), commitment)?;
                        }
                        // Insert the record entry.
                        self.record_map().insert(commitment, (checksum, optional_record))?
                    }
                    Output::ExternalRecord(output_id) => self.external_record_map().insert(output_id, ())?,
                    Output::Future(output_id, future) => self.future_map().insert(output_id, future)?,
                }
            }

            Ok(())
        })
    }

    /// Removes the output for the given `transition ID`.
    fn remove(&self, transition_id: &N::TransitionID) -> Result<()> {
        // Retrieve the output IDs.
        let output_ids: Vec<_> = match self.id_map().get_confirmed(transition_id)? {
            Some(Cow::Borrowed(ids)) => ids.to_vec(),
            Some(Cow::Owned(ids)) => ids.into_iter().collect(),
            None => return Ok(()),
        };

        atomic_batch_scope!(self, {
            // Remove the output IDs.
            self.id_map().remove(transition_id)?;

            // Remove the outputs.
            for output_id in output_ids {
                // Remove the reverse output ID.
                self.reverse_id_map().remove(&output_id)?;

                // If the output is a record, remove the record nonce.
                if let Some(record) = self.record_map().get_confirmed(&output_id)? {
                    if let Some(record) = &record.1 {
                        self.record_nonce_map().remove(record.nonce())?;
                    }
                }

                // Remove the output.
                self.constant_map().remove(&output_id)?;
                self.public_map().remove(&output_id)?;
                self.private_map().remove(&output_id)?;
                self.record_map().remove(&output_id)?;
                self.external_record_map().remove(&output_id)?;
                self.future_map().remove(&output_id)?;
            }

            Ok(())
        })
    }

    /// Returns the transition ID that contains the given `output ID`.
    fn find_transition_id(&self, output_id: &Field<N>) -> Result<Option<N::TransitionID>> {
        match self.reverse_id_map().get_confirmed(output_id)? {
            Some(Cow::Borrowed(transition_id)) => Ok(Some(*transition_id)),
            Some(Cow::Owned(transition_id)) => Ok(Some(transition_id)),
            None => Ok(None),
        }
    }

    /// Returns the output IDs for the given `transition ID`.
    fn get_ids(&self, transition_id: &N::TransitionID) -> Result<Vec<Field<N>>> {
        // Retrieve the output IDs.
        match self.id_map().get_confirmed(transition_id)? {
            Some(Cow::Borrowed(outputs)) => Ok(outputs.to_vec()),
            Some(Cow::Owned(outputs)) => Ok(outputs),
            None => Ok(vec![]),
        }
    }

    /// Returns the output for the given `transition ID`.
    fn get(&self, transition_id: &N::TransitionID) -> Result<Vec<Output<N>>> {
        // Constructs the output given the output ID and output value.
        macro_rules! into_output {
            (Output::Record($output_id:ident, $output:expr)) => {
                match $output {
                    Cow::Borrowed((checksum, opt_record)) => Output::Record($output_id, *checksum, opt_record.clone()),
                    Cow::Owned((checksum, opt_record)) => Output::Record($output_id, checksum, opt_record),
                }
            };
            (Output::$Variant:ident($output_id:ident, $output:expr)) => {
                match $output {
                    Cow::Borrowed(output) => Output::$Variant($output_id, output.clone()),
                    Cow::Owned(output) => Output::$Variant($output_id, output),
                }
            };
        }

        // A helper function to construct the output given the output ID.
        let construct_output = |output_id| {
            if let Some(constant) = self.constant_map().get_confirmed(&output_id)? {
                return Ok(into_output!(Output::Constant(output_id, constant)));
            }
            if let Some(public) = self.public_map().get_confirmed(&output_id)? {
                return Ok(into_output!(Output::Public(output_id, public)));
            }
            if let Some(private) = self.private_map().get_confirmed(&output_id)? {
                return Ok(into_output!(Output::Private(output_id, private)));
            }
            if let Some(record) = self.record_map().get_confirmed(&output_id)? {
                return Ok(into_output!(Output::Record(output_id, record)));
            }
            if self.external_record_map().get_confirmed(&output_id)?.is_some() {
                return Ok(Output::ExternalRecord(output_id));
            }
            if let Some(future) = self.future_map().get_confirmed(&output_id)? {
                return Ok(into_output!(Output::Future(output_id, future)));
            }

            bail!("Missing output '{output_id}' in transition '{transition_id}'")
        };

        // Retrieve the output IDs.
        match self.id_map().get_confirmed(transition_id)? {
            Some(Cow::Borrowed(ids)) => ids.iter().map(|output_id| construct_output(*output_id)).collect(),
            Some(Cow::Owned(ids)) => ids.iter().map(|output_id| construct_output(*output_id)).collect(),
            None => Ok(vec![]),
        }
    }
}

/// The transition output store.
#[derive(Clone)]
pub struct OutputStore<N: Network, O: OutputStorage<N>> {
    /// The map of constant outputs.
    constant: O::ConstantMap,
    /// The map of public outputs.
    public: O::PublicMap,
    /// The map of private outputs.
    private: O::PrivateMap,
    /// The map of record outputs.
    record: O::RecordMap,
    /// The map of record nonces.
    record_nonce: O::RecordNonceMap,
    /// The map of external record outputs.
    external_record: O::ExternalRecordMap,
    /// The map of future outputs.
    future: O::FutureMap,
    /// The output storage.
    storage: O,
}

impl<N: Network, O: OutputStorage<N>> OutputStore<N, O> {
    /// Initializes the transition output store.
    pub fn open<S: Clone + Into<StorageMode>>(storage: S) -> Result<Self> {
        // Initialize a new transition output storage.
        let storage = O::open(storage)?;
        // Return the transition output store.
        Ok(Self {
            constant: storage.constant_map().clone(),
            public: storage.public_map().clone(),
            private: storage.private_map().clone(),
            record: storage.record_map().clone(),
            record_nonce: storage.record_nonce_map().clone(),
            external_record: storage.external_record_map().clone(),
            future: storage.future_map().clone(),
            storage,
        })
    }

    /// Initializes a transition output store from storage.
    pub fn from(storage: O) -> Self {
        Self {
            constant: storage.constant_map().clone(),
            public: storage.public_map().clone(),
            private: storage.private_map().clone(),
            record: storage.record_map().clone(),
            record_nonce: storage.record_nonce_map().clone(),
            external_record: storage.external_record_map().clone(),
            future: storage.future_map().clone(),
            storage,
        }
    }

    /// Stores the given `(transition ID, output)` pair into storage.
    pub fn insert(&self, transition_id: N::TransitionID, outputs: &[Output<N>]) -> Result<()> {
        self.storage.insert(transition_id, outputs)
    }

    /// Removes the output for the given `transition ID`.
    pub fn remove(&self, transition_id: &N::TransitionID) -> Result<()> {
        self.storage.remove(transition_id)
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

impl<N: Network, O: OutputStorage<N>> OutputStore<N, O> {
    /// Returns the output IDs for the given `transition ID`.
    pub fn get_output_ids(&self, transition_id: &N::TransitionID) -> Result<Vec<Field<N>>> {
        self.storage.get_ids(transition_id)
    }

    /// Returns the outputs for the given `transition ID`.
    pub fn get_outputs(&self, transition_id: &N::TransitionID) -> Result<Vec<Output<N>>> {
        self.storage.get(transition_id)
    }

    /// Returns the record for the given `commitment`.
    ///
    /// If the record exists, `Ok(Some(record))` is returned.
    /// If the record was purged, `Ok(None)` is returned.
    /// If the record does not exist, `Err(error)` is returned.
    pub fn get_record(&self, commitment: &Field<N>) -> Result<Option<Record<N, Ciphertext<N>>>> {
        match self.record.get_confirmed(commitment) {
            Ok(Some(Cow::Borrowed((_, Some(record))))) => Ok(Some((*record).clone())),
            Ok(Some(Cow::Owned((_, Some(record))))) => Ok(Some(record)),
            Ok(Some(Cow::Borrowed((_, None)))) => Ok(None),
            Ok(Some(Cow::Owned((_, None)))) => Ok(None),
            Ok(None) => bail!("Record '{commitment}' not found"),
            Err(e) => Err(e),
        }
    }
}

impl<N: Network, O: OutputStorage<N>> OutputStore<N, O> {
    /// Returns the transition ID that contains the given `output ID`.
    pub fn find_transition_id(&self, output_id: &Field<N>) -> Result<Option<N::TransitionID>> {
        self.storage.find_transition_id(output_id)
    }
}

impl<N: Network, O: OutputStorage<N>> OutputStore<N, O> {
    /// Returns `true` if the given output ID exists.
    pub fn contains_output_id(&self, output_id: &Field<N>) -> Result<bool> {
        self.storage.reverse_id_map().contains_key_confirmed(output_id)
    }

    /// Returns `true` if the given commitment exists.
    pub fn contains_commitment(&self, commitment: &Field<N>) -> Result<bool> {
        self.record.contains_key_confirmed(commitment)
    }

    /// Returns `true` if the given checksum exists.
    pub fn contains_checksum(&self, checksum: &Field<N>) -> bool {
        self.checksums().contains(checksum)
    }

    /// Returns `true` if the given nonce exists.
    pub fn contains_nonce(&self, nonce: &Group<N>) -> Result<bool> {
        self.record_nonce.contains_key_confirmed(nonce)
    }
}

impl<N: Network, O: OutputStorage<N>> OutputStore<N, O> {
    /// Returns an iterator over the output IDs, for all transition outputs.
    pub fn output_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.storage.reverse_id_map().keys_confirmed()
    }

    /// Returns an iterator over the constant output IDs, for all transition outputs that are constant.
    pub fn constant_output_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.constant.keys_confirmed()
    }

    /// Returns an iterator over the public output IDs, for all transition outputs that are public.
    pub fn public_output_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.public.keys_confirmed()
    }

    /// Returns an iterator over the private output IDs, for all transition outputs that are private.
    pub fn private_output_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.private.keys_confirmed()
    }

    /// Returns an iterator over the commitments, for all transition outputs that are records.
    pub fn commitments(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.record.keys_confirmed()
    }

    /// Returns an iterator over the external record output IDs, for all transition outputs that are external records.
    pub fn external_output_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.external_record.keys_confirmed()
    }

    /// Returns an iterator over the future output IDs, for all transition outputs that are future outputs.
    pub fn future_output_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.future.keys_confirmed()
    }
}

impl<N: Network, I: OutputStorage<N>> OutputStore<N, I> {
    /// Returns an iterator over the constant outputs, for all transitions.
    pub fn constant_outputs(&self) -> impl '_ + Iterator<Item = Cow<'_, Plaintext<N>>> {
        self.constant.values_confirmed().flat_map(|output| match output {
            Cow::Borrowed(Some(output)) => Some(Cow::Borrowed(output)),
            Cow::Owned(Some(output)) => Some(Cow::Owned(output)),
            _ => None,
        })
    }

    /// Returns an iterator over the constant outputs, for all transitions.
    pub fn public_outputs(&self) -> impl '_ + Iterator<Item = Cow<'_, Plaintext<N>>> {
        self.public.values_confirmed().flat_map(|output| match output {
            Cow::Borrowed(Some(output)) => Some(Cow::Borrowed(output)),
            Cow::Owned(Some(output)) => Some(Cow::Owned(output)),
            _ => None,
        })
    }

    /// Returns an iterator over the private outputs, for all transitions.
    pub fn private_outputs(&self) -> impl '_ + Iterator<Item = Cow<'_, Ciphertext<N>>> {
        self.private.values_confirmed().flat_map(|output| match output {
            Cow::Borrowed(Some(output)) => Some(Cow::Borrowed(output)),
            Cow::Owned(Some(output)) => Some(Cow::Owned(output)),
            _ => None,
        })
    }

    /// Returns an iterator over the checksums, for all transition outputs that are records.
    pub fn checksums(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.record.values_confirmed().map(|output| match output {
            Cow::Borrowed((checksum, _)) => Cow::Borrowed(checksum),
            Cow::Owned((checksum, _)) => Cow::Owned(checksum),
        })
    }

    /// Returns an iterator over the nonces, for all transition outputs that are records.
    pub fn nonces(&self) -> impl '_ + Iterator<Item = Cow<'_, Group<N>>> {
        self.record_nonce.keys_confirmed()
    }

    /// Returns an iterator over the `(commitment, record)` pairs, for all transition outputs that are records.
    pub fn records(&self) -> impl '_ + Iterator<Item = (Cow<'_, Field<N>>, Cow<'_, Record<N, Ciphertext<N>>>)> {
        self.record.iter_confirmed().flat_map(|(commitment, output)| match output {
            Cow::Borrowed((_, Some(record))) => Some((commitment, Cow::Borrowed(record))),
            Cow::Owned((_, Some(record))) => Some((commitment, Cow::Owned(record))),
            _ => None,
        })
    }

    /// Returns an iterator over the future outputs, for all transitions.
    pub fn future_outputs(&self) -> impl '_ + Iterator<Item = Cow<'_, Future<N>>> {
        self.future.values_confirmed().flat_map(|output| match output {
            Cow::Borrowed(Some(output)) => Some(Cow::Borrowed(output)),
            Cow::Owned(Some(output)) => Some(Cow::Owned(output)),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::memory::OutputMemory;

    #[test]
    fn test_insert_get_remove() {
        // Sample the transition outputs.
        for (transition_id, output) in ledger_test_helpers::sample_outputs() {
            // Initialize a new output store.
            let output_store = OutputMemory::open(None).unwrap();

            // Ensure the transition output does not exist.
            let candidate = output_store.get(&transition_id).unwrap();
            assert!(candidate.is_empty());

            // Insert the transition output.
            output_store.insert(transition_id, &[output.clone()]).unwrap();

            // Retrieve the transition output.
            let candidate = output_store.get(&transition_id).unwrap();
            assert_eq!(vec![output.clone()], candidate);

            // Remove the transition output.
            output_store.remove(&transition_id).unwrap();

            // Retrieve the transition output.
            let candidate = output_store.get(&transition_id).unwrap();
            assert!(candidate.is_empty());
        }
    }

    #[test]
    fn test_find_transition_id() {
        // Sample the transition outputs.
        for (transition_id, output) in ledger_test_helpers::sample_outputs() {
            // Initialize a new output store.
            let output_store = OutputMemory::open(None).unwrap();

            // Ensure the transition output does not exist.
            let candidate = output_store.get(&transition_id).unwrap();
            assert!(candidate.is_empty());

            // Ensure the transition ID is not found.
            let candidate = output_store.find_transition_id(output.id()).unwrap();
            assert!(candidate.is_none());

            // Insert the transition output.
            output_store.insert(transition_id, &[output.clone()]).unwrap();

            // Find the transition ID.
            let candidate = output_store.find_transition_id(output.id()).unwrap();
            assert_eq!(Some(transition_id), candidate);

            // Remove the transition output.
            output_store.remove(&transition_id).unwrap();

            // Ensure the transition ID is not found.
            let candidate = output_store.find_transition_id(output.id()).unwrap();
            assert!(candidate.is_none());
        }
    }
}
