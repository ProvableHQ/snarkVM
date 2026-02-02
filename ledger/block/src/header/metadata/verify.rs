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

#![allow(clippy::too_many_arguments)]

use snarkvm_utilities::ensure_equals;

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
        if let Err(err) = self.check_validity() {
            bail!("Metadata is malformed in block {expected_height}: {err}");
        }

        ensure_equals!(self.round, expected_round, "Round is incorrect in block {expected_height}");
        ensure_equals!(self.height, expected_height, "Height is incorrect in block {expected_height}");
        ensure_equals!(
            self.cumulative_weight,
            expected_cumulative_weight,
            "Cumulative weight is incorrect in block {expected_height}"
        );
        ensure_equals!(
            self.cumulative_proof_target,
            expected_cumulative_proof_target,
            "Cumulative proof target is incorrect in block {expected_height}"
        );
        ensure_equals!(
            self.coinbase_target,
            expected_coinbase_target,
            "Coinbase target is incorrect in block {expected_height}"
        );
        ensure_equals!(
            self.proof_target,
            expected_proof_target,
            "Proof target is incorrect in block {expected_height}"
        );
        ensure_equals!(
            self.last_coinbase_target,
            expected_last_coinbase_target,
            "Last coinbase target is incorrect in block {expected_height}"
        );
        ensure_equals!(
            self.last_coinbase_timestamp,
            expected_last_coinbase_timestamp,
            "Last coinbase timestamp is incorrect in block {expected_height}"
        );
        ensure_equals!(self.timestamp, expected_timestamp, "Timestamp is incorrect in block {expected_height}");
        ensure!(
            self.timestamp <= current_timestamp,
            "Timestamp is in the future in block {expected_height} (found '{}', expected before '{}')",
            self.timestamp,
            current_timestamp
        );

        Ok(())
    }
}
