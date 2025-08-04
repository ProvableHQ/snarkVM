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

use super::*;

impl<N: Network> Block<N> {
    /// Specifies the number of genesis transactions.
    pub const NUM_GENESIS_TRANSACTIONS: usize = 4;

    /// Returns `Ok(true)` if the block is a genesis block.
    pub fn is_genesis(&self) -> Result<bool> {
        // Ensure the previous block hash is zero.
        if self.previous_hash != N::BlockHash::default()
            || !self.header.is_genesis()
            || !self.authority.is_beacon()
            || !self.ratifications.len() == 1
        {
            return Ok(false);
        }

        if !self.solutions.is_empty() {
            return Err(error("Genesis block must have no solutions").into());
        }

        if !self.transactions.num_accepted() == Self::NUM_GENESIS_TRANSACTIONS {
            return Err(error(format!(
                "Genesis block must have {} accepted transactions",
                Self::NUM_GENESIS_TRANSACTIONS
            ))
            .into());
        }

        if !self.transactions.num_rejected() == 0 {
            return Err(error("Genesis block must have no rejected transactions").into());
        }

        if !self.transactions.num_finalize() == 2 * Self::NUM_GENESIS_TRANSACTIONS {
            return Err(error(format!(
                "Genesis block must have {} finalize operations",
                2 * Self::NUM_GENESIS_TRANSACTIONS
            ))
            .into());
        }

        if !self.aborted_transaction_ids.is_empty() {
            return Err(error("Genesis block must have no aborted transaction IDs").into());
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_genesis() {
        // Load the genesis block.
        let genesis_block = Block::<CurrentNetwork>::read_le(CurrentNetwork::genesis_bytes()).unwrap();
        assert!(genesis_block.is_genesis().unwrap());
    }
}
