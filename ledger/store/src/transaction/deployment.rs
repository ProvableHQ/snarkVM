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
    FeeStorage,
    FeeStore,
    atomic_batch_scope,
    cow_to_cloned,
    cow_to_copied,
    helpers::{Map, MapRead},
};
use console::{
    network::prelude::*,
    program::{Identifier, ProgramID, ProgramOwner},
};
use ledger_block::{Deployment, Fee, Transaction};
use synthesizer_program::Program;
use synthesizer_snark::{Certificate, VerifyingKey};

use aleo_std_storage::StorageMode;
use anyhow::Result;
use core::marker::PhantomData;
use std::borrow::Cow;

/// A trait for deployment storage.
pub trait DeploymentStorage<N: Network>: Clone + Send + Sync {
    /// The mapping of `transaction ID` to `program ID`.
    /// Note. After the migration height, `IDMapV1` will only be used for v1 programs.
    ///   V1 program editions are always `N::EDITION`.
    type IDMapV1: for<'a> Map<'a, N::TransactionID, ProgramID<N>>;
    /// The mapping of `transaction ID` to `(program ID, edition)`.
    /// Note. After the migration height, `IDMapV2` will be used for v2 programs.
    ///   V2 program editions start at zero and increment by one for each update.
    type IDMapV2: for<'a> Map<'a, N::TransactionID, (ProgramID<N>, u16)>;
    /// The mapping of `program ID` to `edition`.
    type EditionMap: for<'a> Map<'a, ProgramID<N>, u16>;
    /// The mapping of `(program ID, edition)` to `transaction ID`.
    type ReverseIDMap: for<'a> Map<'a, (ProgramID<N>, u16), N::TransactionID>;
    /// The mapping of `(program ID, edition)` to `ProgramOwner`.
    type OwnerMap: for<'a> Map<'a, (ProgramID<N>, u16), ProgramOwner<N>>;
    /// The mapping of `(program ID, edition)` to `program`.
    type ProgramMap: for<'a> Map<'a, (ProgramID<N>, u16), Program<N>>;
    /// The mapping of `(program ID, function name, edition)` to `verifying key`.
    type VerifyingKeyMap: for<'a> Map<'a, (ProgramID<N>, Identifier<N>, u16), VerifyingKey<N>>;
    /// The mapping of `(program ID, function name, edition)` to `certificate`.
    type CertificateMap: for<'a> Map<'a, (ProgramID<N>, Identifier<N>, u16), Certificate<N>>;
    /// The fee storage.
    type FeeStorage: FeeStorage<N>;

    /// Initializes the deployment storage.
    fn open(fee_store: FeeStore<N, Self::FeeStorage>) -> Result<Self>;

    /// Returns the ID map.
    fn id_map_v1(&self) -> &Self::IDMapV1;
    /// Returns the V2 ID map.
    fn id_map_v2(&self) -> &Self::IDMapV2;
    /// Returns the edition map.
    fn edition_map(&self) -> &Self::EditionMap;
    /// Returns the reverse ID map.
    fn reverse_id_map(&self) -> &Self::ReverseIDMap;
    /// Returns the owner map.
    fn owner_map(&self) -> &Self::OwnerMap;
    /// Returns the program map.
    fn program_map(&self) -> &Self::ProgramMap;
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
        self.id_map_v1().start_atomic();
        self.id_map_v2().start_atomic();
        self.edition_map().start_atomic();
        self.reverse_id_map().start_atomic();
        self.owner_map().start_atomic();
        self.program_map().start_atomic();
        self.verifying_key_map().start_atomic();
        self.certificate_map().start_atomic();
        self.fee_store().start_atomic();
    }

    /// Checks if an atomic batch is in progress.
    fn is_atomic_in_progress(&self) -> bool {
        self.id_map_v1().is_atomic_in_progress()
            || self.id_map_v2().is_atomic_in_progress()
            || self.edition_map().is_atomic_in_progress()
            || self.reverse_id_map().is_atomic_in_progress()
            || self.owner_map().is_atomic_in_progress()
            || self.program_map().is_atomic_in_progress()
            || self.verifying_key_map().is_atomic_in_progress()
            || self.certificate_map().is_atomic_in_progress()
            || self.fee_store().is_atomic_in_progress()
    }

    /// Checkpoints the atomic batch.
    fn atomic_checkpoint(&self) {
        self.id_map_v1().atomic_checkpoint();
        self.id_map_v2().atomic_checkpoint();
        self.edition_map().atomic_checkpoint();
        self.reverse_id_map().atomic_checkpoint();
        self.owner_map().atomic_checkpoint();
        self.program_map().atomic_checkpoint();
        self.verifying_key_map().atomic_checkpoint();
        self.certificate_map().atomic_checkpoint();
        self.fee_store().atomic_checkpoint();
    }

    /// Clears the latest atomic batch checkpoint.
    fn clear_latest_checkpoint(&self) {
        self.id_map_v1().clear_latest_checkpoint();
        self.id_map_v2().clear_latest_checkpoint();
        self.edition_map().clear_latest_checkpoint();
        self.reverse_id_map().clear_latest_checkpoint();
        self.owner_map().clear_latest_checkpoint();
        self.program_map().clear_latest_checkpoint();
        self.verifying_key_map().clear_latest_checkpoint();
        self.certificate_map().clear_latest_checkpoint();
        self.fee_store().clear_latest_checkpoint();
    }

    /// Rewinds the atomic batch to the previous checkpoint.
    fn atomic_rewind(&self) {
        self.id_map_v1().atomic_rewind();
        self.id_map_v2().atomic_rewind();
        self.edition_map().atomic_rewind();
        self.reverse_id_map().atomic_rewind();
        self.owner_map().atomic_rewind();
        self.program_map().atomic_rewind();
        self.verifying_key_map().atomic_rewind();
        self.certificate_map().atomic_rewind();
        self.fee_store().atomic_rewind();
    }

    /// Aborts an atomic batch write operation.
    fn abort_atomic(&self) {
        self.id_map_v1().abort_atomic();
        self.id_map_v2().abort_atomic();
        self.edition_map().abort_atomic();
        self.reverse_id_map().abort_atomic();
        self.owner_map().abort_atomic();
        self.program_map().abort_atomic();
        self.verifying_key_map().abort_atomic();
        self.certificate_map().abort_atomic();
        self.fee_store().abort_atomic();
    }

    /// Finishes an atomic batch write operation.
    fn finish_atomic(&self) -> Result<()> {
        self.id_map_v1().finish_atomic()?;
        self.id_map_v2().finish_atomic()?;
        self.edition_map().finish_atomic()?;
        self.reverse_id_map().finish_atomic()?;
        self.owner_map().finish_atomic()?;
        self.program_map().finish_atomic()?;
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
        let edition = deployment.edition();
        // Retrieve the program.
        let program = deployment.program();
        // Retrieve the program ID.
        let program_id = *program.id();

        atomic_batch_scope!(self, {
            // Store the program ID and edition.
            // Note that the `id_map_v1` is intentionally excluded.
            self.id_map_v2().insert(*transaction_id, (program_id, edition))?;
            // Store the latest edition.
            self.edition_map().insert(program_id, edition)?;

            // Store the reverse program ID.
            self.reverse_id_map().insert((program_id, edition), *transaction_id)?;
            // Store the owner.
            self.owner_map().insert((program_id, edition), *owner)?;
            // Store the program.
            self.program_map().insert((program_id, edition), program.clone())?;

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
        // Retrieve the program ID and edition from the ID map.
        let (program_id, edition) = match self.get_program_id_and_edition(transaction_id)? {
            Some((program_id, edition)) => (program_id, edition),
            None => bail!("Failed to get the program ID for transaction '{transaction_id}'"),
        };
        // Get the latest edition of the program.
        let latest_edition = match self.get_edition(&program_id)? {
            Some(edition) => edition,
            None => bail!("Failed to locate the edition for program '{program_id}'"),
        };
        // Retrieve the program.
        let program = match self.program_map().get_confirmed(&(program_id, edition))? {
            Some(program) => cow_to_cloned!(program),
            None => bail!("Failed to locate program '{program_id}' for transaction '{transaction_id}'"),
        };

        atomic_batch_scope!(self, {
            // Note that after a specific block migration height, `IDMapV2` will be used instead of `IDMapV1`.
            // This implies that a given transaction will exist in either `IDMapV1` or `IDMapV2`, but not both.
            self.id_map_v1().remove(transaction_id)?;
            self.id_map_v2().remove(transaction_id)?;
            // Update the latest edition.
            match (edition, latest_edition) {
                // If the removed edition is 0, remove the program ID from the edition map.
                (0, _) => self.edition_map().remove(&program_id)?,
                // If the removed edition is the latest one, update the edition map.
                (edition, latest_edition) if edition == latest_edition => {
                    self.edition_map().insert(program_id, edition.saturating_sub(1))?
                }
                // Otherwise, do nothing.
                _ => {}
            }

            // Remove the reverse program ID.
            self.reverse_id_map().remove(&(program_id, edition))?;
            // Remove the owner.
            self.owner_map().remove(&(program_id, edition))?;
            // Remove the program.
            self.program_map().remove(&(program_id, edition))?;

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
    fn find_transaction_id_from_program_id(&self, program_id: &ProgramID<N>) -> Result<Option<N::TransactionID>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            return Ok(None);
        }
        // Retrieve the edition.
        let edition = match self.get_edition(program_id)? {
            Some(edition) => edition,
            None => return Ok(None),
        };
        // Retrieve the transaction ID.
        match self.reverse_id_map().get_confirmed(&(*program_id, edition))? {
            Some(transaction_id) => Ok(Some(cow_to_copied!(transaction_id))),
            None => bail!("Failed to find the transaction ID for program '{program_id}' (edition {edition})"),
        }
    }

    /// Returns the transaction ID that contains the given `program ID` and `edition`.
    fn find_transaction_id_from_program_id_and_edition(
        &self,
        program_id: &ProgramID<N>,
        edition: u16,
    ) -> Result<Option<N::TransactionID>> {
        // Check if the pogram ID is for `credits.aleo`.
        // This case is handled separately, as it is a default program of the VM.
        // TODO: Refer to the prior TODO in `find_transaction_id_from_program_id`.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            return Ok(None);
        }
        // Retrieve the transaction ID.
        match self.reverse_id_map().get_confirmed(&(*program_id, edition))? {
            Some(transaction_id) => Ok(Some(cow_to_copied!(transaction_id))),
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

    /// Returns the program ID and edition for the given `transaction ID`.
    /// Default to the V1 ID map if it does not exist in the V2 ID map.
    fn get_program_id_and_edition(&self, transaction_id: &N::TransactionID) -> Result<Option<(ProgramID<N>, u16)>> {
        match self.id_map_v2().get_confirmed(transaction_id)? {
            Some(result) => Ok(Some(cow_to_copied!(result))),
            None => {
                let Some(program_id) = self.id_map_v1().get_confirmed(transaction_id)? else {
                    return Ok(None);
                };
                let Some(edition) = self.get_edition(&program_id)? else { return Ok(None) };
                Ok(Some((cow_to_copied!(program_id), edition)))
            }
        }
    }

    /// Returns the program ID for the given `transaction ID`.
    fn get_program_id(&self, transaction_id: &N::TransactionID) -> Result<Option<ProgramID<N>>> {
        // Retrieve the program ID.
        Ok(self.get_program_id_and_edition(transaction_id)?.map(|(program_id, _)| program_id))
    }

    /// Returns the latest edition for the given `program ID`.
    fn get_edition(&self, program_id: &ProgramID<N>) -> Result<Option<u16>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            return Ok(None);
        }

        match self.edition_map().get_confirmed(program_id)? {
            Some(edition) => Ok(Some(cow_to_copied!(edition))),
            None => Ok(None),
        }
    }

    /// Returns the latest program for the given `program ID`.
    fn get_program(&self, program_id: &ProgramID<N>) -> Result<Option<Program<N>>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            return Ok(Some(Program::credits()?));
        }
        // Retrieve the edition.
        let edition = match self.get_edition(program_id)? {
            Some(edition) => edition,
            None => return Ok(None),
        };
        // Retrieve the program.
        match self.program_map().get_confirmed(&(*program_id, edition))? {
            Some(program) => Ok(Some(cow_to_cloned!(program))),
            None => bail!("Failed to get program '{program_id}' (edition {edition})"),
        }
    }

    /// Returns the latest program for the given `program ID` and `edition`.
    fn get_program_with_edition(&self, program_id: &ProgramID<N>, edition: u16) -> Result<Option<Program<N>>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            return Ok(Some(Program::credits()?));
        }
        // Retrieve the program.
        match self.program_map().get_confirmed(&(*program_id, edition))? {
            Some(program) => Ok(Some(cow_to_cloned!(program))),
            None => bail!("Failed to get program '{program_id}' (edition {edition})"),
        }
    }

    /// Returns the latest verifying key for the given `program ID` and `function name`.
    fn get_verifying_key(
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

        // Retrieve the edition.
        let edition = match self.get_edition(program_id)? {
            Some(edition) => edition,
            None => return Ok(None),
        };
        // Retrieve the verifying key.
        match self.verifying_key_map().get_confirmed(&(*program_id, *function_name, edition))? {
            Some(verifying_key) => Ok(Some(cow_to_cloned!(verifying_key))),
            None => bail!("Failed to get the verifying key for '{program_id}/{function_name}' (edition {edition})"),
        }
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
            Some(verifying_key) => Ok(Some(cow_to_cloned!(verifying_key))),
            None => bail!("Failed to get the verifying key for '{program_id}/{function_name}' (edition {edition})"),
        }
    }

    /// Returns the certificate for the given `program ID` and `function name`.
    fn get_certificate(
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

        // Retrieve the edition.
        let edition = match self.get_edition(program_id)? {
            Some(edition) => edition,
            None => return Ok(None),
        };
        // Retrieve the certificate.
        match self.certificate_map().get_confirmed(&(*program_id, *function_name, edition))? {
            Some(certificate) => Ok(Some(cow_to_cloned!(certificate))),
            None => bail!("Failed to get the certificate for '{program_id}/{function_name}' (edition {edition})"),
        }
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
            Some(certificate) => Ok(Some(cow_to_cloned!(certificate))),
            None => bail!("Failed to get the certificate for '{program_id}/{function_name}' (edition {edition})"),
        }
    }

    /// Returns the deployment for the given `transaction ID`.
    fn get_deployment(&self, transaction_id: &N::TransactionID) -> Result<Option<Deployment<N>>> {
        // Retrieve the program ID and edition.
        let (program_id, edition) = match self.get_program_id_and_edition(transaction_id)? {
            Some((program_id, edition)) => (program_id, edition),
            None => return Ok(None),
        };
        // Retrieve the program.
        let program = match self.program_map().get_confirmed(&(program_id, edition))? {
            Some(program) => cow_to_cloned!(program),
            None => bail!("Failed to get the deployed program '{program_id}' (edition {edition})"),
        };

        // Initialize a vector for the verifying keys and certificates.
        let mut verifying_keys = Vec::with_capacity(program.functions().len());

        // Retrieve the verifying keys and certificates.
        for function_name in program.functions().keys() {
            // Retrieve the verifying key.
            let verifying_key = match self.verifying_key_map().get_confirmed(&(program_id, *function_name, edition))? {
                Some(verifying_key) => cow_to_cloned!(verifying_key),
                None => bail!("Failed to get the verifying key for '{program_id}/{function_name}' (edition {edition})"),
            };
            // Retrieve the certificate.
            let certificate = match self.certificate_map().get_confirmed(&(program_id, *function_name, edition))? {
                Some(certificate) => cow_to_cloned!(certificate),
                None => bail!("Failed to get the certificate for '{program_id}/{function_name}' (edition {edition})"),
            };
            // Add the verifying key and certificate to the deployment.
            verifying_keys.push((*function_name, (verifying_key, certificate)));
        }

        // Return the deployment.
        Ok(Some(Deployment::new(edition, program, verifying_keys)?))
    }

    /// Returns the fee for the given `transaction ID`.
    fn get_fee(&self, transaction_id: &N::TransactionID) -> Result<Option<Fee<N>>> {
        self.fee_store().get_fee(transaction_id)
    }

    /// Returns the latest owner for the given `program ID`.
    fn get_owner(&self, program_id: &ProgramID<N>) -> Result<Option<ProgramOwner<N>>> {
        // Check if the program ID is for 'credits.aleo'.
        // This case is handled separately, as it is a default program of the VM.
        // TODO (howardwu): After we update 'fee' rules and 'Ratify' in genesis, we can remove this.
        if program_id == &ProgramID::from_str("credits.aleo")? {
            return Ok(None);
        }

        // TODO (raychu86): Consider program upgrades and edition changes.
        // Retrieve the edition.
        let edition = match self.get_edition(program_id)? {
            Some(edition) => edition,
            None => return Ok(None),
        };

        // Retrieve the owner.
        match self.owner_map().get_confirmed(&(*program_id, edition))? {
            Some(owner) => Ok(Some(cow_to_copied!(owner))),
            None => bail!("Failed to find the Owner for program '{program_id}' (edition {edition})"),
        }
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
            Some(owner) => Ok(Some(cow_to_copied!(owner))),
            None => bail!("Failed to find the Owner for program '{program_id}' (edition {edition})"),
        }
    }

    /// Returns the transaction for the given `transaction ID`.
    fn get_transaction(&self, transaction_id: &N::TransactionID) -> Result<Option<Transaction<N>>> {
        // Retrieve the deployment.
        let deployment = match self.get_deployment(transaction_id)? {
            Some(deployment) => deployment,
            None => return Ok(None),
        };
        // Retrieve the fee.
        let fee = match self.get_fee(transaction_id)? {
            Some(fee) => fee,
            None => bail!("Failed to get the fee for transaction '{transaction_id}'"),
        };

        // Retrieve the owner.
        let owner = match self.get_owner_with_edition(deployment.program_id(), deployment.edition())? {
            Some(owner) => owner,
            None => bail!("Failed to get the owner for transaction '{transaction_id}'"),
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
    pub fn get_edition(&self, program_id: &ProgramID<N>) -> Result<Option<u16>> {
        self.storage.get_edition(program_id)
    }

    /// Returns the latest owner for the given `program ID`.
    pub fn get_owner(&self, program_id: &ProgramID<N>) -> Result<Option<ProgramOwner<N>>> {
        self.storage.get_owner(program_id)
    }

    /// Returns the owner for the given `program ID` and `edition`.
    pub fn get_owner_with_edition(&self, program_id: &ProgramID<N>, edition: u16) -> Result<Option<ProgramOwner<N>>> {
        self.storage.get_owner_with_edition(program_id, edition)
    }

    /// Returns the program ID for the given `transaction ID`.
    pub fn get_program_id(&self, transaction_id: &N::TransactionID) -> Result<Option<ProgramID<N>>> {
        self.storage.get_program_id(transaction_id)
    }

    /// Returns the program ID and edition for the given `transaction ID`.
    pub fn get_program_id_and_edition(&self, transaction_id: &N::TransactionID) -> Result<Option<(ProgramID<N>, u16)>> {
        self.storage.get_program_id_and_edition(transaction_id)
    }

    /// Returns the latest program for the given `program ID`.
    pub fn get_program(&self, program_id: &ProgramID<N>) -> Result<Option<Program<N>>> {
        self.storage.get_program(program_id)
    }

    /// Returns the program for the given `program ID` and `edition`.
    pub fn get_program_with_edition(&self, program_id: &ProgramID<N>, edition: u16) -> Result<Option<Program<N>>> {
        self.storage.get_program_with_edition(program_id, edition)
    }

    /// Returns the latest verifying key for the given `(program ID, function name)`.
    pub fn get_verifying_key(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
    ) -> Result<Option<VerifyingKey<N>>> {
        self.storage.get_verifying_key(program_id, function_name)
    }

    /// Returns the verifying key for the given `program ID`, `function name`, and `edition`.
    pub fn get_verifying_key_with_edition(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        edition: u16,
    ) -> Result<Option<VerifyingKey<N>>> {
        self.storage.get_verifying_key_with_edition(program_id, function_name, edition)
    }

    /// Returns the certificate for the given `(program ID, function name)`.
    pub fn get_certificate(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
    ) -> Result<Option<Certificate<N>>> {
        self.storage.get_certificate(program_id, function_name)
    }

    /// Returns the certificate for the given `program ID`, `function name`, and `edition`.
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
}

impl<N: Network, D: DeploymentStorage<N>> DeploymentStore<N, D> {
    /// Returns the transaction ID that deployed the given `program ID`.
    pub fn find_transaction_id_from_program_id(&self, program_id: &ProgramID<N>) -> Result<Option<N::TransactionID>> {
        self.storage.find_transaction_id_from_program_id(program_id)
    }

    /// Returns the transaction ID that deployed the given `program ID` and `edition`.
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

    /// Returns `true` if the given program ID and edition exists.
    pub fn contains_program_id_and_edition(&self, program_id: &ProgramID<N>, edition: u16) -> Result<bool> {
        self.storage.reverse_id_map().contains_key_confirmed(&(*program_id, edition))
    }
}

type ProgramTriplet<N> = (ProgramID<N>, Identifier<N>, u16);

impl<N: Network, D: DeploymentStorage<N>> DeploymentStore<N, D> {
    /// Returns an iterator over the deployment transaction IDs, for all deployments.
    pub fn deployment_transaction_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, N::TransactionID>> {
        self.storage.id_map_v1().keys_confirmed().chain(self.storage.id_map_v2().keys_confirmed())
    }

    /// Returns an iterator over the program IDs, for all deployments.
    /// Note that this can have duplicates.
    pub fn program_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, ProgramID<N>>> {
        self.storage
            .id_map_v1()
            .values_confirmed()
            .map(|id| match id {
                Cow::Borrowed(id) => Cow::Borrowed(id),
                Cow::Owned(id) => Cow::Owned(id),
            })
            .chain(self.storage.id_map_v2().values_confirmed().map(|id| match id {
                Cow::Borrowed(id) => Cow::Borrowed(&id.0),
                Cow::Owned(id) => Cow::Owned(id.0),
            }))
    }

    /// Returns an iterators over the program IDs and editions, for all deployments.
    pub fn program_ids_and_editions(&self) -> impl '_ + Iterator<Item = (Cow<'_, ProgramID<N>>, u16)> {
        self.storage
            .id_map_v1()
            .values_confirmed()
            .map(|id| match id {
                Cow::Borrowed(id) => (Cow::Borrowed(id), N::EDITION),
                Cow::Owned(id) => (Cow::Owned(id), N::EDITION),
            })
            .chain(self.storage.id_map_v2().values_confirmed().map(|id| match id {
                Cow::Borrowed(id) => (Cow::Borrowed(&id.0), id.1),
                Cow::Owned(id) => (Cow::Owned(id.0), id.1),
            }))
    }

    /// Returns an iterator over the programs, for all deployments.
    /// Note that this may contain multiple versions of the same program.
    pub fn programs(&self) -> impl '_ + Iterator<Item = Cow<'_, Program<N>>> {
        self.storage.program_map().values_confirmed().map(|program| match program {
            Cow::Borrowed(program) => Cow::Borrowed(program),
            Cow::Owned(program) => Cow::Owned(program),
        })
    }

    /// Returns an iterator over the programs and their respective editions, for all deployments.
    pub fn programs_and_editions(&self) -> impl '_ + Iterator<Item = (Cow<'_, Program<N>>, u16)> {
        self.storage.program_map().iter_confirmed().map(|(key, value)| match value {
            Cow::Borrowed(program) => (Cow::Borrowed(program), key.1),
            Cow::Owned(program) => (Cow::Owned(program), key.1),
        })
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

        // Sample the transactions.
        let transaction_0 = ledger_test_helpers::sample_deployment_transaction(true, rng);
        let transaction_1 = ledger_test_helpers::sample_deployment_transaction(false, rng);
        let transactions = vec![transaction_0, transaction_1];

        for transaction in transactions {
            let transaction_id = transaction.id();

            // Initialize a new transition store.
            let transition_store = TransitionStore::open(None).unwrap();
            // Initialize a new fee store.
            let fee_store = FeeStore::open(transition_store).unwrap();
            // Initialize a new deployment store.
            let deployment_store = DeploymentMemory::open(fee_store).unwrap();

            // Ensure the deployment transaction does not exist.
            let candidate = deployment_store.get_transaction(&transaction_id).unwrap();
            assert_eq!(None, candidate);

            // Insert the deployment transaction.
            deployment_store.insert(&transaction).unwrap();

            // Retrieve the deployment transaction.
            let candidate = deployment_store.get_transaction(&transaction_id).unwrap();
            assert_eq!(Some(transaction), candidate);

            // Remove the deployment.
            deployment_store.remove(&transaction_id).unwrap();

            // Ensure the deployment transaction does not exist.
            let candidate = deployment_store.get_transaction(&transaction_id).unwrap();
            assert_eq!(None, candidate);
        }
    }

    #[test]
    fn test_find_transaction_id() {
        let rng = &mut TestRng::default();

        // Sample the transactions.
        let transaction_0 = ledger_test_helpers::sample_deployment_transaction(true, rng);
        let transaction_1 = ledger_test_helpers::sample_deployment_transaction(false, rng);
        let transactions = vec![transaction_0, transaction_1];

        for transaction in transactions {
            let transaction_id = transaction.id();
            let program_id = match transaction {
                Transaction::Deploy(_, _, _, ref deployment, _) => *deployment.program_id(),
                _ => panic!("Incorrect transaction type"),
            };

            // Initialize a new transition store.
            let transition_store = TransitionStore::open(None).unwrap();
            // Initialize a new fee store.
            let fee_store = FeeStore::open(transition_store).unwrap();
            // Initialize a new deployment store.
            let deployment_store = DeploymentMemory::open(fee_store).unwrap();

            // Ensure the deployment transaction does not exist.
            let candidate = deployment_store.get_transaction(&transaction_id).unwrap();
            assert_eq!(None, candidate);

            // Ensure the transaction ID is not found.
            let candidate = deployment_store.find_transaction_id_from_program_id(&program_id).unwrap();
            assert_eq!(None, candidate);

            // Insert the deployment.
            deployment_store.insert(&transaction).unwrap();

            // Find the transaction ID.
            let candidate = deployment_store.find_transaction_id_from_program_id(&program_id).unwrap();
            assert_eq!(Some(transaction_id), candidate);

            // Remove the deployment.
            deployment_store.remove(&transaction_id).unwrap();

            // Ensure the transaction ID is not found.
            let candidate = deployment_store.find_transaction_id_from_program_id(&program_id).unwrap();
            assert_eq!(None, candidate);
        }
    }
}
