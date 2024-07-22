// Copyright (C) 2019-2023 Aleo Systems Inc.
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

use super::*;

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Returns `true` if the given state root exists.
    pub fn contains_state_root(&self, state_root: &N::StateRoot) -> Result<bool> {
        self.vm.block_store().contains_state_root(state_root)
    }

    /// Returns `true` if the given block height exists.
    pub fn contains_block_height(&self, height: u32) -> Result<bool> {
        self.vm.block_store().contains_block_height(height)
    }

    /// Returns `true` if the given block hash exists.
    pub fn contains_block_hash(&self, block_hash: &N::BlockHash) -> Result<bool> {
        self.vm.block_store().contains_block_hash(block_hash)
    }

    /// Returns `true` if the given batch certificate ID exists.
    pub fn contains_certificate(&self, certificate_id: &Field<N>) -> Result<bool> {
        self.vm.block_store().contains_certificate(certificate_id)
    }

    /// Returns `true` if the given program ID exists.
    pub fn contains_program_id(&self, program_id: &ProgramID<N>) -> Result<bool> {
        self.vm.transaction_store().contains_program_id(program_id)
    }

    /// Returns `true` if the transmission exists in the ledger.
    pub fn contains_transmission(&self, transmission_id: &TransmissionID<N>) -> Result<bool> {
        match transmission_id {
            TransmissionID::Ratification => Ok(false),
            TransmissionID::Solution(solution_id, _) => self.contains_solution_id(solution_id),
            TransmissionID::Transaction(transaction_id, _) => self.contains_transaction_id(transaction_id),
        }
    }

    /// Returns `true` if the given solution ID exists.
    pub fn contains_solution_id(&self, solution_id: &SolutionID<N>) -> Result<bool> {
        self.vm.block_store().contains_solution_id(solution_id)
    }

    /* Transaction */

    /// Returns `true` if the given transaction ID exists.
    pub fn contains_transaction_id(&self, transaction_id: &N::TransactionID) -> Result<bool> {
        self.vm.block_store().contains_transaction_id(transaction_id)
    }

    /* Transition */

    /// Returns `true` if the given transition ID exists.
    pub fn contains_transition_id(&self, transition_id: &N::TransitionID) -> Result<bool> {
        self.vm.transition_store().contains_transition_id(transition_id)
    }

    /* Input */

    /// Returns `true` if the given input ID exists.
    pub fn contains_input_id(&self, input_id: &Field<N>) -> Result<bool> {
        self.vm.transition_store().contains_input_id(input_id)
    }

    /// Returns `true` if the given serial number exists.
    pub fn contains_serial_number(&self, serial_number: &Field<N>) -> Result<bool> {
        self.vm.transition_store().contains_serial_number(serial_number)
    }

    /// Returns `true` if the given tag exists.
    pub fn contains_tag(&self, tag: &Field<N>) -> Result<bool> {
        self.vm.transition_store().contains_tag(tag)
    }

    /* Output */

    /// Returns `true` if the given output ID exists.
    pub fn contains_output_id(&self, output_id: &Field<N>) -> Result<bool> {
        self.vm.transition_store().contains_output_id(output_id)
    }

    /// Returns `true` if the given commitment exists.
    pub fn contains_commitment(&self, commitment: &Field<N>) -> Result<bool> {
        self.vm.transition_store().contains_commitment(commitment)
    }

    /// Returns `true` if the given checksum exists.
    pub fn contains_checksum(&self, checksum: &Field<N>) -> bool {
        self.vm.transition_store().contains_checksum(checksum)
    }

    /// Returns `true` if the given nonce exists.
    pub fn contains_nonce(&self, nonce: &Group<N>) -> Result<bool> {
        self.vm.transition_store().contains_nonce(nonce)
    }

    /* Metadata */

    /// Returns `true` if the given transition public key exists.
    pub fn contains_tpk(&self, tpk: &Group<N>) -> Result<bool> {
        self.vm.transition_store().contains_tpk(tpk)
    }

    /// Returns `true` if the given transition commitment exists.
    pub fn contains_tcm(&self, tcm: &Field<N>) -> Result<bool> {
        self.vm.transition_store().contains_tcm(tcm)
    }
}
