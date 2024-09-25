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

use console::network::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FinalizeGlobalState {
    /// The block round.
    block_round: u64,
    /// The block height.
    block_height: u32,
    /// The block-specific random seed.
    random_seed: [u8; 32],
}

impl FinalizeGlobalState {
    /// Initializes a new genesis global state.
    #[inline]
    pub fn new_genesis<N: Network>() -> Result<Self> {
        // Initialize the parameters.
        let block_round = 0;
        let block_height = 0;
        let block_cumulative_weight = 0;
        let block_cumulative_proof_target = 0;
        let previous_block_hash = N::BlockHash::default();
        // Return the new global state.
        Self::new::<N>(
            block_round,
            block_height,
            block_cumulative_weight,
            block_cumulative_proof_target,
            previous_block_hash,
        )
    }

    /// Initializes a new global state from the given inputs.
    #[inline]
    pub fn new<N: Network>(
        block_round: u64,
        block_height: u32,
        block_cumulative_weight: u128,
        block_cumulative_proof_target: u128,
        previous_block_hash: N::BlockHash,
    ) -> Result<Self> {
        // Initialize the preimage.
        let preimage = to_bits_le![
            block_round,
            block_height,
            block_cumulative_weight,
            block_cumulative_proof_target,
            (*previous_block_hash); 605
        ];

        // Hash the preimage to get the random seed.
        let seed = N::hash_bhp768(&preimage)?.to_bytes_le()?;
        // Ensure the seed is 32-bytes.
        ensure!(seed.len() == 32, "Invalid seed length for finalize global state.");

        // Convert the seed into a 32-byte array.
        let mut random_seed = [0u8; 32];
        random_seed.copy_from_slice(&seed[..32]);

        Ok(Self { block_round, block_height, random_seed })
    }

    /// Initializes a new global state.
    #[inline]
    pub const fn from(block_round: u64, block_height: u32, random_seed: [u8; 32]) -> Self {
        Self { block_round, block_height, random_seed }
    }

    /// Returns the block round.
    #[inline]
    pub const fn block_round(&self) -> u64 {
        self.block_round
    }

    /// Returns the block height.
    #[inline]
    pub const fn block_height(&self) -> u32 {
        self.block_height
    }

    /// Returns the random seed.
    #[inline]
    pub const fn random_seed(&self) -> &[u8; 32] {
        &self.random_seed
    }
}
