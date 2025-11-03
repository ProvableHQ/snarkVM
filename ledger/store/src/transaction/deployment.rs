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

use crate::{
    FeeStorage,
    FeeStore,
    atomic_batch_scope,
    helpers::{Map, MapRead},
};
use console::{
    network::prelude::*,
    program::{Identifier, ProgramID, ProgramOwner},
    types::U8,
};
use snarkvm_ledger_block::{Deployment, Fee, Transaction};
use snarkvm_synthesizer_program::Program;
use snarkvm_synthesizer_snark::{Certificate, VerifyingKey};

use aleo_std_storage::StorageMode;
use anyhow::Result;
use core::marker::PhantomData;
use std::borrow::Cow;

/// A trait for deployment storage.
/// The deployment storage contains the `Deployment`s for all programs deployed on the network.
/// The storage has been migrated a few to times to support new features.
/// Here we describe the changes made to the storage and the invariants that must hold.
/// - **ConsensusVersion::V1..V7**: The deployment edition is always zero. The `IDEditionMap` and `ChecksumMap` did not exist.
/// - **ConsensusVersion::V8**: The deployment edition is either zero or one. The `IDEditionMap` is introduced and the `EditionMap`
///   is interpreted as the latest edition for the program ID. The `ChecksumMap` did not exist.
/// - **ConsensusVersion::V9**: The deployment edition can be any value from zero to `u16::MAX`. The `ChecksumMap` is introduced and
///   stores the program checksum, required in each deployment after `ConsensusVersion::V9`.
pub trait DeploymentStorage<N: Network>: Clone + Send + Sync {
    /// The mapping of `transaction ID` to `program ID`.
    type IDMap: for<'a> Map<'a, N::TransactionID, ProgramID<N>>;
    /// The mapping of `transaction ID` to `edition`.
    /// This was introduced in `ConsensusVersion::V8`.
    type IDEditionMap: for<'a> Map<'a, N::TransactionID, u16>;
    /// The mapping of `program ID` to the **latest** `edition`.
    type EditionMap: for<'a> Map<'a, ProgramID<N>, u16>;
    /// The mapping of `(program ID, edition)` to `transaction ID`.
    type ReverseIDMap: for<'a> Map<'a, (ProgramID<N>, u16), N::TransactionID>;
    /// The mapping of `(program ID, edition)` to `ProgramOwner`.
    type OwnerMap: for<'a> Map<'a, (ProgramID<N>, u16), ProgramOwner<N>>;
    /// The mapping of `(program ID, edition)` to `program`.
    type ProgramMap: for<'a> Map<'a, (ProgramID<N>, u16), Program<N>>;
    /// The mapping of `(program ID, edition)` to `checksum`.
    /// This was introduced in `ConsensusVersion::V9`.
    type ChecksumMap: for<'a> Map<'a, (ProgramID<N>, u16), [U8<N>; 32]>;
    /// The mapping of `(program ID, function name, edition)` to `verifying key`.
    type VerifyingKeyMap: for<'a> Map<'a, (ProgramID<N>, Identifier<N>, u16), VerifyingKey<N>>;
    /// The mapping of `(program ID, function name, edition)` to `certificate`.
    type CertificateMap: for<'a> Map<'a, (ProgramID<N>, Identifier<N>, u16), Certificate<N>>;
    /// The fee storage.
    type FeeStorage: FeeStorage<N>;

    /// Initializes the deployment storage.
    fn open(fee_store: FeeStore<N, Self::FeeStorage>) -> Result<Self>;

    /// Returns the ID map.
    fn id_map(&self) -> &Self::IDMap;
    /// Returns the ID edition map.
    fn id_edition_map(&self) -> &Self::IDEditionMap;
    /// Returns the edition map.
    fn edition_map(&self) -> &Self::EditionMap;
    /// Returns the reverse ID map.
    fn reverse_id_map(&self) -> &Self::ReverseIDMap;
    /// Returns the owner map.
    fn owner_map(&self) -> &Self::OwnerMap;
    /// Returns the program map.
    fn program_map(&self) -> &Self::ProgramMap;
    /// Returns the checksum map.
    fn checksum_map(&self) -> &Self::ChecksumMap;
    /// Returns the verifying key map.
    fn verifying_key_map(&self) -> &Self::VerifyingKeyMap;
    /// Returns the certificate map.
    fn certificate_map(&self) -> &Self::CertificateMap;
    /// Returns the fee storage.
    fn fee_store(&self) -> &FeeStore<N, Self::FeeStorage>;

    /// Returns the storage mode.
    fn storage_mode(&self) -> &StorageMode {
        self.fee_store().storage_mode()
    }

    /// Starts an atomic batch write operation.
    fn start_atomic(&self) {
        self.id_map().start_atomic();
        self.id_edition_map().start_atomic();
        self.edition_map().start_atomic();
        self.reverse_id_map().start_atomic();
        self.owner_map().start_atomic();
        self.program_map().start_atomic();
        self.checksum_map().start_atomic();
        self.verifying_key_map().start_atomic();
        self.certificate_map().start_atomic();
        self.fee_store().start_atomic();
    }

    /// Checks if an atomic batch is in progress.
    fn is_atomic_in_progress(&self) -> bool {
        self.id_map().is_atomic_in_progress()
            || self.id_edition_map().is_atomic_in_progress()
            || self.edition_map().is_atomic_in_progress()
            || self.reverse_id_map().is_atomic_in_progress()
            || self.owner_map().is_atomic_in_progress()
            || self.program_map().is_atomic_in_progress()
            || self.checksum_map().is_atomic_in_progress()
            || self.verifying_key_map().is_atomic_in_progress()
            || self.certificate_map().is_atomic_in_progress()
            || self.fee_store().is_atomic_in_progress()
    }

    /// Checkpoints the atomic batch.
    fn atomic_checkpoint(&self) {
        self.id_map().atomic_checkpoint();
        self.id_edition_map().atomic_checkpoint();
        self.edition_map().atomic_checkpoint();
        self.reverse_id_map().atomic_checkpoint();
        self.owner_map().atomic_checkpoint();
        self.program_map().atomic_checkpoint();
        self.checksum_map().atomic_checkpoint();
        self.verifying_key_map().atomic_checkpoint();
        self.certificate_map().atomic_checkpoint();
        self.fee_store().atomic_checkpoint();
    }

    /// Clears the latest atomic batch checkpoint.
    fn clear_latest_checkpoint(&self) {
        self.id_map().clear_latest_checkpoint();
        self.id_edition_map().clear_latest_checkpoint();
        self.edition_map().clear_latest_checkpoint();
        self.reverse_id_map().clear_latest_checkpoint();
        self.owner_map().clear_latest_checkpoint();
        self.program_map().clear_latest_checkpoint();
        self.checksum_map().clear_latest_checkpoint();
        self.verifying_key_map().clear_latest_checkpoint();
        self.certificate_map().clear_latest_checkpoint();
        self.fee_store().clear_latest_checkpoint();
    }

    /// Rewinds the atomic batch to the previous checkpoint.
    fn atomic_rewind(&self) {
        self.id_map().atomic_rewind();
        self.id_edition_map().atomic_rewind();
        self.edition_map().atomic_rewind();
        self.reverse_id_map().atomic_rewind();
        self.owner_map().atomic_rewind();
        self.program_map().atomic_rewind();
        self.checksum_map().atomic_rewind();
        self.verifying_key_map().atomic_rewind();
        self.certificate_map().atomic_rewind();
        self.fee_store().atomic_rewind();
    }

    /// Aborts an atomic batch write operation.
    fn abort_atomic(&self) {
        self.id_map().abort_atomic();
        self.id_edition_map().abort_atomic();
        self.edition_map().abort_atomic();
        self.reverse_id_map().abort_atomic();
        self.owner_map().abort_atomic();
        self.program_map().abort_atomic();
        self.checksum_map().abort_atomic();
        self.verifying_key_map().abort_atomic();
        self.certificate_map().abort_atomic();
        self.fee_store().abort_atomic();
    }

    /// Finishes an atomic batch write operation.
    fn finish_atomic(&self) -> Result<()> {
        self.id_map().finish_atomic()?;
        self.id_edition_map().finish_atomic()?;
        self.edition_map().finish_atomic()?;
        self.reverse_id_map().finish_atomic()?;
        self.owner_map().finish_atomic()?;
        self.program_map().finish_atomic()?;
        self.checksum_map().finish_atomic()?;
        self.verifying_key_map().finish_atomic()?;
        self.certificate_map().finish_atomic()?;
        self.fee_store().finish_atomic()
    }

    /// Stores the given `deployment transaction` pair into storage.
    fn insert(&self, transaction: &Transaction<N>) -> Result<()> {
        // Ensure the transaction is a deployment.
        let (transaction_id, owner, deployment, fee) = match transaction {
            Transaction::Deploy(transaction_id, _, owner, deployment, fee) => (transaction_id, owner, deployment, fee),
            Transaction::Execute(..) => bail!("Attempted to insert an execute transaction into deployment storage."),
            Transaction::Fee(..) => bail!("Attempted to insert fee transaction into deployment storage."),
        };

        // Ensure the deployment is ordered.
        if let Err(error) = deployment.check_is_ordered() {
            bail!("Failed to insert malformed deployment transaction: {error}")
        }

        // Retrieve the edition.
        // Note: The VM enforces that the edition is always 0 for the first deployment of a program and that subsequent upgrades increment the edition.
        let edition = deployment.edition();
        // Retrieve the program.
        let program = deployment.program();
        // Retrieve the program ID.
        let program_id = *program.id();
        // Retrieve the checksum.
        let checksum = deployment.program_checksum();

        // If the deployment edition is greater than zero, then ensure that it increments the latest edition for the program ID.
        let expected_edition = match self.get_latest_edition_for_program(&program_id)? {
            Some(latest_edition) => latest_edition.saturating_add(1),
            None => 0,
        };
        ensure!(
            edition == expected_edition,
            "Failed to insert deployment transaction '{transaction_id}' for program '{program_id}', expected edition {expected_edition}, found edition {edition}"
        );

        atomic_batch_scope!(self, {
            // Store the program ID.
            self.id_map().insert(*transaction_id, program_id)?;
            // Store the latest edition for the program ID.
            self.edition_map().insert(program_id, edition)?;

            // Store the reverse program ID.
            self.reverse_id_map().insert((program_id, edition), *transaction_id)?;
            // Store the owner.
            self.owner_map().insert((program_id, edition), *owner)?;
            // Store the program.
            self.program_map().insert((program_id, edition), program.clone())?;

            // Store the edition in the ID edition map.
            // Note: Prior to `ConsensusVersion::V8`, the edition is always zero.
            //  `ConsensusVersion::V8` allows the edition to be one via a one-time redeployment.
            //  `ConsensusVersion::V9` introduces upgradability which allows editions to be incremented up to `u16::MAX`
            self.id_edition_map().insert(*transaction_id, edition)?;

            // If the checksum exists, then store it into the `ChecksumMap`.
            if let Some(checksum) = checksum {
                self.checksum_map().insert((program_id, edition), checksum)?;
            }

            // Store the verifying keys and certificates.
            for (function_name, (verifying_key, certificate)) in deployment.verifying_keys() {
                // Store the verifying key.
                self.verifying_key_map().insert((program_id, *function_name, edition), verifying_key.clone())?;
                // Store the certificate.
                self.certificate_map().insert((program_id, *function_name, edition), certificate.clone())?;
            }

            // Store the fee transition.
            self.fee_store().insert(*transaction_id, fee)?;

            Ok(())
        })
    }

    /// Removes the deployment transaction for the given `transaction ID`.
    fn remove(&self, transaction_id: &N::TransactionID) -> Result<()> {
        // Retrieve the program ID for the transaction ID.
        let Some(program_id) = self.get_program_id(transaction_id)? else {
            bail!("Failed to get the program ID for transaction '{transaction_id}'");
        };
        // Retrieve the edition for the transaction ID.
        let Some(edition) = self.get_edition_for_transaction(transaction_id)? else {
            bail!("Failed to locate the edition for transaction '{transaction_id}'");
        };
        // Retrieve the latest edition for the program ID.
        let Some(latest_edition) = self.get_latest_edition_for_program(&program_id)? else {
            bail!("Failed to locate the latest edition for program '{program_id}'");
        };
        // Verify that the removed edition is latest edition.
        // Note: This is condition should always hold true because:
        //  - The VM enforces that exactly one deployment or upgrade is allowed per program per block.
        //  - The only time a transaction is removed is when `remove_last_n` is invoked.
        //  - `remove_last_n` is only invoked when finalization for the latest block fails.
        //  - `remove_last_n` is only invoked with the parameter `1`.
        // If any of these conditions are changed, then this check is no longer valid.
        ensure!(
            edition == latest_edition,
            "Failed to remove the deployment for transaction '{transaction_id}' because it is not the latest edition"
        );
        // Retrieve the program.
        let Some(program) = self.program_map().get_confirmed(&(program_id, edition))?.map(|x| x.into_owned()) else {
            bail!("Failed to locate program '{program_id}' for transaction '{transaction_id}'");
        };

        atomic_batch_scope!(self, {
            // Remove the program ID.
            self.id_map().remove(transaction_id)?;
            // Remove the edition for the transaction ID.
            self.id_edition_map().remove(transaction_id)?;
            // Update the latest edition.
            match edition.is_zero() {
                // If the removed edition is 0, then remove the program ID from the latest edition map.
                true => self.edition_map().remove(&program_id)?,
                // Otherwise, decrement the edition.
                // Note: This is safe because the VM enforces that the edition is always incremented.
                false => self.edition_map().insert(program_id, edition.saturating_sub(1))?,
            }

            // Remove the reverse program ID.
            self.reverse_id_map().remove(&(program_id, edition))?;
            // Remove the owner.
            self.owner_map().remove(&(program_id, edition))?;
            // Remove the program.
            self.program_map().remove(&(program_id, edition))?;
            // Remove the checksum.
            self.checksum_map().remove(&(program_id, edition))?;

            // Remove the verifying keys and certificates.
            for function_name in program.functions().keys() {
                // Remove the verifying key.
                self.verifying_key_map().remove(&(program_id, *function_name, edition))?;
                // Remove the certificate.
                self.certificate_map().remove(&(program_id, *function_name, edition))?;
            }

            // Remove the fee transition.
            self.fee_store().remove(transaction_id)?;

            Ok(())
        })
    }

    /// Returns the latest transaction ID that contains the given `program ID`.
    fn find_latest_transaction_id_from_program_id(
        &self,
        program_id: &ProgramID<N>,
    ) -> Result<Option<N::TransactionID>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            return Ok(None);
        }

        // Retrieve the latest edition.
        let Some(edition) = self.get_latest_edition_for_program(program_id)? else {
            return Ok(None);
        };
        // Retrieve the transaction ID.
        let Some(transaction_id) = self.reverse_id_map().get_confirmed(&(*program_id, edition))? else {
            bail!("Failed to find the transaction ID for program '{program_id}' (edition {edition})");
        };
        Ok(Some(*transaction_id))
    }

    /// Returns the transaction ID that contains the given `program ID` and `edition`.
    fn find_transaction_id_from_program_id_and_edition(
        &self,
        program_id: &ProgramID<N>,
        edition: u16,
    ) -> Result<Option<N::TransactionID>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            return Ok(None);
        }
        // Retrieve the transaction ID.
        match self.reverse_id_map().get_confirmed(&(*program_id, edition))? {
            Some(transaction_id) => Ok(Some(*transaction_id)),
            None => Ok(None),
        }
    }

    /// Returns the transaction ID that contains the given `transition ID`.
    fn find_transaction_id_from_transition_id(
        &self,
        transition_id: &N::TransitionID,
    ) -> Result<Option<N::TransactionID>> {
        self.fee_store().find_transaction_id_from_transition_id(transition_id)
    }

    /// Returns the program ID for the given `transaction ID`.
    fn get_program_id(&self, transaction_id: &N::TransactionID) -> Result<Option<ProgramID<N>>> {
        // Retrieve the program ID.
        Ok(self.id_map().get_confirmed(transaction_id)?.map(|x| *x))
    }

    /// Returns the latest edition for the given `program ID`.
    fn get_latest_edition_for_program(&self, program_id: &ProgramID<N>) -> Result<Option<u16>> {
        Ok(self.edition_map().get_confirmed(program_id)?.map(|x| *x))
    }

    /// Returns the edition for the given `transaction ID`.
    fn get_edition_for_transaction(&self, transaction_id: &N::TransactionID) -> Result<Option<u16>> {
        // Retrieve the edition.
        match self.id_edition_map().get_confirmed(transaction_id)? {
            Some(edition) => Ok(Some(*edition)),
            None => {
                // Check if the program exists in the store.
                match self.get_program_id(transaction_id)?.is_none() {
                    true => Ok(None),
                    // If a program is not in the `IDEditionMap` but exists in the store,
                    // then it must have been deployed prior to `ConsensusVersion::V8` when editions were exclusively zero.
                    false => Ok(Some(0)),
                }
            }
        }
    }

    /// Returns the latest program for the given `program ID`.
    fn get_latest_program(&self, program_id: &ProgramID<N>) -> Result<Option<Program<N>>> {
        // Retrieve the latest edition.
        let edition = match self.get_latest_edition_for_program(program_id)? {
            Some(edition) => edition,
            None => return Ok(None),
        };
        // Retrieve the program.
        let Some(program) = self.program_map().get_confirmed(&(*program_id, edition))? else {
            bail!("Failed to get program '{program_id}' (edition {edition})");
        };
        Ok(Some(program.into_owned()))
    }

    /// Returns the program for the given `program ID` and `edition`.
    fn get_program_for_edition(&self, program_id: &ProgramID<N>, edition: u16) -> Result<Option<Program<N>>> {
        self.program_map().get_confirmed(&(*program_id, edition)).map(|p| p.map(|p| p.into_owned()))
    }

    /// Returns the latest verifying key for the given `program ID` and `function name`.
    fn get_latest_verifying_key(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
    ) -> Result<Option<VerifyingKey<N>>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            // Load the verifying key.
            let verifying_key = N::get_credits_verifying_key(function_name.to_string())?;
            // Retrieve the number of public and private variables.
            // Note: This number does *NOT* include the number of constants. This is safe because
            // this program is never deployed, as it is a first-class citizen of the protocol.
            let num_variables = verifying_key.circuit_info.num_public_and_private_variables as u64;
            // Return the verifying key.
            return Ok(Some(VerifyingKey::new(verifying_key.clone(), num_variables)));
        }

        // Retrieve the latest edition.
        let Some(edition) = self.get_latest_edition_for_program(program_id)? else {
            return Ok(None);
        };
        // Retrieve the verifying key.
        let Some(verifying_key) = self.verifying_key_map().get_confirmed(&(*program_id, *function_name, edition))?
        else {
            bail!("Failed to get the verifying key for '{program_id}/{function_name}' (edition {edition})");
        };
        Ok(Some(verifying_key.into_owned()))
    }

    /// Returns the verifying key for the given `program ID`, `function name` and `edition`.
    fn get_verifying_key_with_edition(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        edition: u16,
    ) -> Result<Option<VerifyingKey<N>>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            // Load the verifying key.
            let verifying_key = N::get_credits_verifying_key(function_name.to_string())?;
            // Retrieve the number of public and private variables.
            // Note: This number does *NOT* include the number of constants. This is safe because
            // this program is never deployed, as it is a first-class citizen of the protocol.
            let num_variables = verifying_key.circuit_info.num_public_and_private_variables as u64;
            // Return the verifying key.
            return Ok(Some(VerifyingKey::new(verifying_key.clone(), num_variables)));
        }

        // Retrieve the verifying key.
        match self.verifying_key_map().get_confirmed(&(*program_id, *function_name, edition))? {
            Some(verifying_key) => Ok(Some(verifying_key.into_owned())),
            None => bail!("Failed to get the verifying key for '{program_id}/{function_name}' (edition {edition})"),
        }
    }

    /// Returns the latest certificate for the given `program ID` and `function name`.
    fn get_latest_certificate(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
    ) -> Result<Option<Certificate<N>>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            return Ok(None);
        }

        // Retrieve the latest edition.
        let Some(edition) = self.get_latest_edition_for_program(program_id)? else {
            return Ok(None);
        };
        // Retrieve the certificate.
        let Some(certificate) = self.certificate_map().get_confirmed(&(*program_id, *function_name, edition))? else {
            bail!("Failed to get the certificate for '{program_id}/{function_name}' (edition {edition})");
        };
        Ok(Some(certificate.into_owned()))
    }

    /// Returns the certificate for the given `program ID`, `function name`, and `edition`.
    fn get_certificate_with_edition(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        edition: u16,
    ) -> Result<Option<Certificate<N>>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            return Ok(None);
        }

        // Retrieve the certificate.
        match self.certificate_map().get_confirmed(&(*program_id, *function_name, edition))? {
            Some(certificate) => Ok(Some(certificate.into_owned())),
            None => bail!("Failed to get the certificate for '{program_id}/{function_name}' (edition {edition})"),
        }
    }

    /// Returns the deployment for the given `transaction ID`.
    fn get_deployment(&self, transaction_id: &N::TransactionID) -> Result<Option<Deployment<N>>> {
        // Retrieve the program ID.
        let Some(program_id) = self.get_program_id(transaction_id)? else {
            return Ok(None);
        };
        // Retrieve the edition.
        let Some(edition) = self.get_edition_for_transaction(transaction_id)? else {
            bail!("Failed to get the edition for program '{program_id}'");
        };
        // Retrieve the program.
        let Some(program) = self.program_map().get_confirmed(&(program_id, edition))?.map(|x| x.into_owned()) else {
            bail!("Failed to get the deployed program '{program_id}' (edition {edition})");
        };
        // Retrieve the checksum.
        let program_checksum =
            self.checksum_map().get_confirmed(&(program_id, edition))?.map(|checksum| checksum.into_owned());
        // If the checksum is present, then retrieve the owner address.
        // Note: This is done to ensure that `Deployment` is consistent. Both the checksum and owner must be present or absent.
        // This invariant is also enforced in `check_transaction`.
        let program_owner = match program_checksum.is_some() {
            false => None,
            true => match self.owner_map().get_confirmed(&(program_id, edition))? {
                Some(owner) => Some(owner.address()),
                None => bail!("Failed to get the owner for program '{program_id}' (edition {edition})"),
            },
        };

        // Initialize a vector for the verifying keys and certificates.
        let mut verifying_keys = Vec::with_capacity(program.functions().len());

        // Retrieve the verifying keys and certificates.
        for function_name in program.functions().keys() {
            // Retrieve the verifying key.
            let Some(verifying_key) =
                self.verifying_key_map().get_confirmed(&(program_id, *function_name, edition))?.map(|x| x.into_owned())
            else {
                bail!("Failed to get the verifying key for '{program_id}/{function_name}' (edition {edition})");
            };
            // Retrieve the certificate.
            let Some(certificate) =
                self.certificate_map().get_confirmed(&(program_id, *function_name, edition))?.map(|x| x.into_owned())
            else {
                bail!("Failed to get the certificate for '{program_id}/{function_name}' (edition {edition})");
            };
            // Add the verifying key and certificate to the deployment.
            verifying_keys.push((*function_name, (verifying_key, certificate)));
        }

        // Return the deployment.
        Ok(Some(Deployment::new(edition, program, verifying_keys, program_checksum, program_owner)?))
    }

    /// Returns the fee for the given `transaction ID`.
    fn get_fee(&self, transaction_id: &N::TransactionID) -> Result<Option<Fee<N>>> {
        self.fee_store().get_fee(transaction_id)
    }

    /// Returns the latest owner for the given `program ID`.
    fn get_latest_owner(&self, program_id: &ProgramID<N>) -> Result<Option<ProgramOwner<N>>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            return Ok(None);
        }

        // TODO (raychu86): Consider program upgrades and edition changes.
        // Retrieve the latest edition.
        let Some(edition) = self.get_latest_edition_for_program(program_id)? else {
            return Ok(None);
        };

        // Retrieve the owner.
        let Some(owner) = self.owner_map().get_confirmed(&(*program_id, edition))? else {
            bail!("Failed to find the Owner for program '{program_id}' (edition {edition})");
        };
        Ok(Some(*owner))
    }

    /// Returns the owner for the given `program ID` and `edition`.
    fn get_owner_with_edition(&self, program_id: &ProgramID<N>, edition: u16) -> Result<Option<ProgramOwner<N>>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            return Ok(None);
        }

        // Retrieve the owner.
        match self.owner_map().get_confirmed(&(*program_id, edition))? {
            Some(owner) => Ok(Some(*owner)),
            None => bail!("Failed to find the Owner for program '{program_id}' (edition {edition})"),
        }
    }

    /// Returns the transaction for the given `transaction ID`.
    fn get_transaction(&self, transaction_id: &N::TransactionID) -> Result<Option<Transaction<N>>> {
        // Retrieve the deployment.
        let Some(deployment) = self.get_deployment(transaction_id)? else {
            return Ok(None);
        };
        // Retrieve the fee.
        let Some(fee) = self.get_fee(transaction_id)? else {
            bail!("Failed to get the fee for transaction '{transaction_id}'");
        };

        // Retrieve the owner.
        let Some(owner) = self.get_owner_with_edition(deployment.program_id(), deployment.edition())? else {
            bail!("Failed to get the owner for transaction '{transaction_id}'");
        };

        // Construct the deployment transaction.
        let deployment_transaction = Transaction::from_deployment(owner, deployment, fee)?;
        // Ensure the transaction ID matches.
        match *transaction_id == deployment_transaction.id() {
            true => Ok(Some(deployment_transaction)),
            false => bail!("The deployment transaction ID does not match '{transaction_id}'"),
        }
    }
}

/// The deployment store.
#[derive(Clone)]
pub struct DeploymentStore<N: Network, D: DeploymentStorage<N>> {
    /// The deployment storage.
    storage: D,
    /// PhantomData.
    _phantom: PhantomData<N>,
}

impl<N: Network, D: DeploymentStorage<N>> DeploymentStore<N, D> {
    /// Initializes the deployment store.
    pub fn open(fee_store: FeeStore<N, D::FeeStorage>) -> Result<Self> {
        // Initialize the deployment storage.
        let storage = D::open(fee_store)?;

        // Insert `credits.aleo`, which is the default program.
        let credits_id = ProgramID::from_str("credits.aleo")?;
        storage.edition_map().insert(credits_id, 0)?;
        storage.program_map().insert((credits_id, 0), Program::credits()?)?;

        // Return the deployment store.
        Ok(Self { storage, _phantom: PhantomData })
    }

    /// Initializes a deployment store from storage.
    pub fn from(storage: D) -> Self {
        Self { storage, _phantom: PhantomData }
    }

    /// Stores the given `deployment transaction` into storage.
    pub fn insert(&self, transaction: &Transaction<N>) -> Result<()> {
        self.storage.insert(transaction)
    }

    /// Removes the transaction for the given `transaction ID`.
    pub fn remove(&self, transaction_id: &N::TransactionID) -> Result<()> {
        self.storage.remove(transaction_id)
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

impl<N: Network, D: DeploymentStorage<N>> DeploymentStore<N, D> {
    /// Returns the transaction for the given `transaction ID`.
    pub fn get_transaction(&self, transaction_id: &N::TransactionID) -> Result<Option<Transaction<N>>> {
        self.storage.get_transaction(transaction_id)
    }

    /// Returns the deployment for the given `transaction ID`.
    pub fn get_deployment(&self, transaction_id: &N::TransactionID) -> Result<Option<Deployment<N>>> {
        self.storage.get_deployment(transaction_id)
    }

    /// Returns the latest edition for the given `program ID`.
    pub fn get_latest_edition_for_program(&self, program_id: &ProgramID<N>) -> Result<Option<u16>> {
        self.storage.get_latest_edition_for_program(program_id)
    }

    /// Returns the edition for the given `transaction ID`.
    pub fn get_edition_for_transaction(&self, transaction_id: &N::TransactionID) -> Result<Option<u16>> {
        self.storage.get_edition_for_transaction(transaction_id)
    }

    /// Returns the program ID for the given `transaction ID`.
    pub fn get_program_id(&self, transaction_id: &N::TransactionID) -> Result<Option<ProgramID<N>>> {
        self.storage.get_program_id(transaction_id)
    }

    /// Returns the latest program for the given `program ID`.
    pub fn get_latest_program(&self, program_id: &ProgramID<N>) -> Result<Option<Program<N>>> {
        self.storage.get_latest_program(program_id)
    }

    /// Returns the program for the given `program ID` and `edition`.
    pub fn get_program_for_edition(&self, program_id: &ProgramID<N>, edition: u16) -> Result<Option<Program<N>>> {
        self.storage.get_program_for_edition(program_id, edition)
    }

    /// Returns the latest verifying key for the given `(program ID, function name)`.
    pub fn get_latest_verifying_key(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
    ) -> Result<Option<VerifyingKey<N>>> {
        self.storage.get_latest_verifying_key(program_id, function_name)
    }

    /// Returns the verifying key for the given `(program ID, function name, edition)`.
    pub fn get_verifying_key_with_edition(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        edition: u16,
    ) -> Result<Option<VerifyingKey<N>>> {
        self.storage.get_verifying_key_with_edition(program_id, function_name, edition)
    }

    /// Returns the latest certificate for the given `(program ID, function name)`.
    pub fn get_latest_certificate(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
    ) -> Result<Option<Certificate<N>>> {
        self.storage.get_latest_certificate(program_id, function_name)
    }

    /// Returns the certificate for the given `(program ID, function name, edition)`.
    pub fn get_certificate_with_edition(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        edition: u16,
    ) -> Result<Option<Certificate<N>>> {
        self.storage.get_certificate_with_edition(program_id, function_name, edition)
    }

    /// Returns the fee for the given `transaction ID`.
    pub fn get_fee(&self, transaction_id: &N::TransactionID) -> Result<Option<Fee<N>>> {
        self.storage.get_fee(transaction_id)
    }

    /// Returns the latest owner for the given `program ID`.
    pub fn get_latest_owner(&self, program_id: &ProgramID<N>) -> Result<Option<ProgramOwner<N>>> {
        self.storage.get_latest_owner(program_id)
    }

    /// Returns the owner for the given `program ID` and `edition`.
    pub fn get_owner_with_edition(&self, program_id: &ProgramID<N>, edition: u16) -> Result<Option<ProgramOwner<N>>> {
        self.storage.get_owner_with_edition(program_id, edition)
    }
}

impl<N: Network, D: DeploymentStorage<N>> DeploymentStore<N, D> {
    /// Returns the latest transaction ID that deployed or upgraded the given `program ID`.
    pub fn find_latest_transaction_id_from_program_id(
        &self,
        program_id: &ProgramID<N>,
    ) -> Result<Option<N::TransactionID>> {
        self.storage.find_latest_transaction_id_from_program_id(program_id)
    }

    /// Returns the transaction `ID` that deployed the given `program ID` and `edition`.
    pub fn find_transaction_id_from_program_id_and_edition(
        &self,
        program_id: &ProgramID<N>,
        edition: u16,
    ) -> Result<Option<N::TransactionID>> {
        self.storage.find_transaction_id_from_program_id_and_edition(program_id, edition)
    }

    /// Returns the transaction ID that deployed the given `transition ID`.
    pub fn find_transaction_id_from_transition_id(
        &self,
        transition_id: &N::TransitionID,
    ) -> Result<Option<N::TransactionID>> {
        self.storage.find_transaction_id_from_transition_id(transition_id)
    }
}

impl<N: Network, D: DeploymentStorage<N>> DeploymentStore<N, D> {
    /// Returns `true` if the given program ID exists.
    pub fn contains_program_id(&self, program_id: &ProgramID<N>) -> Result<bool> {
        self.storage.edition_map().contains_key_confirmed(program_id)
    }

    /// Returns `true` if the given program ID and edition exist.
    pub fn contains_program_id_and_edition(&self, program_id: &ProgramID<N>, edition: u16) -> Result<bool> {
        self.storage.reverse_id_map().contains_key_confirmed(&(*program_id, edition))
    }
}

type ProgramIDEdition<N> = (ProgramID<N>, u16);
type ProgramTriplet<N> = (ProgramID<N>, Identifier<N>, u16);

impl<N: Network, D: DeploymentStorage<N>> DeploymentStore<N, D> {
    /// Returns an iterator over the deployment transaction IDs, for all deployments.
    pub fn deployment_transaction_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, N::TransactionID>> {
        self.storage.id_map().keys_confirmed()
    }

    /// Returns an iterator over the program IDs, for all deployments.
    /// Note: If a program upgraded, this method will return duplicates of the program ID.
    pub fn program_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, ProgramID<N>>> {
        self.storage.id_map().values_confirmed().map(|id| match id {
            Cow::Borrowed(id) => Cow::Borrowed(id),
            Cow::Owned(id) => Cow::Owned(id),
        })
    }

    /// Returns an iterator over the program IDs and latest editions.
    pub fn program_ids_and_latest_editions(&self) -> impl '_ + Iterator<Item = (Cow<'_, ProgramID<N>>, Cow<'_, u16>)> {
        self.storage.edition_map().iter_confirmed()
    }

    /// Returns an iterator over the programs, for all deployments.
    /// If a program has been upgraded, all instances of the program will be returned.
    pub fn programs(&self) -> impl '_ + Iterator<Item = Cow<'_, Program<N>>> {
        self.storage.program_map().values_confirmed().map(|program| match program {
            Cow::Borrowed(program) => Cow::Borrowed(program),
            Cow::Owned(program) => Cow::Owned(program),
        })
    }

    /// Returns an iterator over the programs and editions, for all deployments.
    pub fn programs_with_editions(
        &self,
    ) -> impl '_ + Iterator<Item = (Cow<'_, ProgramIDEdition<N>>, Cow<'_, Program<N>>)> {
        self.storage.program_map().iter_confirmed()
    }

    /// Returns an iterator over the `((program ID, function name, edition), verifying key)`, for all deployments.
    pub fn verifying_keys(&self) -> impl '_ + Iterator<Item = (Cow<'_, ProgramTriplet<N>>, Cow<'_, VerifyingKey<N>>)> {
        self.storage.verifying_key_map().iter_confirmed()
    }

    /// Returns an iterator over the `((program ID, function name, edition), certificate)`, for all deployments.
    pub fn certificates(&self) -> impl '_ + Iterator<Item = (Cow<'_, ProgramTriplet<N>>, Cow<'_, Certificate<N>>)> {
        self.storage.certificate_map().iter_confirmed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TransitionStore, helpers::memory::DeploymentMemory};

    #[test]
    fn test_insert_get_remove() {
        let rng = &mut TestRng::default();

        // Initialize a new transition store.
        let transition_store = TransitionStore::open(StorageMode::Test(None)).unwrap();
        // Initialize a new fee store.
        let fee_store = FeeStore::open(transition_store).unwrap();
        // Initialize a new deployment store.
        let deployment_store = DeploymentMemory::open(fee_store).unwrap();

        // Sample the transactions.
        let transaction_0 = snarkvm_ledger_test_helpers::sample_deployment_transaction(1, 0, true, rng);
        let transaction_1 = snarkvm_ledger_test_helpers::sample_deployment_transaction(1, 1, false, rng);
        let transaction_2 = snarkvm_ledger_test_helpers::sample_deployment_transaction(2, 0, true, rng);
        let transaction_3 = snarkvm_ledger_test_helpers::sample_deployment_transaction(2, 1, false, rng);
        let transaction_4 = snarkvm_ledger_test_helpers::sample_deployment_transaction(2, 2, true, rng);
        let transactions = vec![transaction_0, transaction_1, transaction_2, transaction_3, transaction_4];

        for transaction in transactions {
            let transaction_id = transaction.id();
            let program_id = *transaction.deployment().unwrap().program_id();
            let checksum = transaction.deployment().unwrap().program_checksum();
            let edition = transaction.deployment().unwrap().edition();

            // Ensure the deployment transaction does not exist.
            let candidate = deployment_store.get_transaction(&transaction_id).unwrap();
            assert_eq!(None, candidate);

            // Insert the deployment transaction.
            deployment_store.insert(&transaction).unwrap();

            // If the deployment has a checksum, then check that it exists in the checksum map.
            match checksum {
                Some(checksum) => {
                    let candidate = deployment_store.checksum_map().get_confirmed(&(program_id, edition)).unwrap();
                    assert_eq!(Some(checksum), candidate.map(|c| c.into_owned()));
                }
                None => {
                    let candidate = deployment_store.checksum_map().get_confirmed(&(program_id, edition)).unwrap();
                    assert_eq!(None, candidate);
                }
            }

            // Check that the transaction exists in the ID edition map
            let candidate = deployment_store.id_edition_map().get_confirmed(&transaction_id).unwrap();
            assert_eq!(Some(edition), candidate.map(|e| *e));

            // Retrieve the deployment transaction.
            let candidate = deployment_store.get_transaction(&transaction_id).unwrap();
            assert_eq!(Some(transaction.clone()), candidate);

            // Retrieve the edition for the transaction and verify that it is matches.
            let actual = deployment_store.get_edition_for_transaction(&transaction_id).unwrap();
            assert_eq!(Some(edition), actual);

            // Retrieve the latest edition for the program ID and verify that it matches.
            let actual = deployment_store.get_latest_edition_for_program(&program_id).unwrap();
            assert_eq!(Some(edition), actual);

            // Retrieve the latest edition for the transaction ID and verify that it matches.
            let actual = deployment_store.get_edition_for_transaction(&transaction_id).unwrap();
            assert_eq!(Some(edition), actual);

            // Remove the deployment.
            deployment_store.remove(&transaction_id).unwrap();

            // Ensure the deployment transaction does not exist.
            let candidate = deployment_store.get_transaction(&transaction_id).unwrap();
            assert_eq!(None, candidate);

            // If the edition is zero, then check that the edition is not found.
            // Otherwise, check that the edition is decremented.
            if edition == 0 {
                let candidate = deployment_store.edition_map().get_confirmed(&program_id).unwrap();
                assert_eq!(None, candidate);
            } else {
                let candidate = deployment_store.edition_map().get_confirmed(&program_id).unwrap();
                assert_eq!(Some(edition.saturating_sub(1)), candidate.as_deref().copied());
            }

            // Ensure the edition is not found in the `IDEditionMap`.
            let candidate = deployment_store.id_edition_map().get_confirmed(&transaction_id).unwrap();
            assert_eq!(None, candidate);

            // Insert the deployment transaction again.
            deployment_store.insert(&transaction).unwrap();
        }
    }

    #[test]
    fn test_find_transaction_id() {
        let rng = &mut TestRng::default();

        // Initialize a new transition store.
        let transition_store = TransitionStore::open(StorageMode::Test(None)).unwrap();
        // Initialize a new fee store.
        let fee_store = FeeStore::open(transition_store).unwrap();
        // Initialize a new deployment store.
        let deployment_store = DeploymentMemory::open(fee_store).unwrap();

        // Sample the transactions.
        let transaction_0 = snarkvm_ledger_test_helpers::sample_deployment_transaction(1, 0, true, rng);
        let transaction_1 = snarkvm_ledger_test_helpers::sample_deployment_transaction(1, 1, false, rng);
        let transaction_2 = snarkvm_ledger_test_helpers::sample_deployment_transaction(2, 0, true, rng);
        let transaction_3 = snarkvm_ledger_test_helpers::sample_deployment_transaction(2, 1, false, rng);
        let transaction_4 = snarkvm_ledger_test_helpers::sample_deployment_transaction(2, 2, true, rng);
        let transactions = vec![transaction_0, transaction_1, transaction_2, transaction_3, transaction_4];

        for transaction in transactions {
            let transaction_id = transaction.id();
            let (program_id, edition) = match transaction {
                Transaction::Deploy(_, _, _, ref deployment, _) => (*deployment.program_id(), deployment.edition()),
                _ => panic!("Incorrect transaction type"),
            };
            let fee_id = *transaction.fee_transition().unwrap().id();

            // Ensure the deployment transaction does not exist.
            let candidate = deployment_store.get_transaction(&transaction_id).unwrap();
            assert_eq!(None, candidate);

            // A helper to test the `find_*` methods.
            let test_find_methods = |program_exists: bool, transaction_exists: bool| {
                // Find the latest transaction ID from the program ID.
                let candidate_0 = deployment_store.find_latest_transaction_id_from_program_id(&program_id).unwrap();
                // Find the transaction ID from the program ID and edition.
                let candidate_1 =
                    deployment_store.find_transaction_id_from_program_id_and_edition(&program_id, edition).unwrap();
                // Find the transaction ID from the transition ID.
                let candidate_2 = deployment_store.find_transaction_id_from_transition_id(&fee_id).unwrap();

                // If the program exists, then the latest transaction ID should be found.
                assert_eq!(program_exists, candidate_0.is_some());
                // If the transaction exists, then the transaction ID should be found.
                assert_eq!(transaction_exists, candidate_1.is_some());
                assert_eq!(candidate_1, candidate_2);
            };

            // If the edition is zero, then check that a transaction is not found.
            // Otherwise, check that the transaction is found.
            if edition == 0 {
                test_find_methods(false, false);
            } else {
                test_find_methods(true, false);
            }

            // Insert the deployment.
            deployment_store.insert(&transaction).unwrap();

            // Get the transaction again.
            let candidate = deployment_store.get_transaction(&transaction_id).unwrap();
            assert_eq!(Some(transaction.clone()), candidate);

            // Find the transaction ID.
            test_find_methods(true, true);

            // Remove the deployment.
            deployment_store.remove(&transaction_id).unwrap();

            // If the edition is zero, then check that a transaction is not found.
            // Otherwise, check that the transaction is found.
            if edition == 0 {
                test_find_methods(false, false);
            } else {
                test_find_methods(true, false);
            }

            // Insert the deployment again.
            deployment_store.insert(&transaction).unwrap();
        }
    }
}
