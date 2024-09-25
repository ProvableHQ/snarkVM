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

#![allow(clippy::too_many_arguments)]

use super::*;

impl<N: Network> Header<N> {
    /// Ensures the block header is correct.
    pub fn verify(
        &self,
        expected_previous_state_root: N::StateRoot,
        expected_transactions_root: Field<N>,
        expected_finalize_root: Field<N>,
        expected_ratifications_root: Field<N>,
        expected_solutions_root: Field<N>,
        expected_subdag_root: Field<N>,
        expected_round: u64,
        expected_height: u32,
        expected_cumulative_weight: u128,
        expected_cumulative_proof_target: u128,
        expected_coinbase_target: u64,
        expected_proof_target: u64,
        expected_last_coinbase_target: u64,
        expected_last_coinbase_timestamp: i64,
        expected_timestamp: i64,
        current_timestamp: i64,
    ) -> Result<()> {
        // Ensure the block header is well-formed.
        ensure!(self.is_valid(), "Header is malformed in block {expected_height}");
        // Ensure the previous state root is correct.
        ensure!(
            self.previous_state_root == expected_previous_state_root,
            "Previous state root is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.previous_state_root,
            expected_previous_state_root
        );
        // Ensure the transactions root is correct.
        ensure!(
            self.transactions_root == expected_transactions_root,
            "Transactions root is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.transactions_root,
            expected_transactions_root
        );
        // Ensure the finalize root is correct.
        ensure!(
            self.finalize_root == expected_finalize_root,
            "Finalize root is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.finalize_root,
            expected_finalize_root
        );
        // Ensure the ratifications root is correct.
        ensure!(
            self.ratifications_root == expected_ratifications_root,
            "Ratifications root is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.ratifications_root,
            expected_ratifications_root
        );
        // Ensure the solutions root is correct.
        ensure!(
            self.solutions_root == expected_solutions_root,
            "Solutions root is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.solutions_root,
            expected_solutions_root
        );
        // Ensure the subdag root is correct.
        ensure!(
            self.subdag_root == expected_subdag_root,
            "Subdag root is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.subdag_root,
            expected_subdag_root
        );
        // Ensure the block metadata is correct.
        self.metadata.verify(
            expected_round,
            expected_height,
            expected_cumulative_weight,
            expected_cumulative_proof_target,
            expected_coinbase_target,
            expected_proof_target,
            expected_last_coinbase_target,
            expected_last_coinbase_timestamp,
            expected_timestamp,
            current_timestamp,
        )
    }
}
