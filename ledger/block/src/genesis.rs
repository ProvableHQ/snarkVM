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

impl<N: Network> Block<N> {
    /// Specifies the number of genesis transactions.
    pub const NUM_GENESIS_TRANSACTIONS: usize = 4;

    /// Returns `true` if the block is a genesis block.
    pub fn is_genesis(&self) -> bool {
        // Ensure the previous block hash is zero.
        self.previous_hash == N::BlockHash::default()
            // Ensure the header is a genesis block header.
            && self.header.is_genesis()
            // Ensure the genesis authority is a beacon.
            && self.authority.is_beacon()
            // Ensure there is the correct number of ratification operations in the genesis block.
            && self.ratifications.len() == 1
            // Ensure there are no solutions in the genesis block.
            && self.solutions.is_empty()
            // Ensure there is the correct number of accepted transaction in the genesis block.
            && self.transactions.num_accepted() == Self::NUM_GENESIS_TRANSACTIONS
            // Ensure there is the correct number of rejected transaction in the genesis block.
            && self.transactions.num_rejected() == 0
            // Ensure there is the correct number of finalize operations in the genesis block.
            && self.transactions.num_finalize() == 2 * Self::NUM_GENESIS_TRANSACTIONS
            // Ensure there are no aborted transaction IDs in the genesis block.
            && self.aborted_transaction_ids.is_empty()
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
        assert!(genesis_block.is_genesis());
    }
}
