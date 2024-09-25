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

use super::*;

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Returns an iterator over the state roots, for all blocks in `self`.
    pub fn state_roots(&self) -> impl '_ + Iterator<Item = Cow<'_, N::StateRoot>> {
        self.vm.block_store().state_roots()
    }

    /// Returns an iterator over the solution IDs, for all blocks in `self`.
    pub fn solution_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, SolutionID<N>>> {
        self.vm.block_store().solution_ids()
    }

    /* Transaction */

    /// Returns an iterator over the program IDs, for all transactions in `self`.
    pub fn program_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, ProgramID<N>>> {
        self.vm.transaction_store().program_ids()
    }

    /// Returns an iterator over the programs, for all transactions in `self`.
    pub fn programs(&self) -> impl '_ + Iterator<Item = Cow<'_, Program<N>>> {
        self.vm.transaction_store().programs()
    }

    /// Returns an iterator over the transaction IDs, for all transactions in `self`.
    pub fn transaction_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, N::TransactionID>> {
        self.vm.transaction_store().transaction_ids()
    }

    /* Transition */

    /// Returns an iterator over the transition IDs, for all transitions.
    pub fn transition_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, N::TransitionID>> {
        self.vm.transition_store().transition_ids()
    }

    /* Input */

    /// Returns an iterator over the input IDs, for all transition inputs.
    pub fn input_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.vm.transition_store().input_ids()
    }

    /// Returns an iterator over the serial numbers, for all transition inputs that are records.
    pub fn serial_numbers(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.vm.transition_store().serial_numbers()
    }

    /// Returns an iterator over the tags, for all transition inputs that are records.
    pub fn tags(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.vm.transition_store().tags()
    }

    /* Output */

    /// Returns an iterator over the output IDs, for all transition outputs that are records.
    pub fn output_ids(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.vm.transition_store().output_ids()
    }

    /// Returns an iterator over the commitments, for all transition outputs that are records.
    pub fn commitments(&self) -> impl '_ + Iterator<Item = Cow<'_, Field<N>>> {
        self.vm.transition_store().commitments()
    }

    /// Returns an iterator over the nonces, for all transition outputs that are records.
    pub fn nonces(&self) -> impl '_ + Iterator<Item = Cow<'_, Group<N>>> {
        self.vm.transition_store().nonces()
    }

    /// Returns an iterator over the `(commitment, record)` pairs, for all transition outputs that are records.
    pub fn records(&self) -> impl '_ + Iterator<Item = (Cow<'_, Field<N>>, Cow<'_, Record<N, Ciphertext<N>>>)> {
        self.vm.transition_store().records()
    }

    /* Metadata */

    /// Returns an iterator over the transition public keys, for all transactions.
    pub fn transition_public_keys(&self) -> impl '_ + Iterator<Item = Cow<'_, Group<N>>> {
        self.vm.transition_store().tpks()
    }
}
