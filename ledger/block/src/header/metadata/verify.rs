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

impl<N: Network> Metadata<N> {
    /// Ensures the block metadata is correct.
    pub fn verify(
        &self,
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
        // Ensure the block metadata is well-formed.
        ensure!(self.is_valid(), "Metadata is malformed in block {expected_height}");
        // Ensure the round is correct.
        ensure!(
            self.round == expected_round,
            "Round is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.round,
            expected_round
        );
        // Ensure the height is correct.
        ensure!(
            self.height == expected_height,
            "Height is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.height,
            expected_height
        );
        // Ensure the cumulative weight is correct.
        ensure!(
            self.cumulative_weight == expected_cumulative_weight,
            "Cumulative weight is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.cumulative_weight,
            expected_cumulative_weight
        );
        // Ensure the cumulative proof target is correct.
        ensure!(
            self.cumulative_proof_target == expected_cumulative_proof_target,
            "Cumulative proof target is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.cumulative_proof_target,
            expected_cumulative_proof_target
        );
        // Ensure the coinbase target is correct.
        ensure!(
            self.coinbase_target == expected_coinbase_target,
            "Coinbase target is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.coinbase_target,
            expected_coinbase_target
        );
        // Ensure the proof target is correct.
        ensure!(
            self.proof_target == expected_proof_target,
            "Proof target is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.proof_target,
            expected_proof_target
        );
        // Ensure the last coinbase target is correct.
        ensure!(
            self.last_coinbase_target == expected_last_coinbase_target,
            "Last coinbase target is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.last_coinbase_target,
            expected_last_coinbase_target
        );
        // Ensure the last coinbase timestamp is correct.
        ensure!(
            self.last_coinbase_timestamp == expected_last_coinbase_timestamp,
            "Last coinbase timestamp is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.last_coinbase_timestamp,
            expected_last_coinbase_timestamp
        );
        // Ensure the timestamp is correct.
        ensure!(
            self.timestamp == expected_timestamp,
            "Timestamp is incorrect in block {expected_height} (found '{}', expected '{}')",
            self.timestamp,
            expected_timestamp
        );
        // Ensure the timestamp is after the current timestamp.
        ensure!(
            self.timestamp <= current_timestamp,
            "Timestamp is in the future in block {expected_height} (found '{}', expected before '{}')",
            self.timestamp,
            current_timestamp
        );
        // Return success.
        Ok(())
    }
}
