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

use snarkvm_utilities::ensure_equals;

const GENESIS_ROUND: u64 = 0;
const GENESIS_HEIGHT: u32 = 0;
const GENESIS_CUMULATIVE_WEIGHT: u128 = 0;
const GENESIS_CUMULATIVE_PROOF_TARGET: u128 = 0;

impl<N: Network> Metadata<N> {
    /// Initializes the genesis metadata.
    pub fn genesis() -> Result<Self> {
        Self::new(
            N::ID,
            GENESIS_ROUND,
            GENESIS_HEIGHT,
            GENESIS_CUMULATIVE_WEIGHT,
            GENESIS_CUMULATIVE_PROOF_TARGET,
            N::GENESIS_COINBASE_TARGET,
            N::GENESIS_PROOF_TARGET,
            N::GENESIS_COINBASE_TARGET,
            N::GENESIS_TIMESTAMP,
            N::GENESIS_TIMESTAMP,
        )
        .with_context(|| "Failed to create genesis block")
    }

    /// Returns `true` if the metadata is a genesis metadata.
    ///
    /// Return an error if the block is at height 0 but not a valid genesis header.
    pub fn is_genesis(&self) -> Result<bool> {
        // Only blocks at height 0 are genesis blocks.
        if self.round != 0u64 {
            return Ok(false);
        }

        // Check the genesis block header is valid otherwise.
        // We cannot call `Self::check_validity` here as that function calls `is_genesis` internally.
        ensure!(self.network == N::ID, "Invalid network ID");
        ensure!(self.round == GENESIS_ROUND, "Genesis block not at genesis round");
        ensure!(self.height == GENESIS_HEIGHT, "Genesis block not at genesis height");

        ensure_equals!(self.cumulative_weight, GENESIS_CUMULATIVE_WEIGHT, "Invalid cumulative weight");
        ensure_equals!(
            self.cumulative_proof_target,
            GENESIS_CUMULATIVE_PROOF_TARGET,
            "Invalid cumulative proof target"
        );
        ensure_equals!(self.timestamp, N::GENESIS_TIMESTAMP, "Invalid timestamp");
        ensure_equals!(self.last_coinbase_timestamp, N::GENESIS_TIMESTAMP, "Invalid last coinbase timestamp");
        ensure_equals!(
            self.coinbase_target,
            N::GENESIS_COINBASE_TARGET,
            "Invalid coinsbase target for genesis block expected {expected}."
        );
        ensure_equals!(
            self.last_coinbase_target,
            N::GENESIS_COINBASE_TARGET,
            "Invalid last coinbase target for genesis block"
        );
        ensure_equals!(self.proof_target, N::GENESIS_PROOF_TARGET, "Invalid proof target for genesis block");

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    /// Returns the expected metadata size by summing its subcomponent sizes.
    /// Update this method if the contents of the metadata have changed.
    fn get_expected_size() -> usize {
        // Metadata size.
        1 + 8 + 4 + 16 + 16 + 8 + 8 + 8 + 8 + 8
            // Add an additional 2 bytes for versioning.
            + 2
    }

    #[test]
    fn test_genesis_metadata_size() {
        let rng = &mut TestRng::default();

        // Prepare the expected size.
        let expected_size = get_expected_size();
        // Prepare the genesis metadata.
        let genesis_metadata = crate::header::metadata::test_helpers::sample_block_metadata(rng);
        // Ensure the size of the genesis metadata is correct.
        assert_eq!(expected_size, genesis_metadata.to_bytes_le().unwrap().len());
    }

    #[test]
    fn test_genesis_metadata() {
        let rng = &mut TestRng::default();

        // Prepare the genesis metadata.
        let metadata = crate::header::metadata::test_helpers::sample_block_metadata(rng);
        // Ensure the metadata is a genesis metadata.
        assert!(metadata.is_genesis().unwrap());

        // Ensure the genesis block contains the following.
        assert_eq!(metadata.network(), CurrentNetwork::ID);
        assert_eq!(metadata.round(), 0);
        assert_eq!(metadata.height(), 0);
        assert_eq!(metadata.cumulative_weight(), 0);
        assert_eq!(metadata.cumulative_proof_target(), 0);
        assert_eq!(metadata.coinbase_target(), CurrentNetwork::GENESIS_COINBASE_TARGET);
        assert_eq!(metadata.proof_target(), CurrentNetwork::GENESIS_PROOF_TARGET);
        assert_eq!(metadata.last_coinbase_target(), CurrentNetwork::GENESIS_COINBASE_TARGET);
        assert_eq!(metadata.last_coinbase_timestamp(), CurrentNetwork::GENESIS_TIMESTAMP);
        assert_eq!(metadata.timestamp(), CurrentNetwork::GENESIS_TIMESTAMP);
    }
}
