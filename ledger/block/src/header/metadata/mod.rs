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

mod bytes;
mod genesis;
mod serialize;
mod string;
mod to_bits;
mod to_hash;
mod verify;

use console::{network::prelude::*, types::Field};

use core::marker::PhantomData;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Metadata<N: Network> {
    /// The network ID of the block.
    network: u16,
    /// The round that produced this block - 8 bytes.
    round: u64,
    /// The height of this block - 4 bytes.
    height: u32,
    /// The cumulative weight for this block - 16 bytes.
    cumulative_weight: u128,
    /// The cumulative proof target for this block - 16 bytes.
    cumulative_proof_target: u128,
    /// The coinbase target for this block - 8 bytes.
    coinbase_target: u64,
    /// The proof target for this block - 8 bytes.
    proof_target: u64,
    /// The coinbase target for the last coinbase - 8 bytes.
    last_coinbase_target: u64,
    /// The Unix timestamp (UTC) for the last coinbase - 8 bytes.
    last_coinbase_timestamp: i64,
    /// The Unix timestamp (UTC) for this block - 8 bytes.
    timestamp: i64,
    /// PhantomData.
    _phantom: PhantomData<N>,
}

impl<N: Network> Metadata<N> {
    /// Initializes a new metadata with the given inputs.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        network: u16,
        round: u64,
        height: u32,
        cumulative_weight: u128,
        cumulative_proof_target: u128,
        coinbase_target: u64,
        proof_target: u64,
        last_coinbase_target: u64,
        last_coinbase_timestamp: i64,
        timestamp: i64,
    ) -> Result<Self> {
        // Construct a new metadata.
        let metadata = Self {
            network,
            round,
            height,
            cumulative_weight,
            cumulative_proof_target,
            coinbase_target,
            proof_target,
            last_coinbase_target,
            last_coinbase_timestamp,
            timestamp,
            _phantom: PhantomData,
        };
        // Ensure the header is valid.
        match metadata.is_valid() {
            true => Ok(metadata),
            false => bail!("Invalid block metadata: {:?}", metadata),
        }
    }

    /// Returns `true` if the block metadata is well-formed.
    pub fn is_valid(&self) -> bool {
        match self.height == 0u32 {
            true => self.is_genesis(),
            false => {
                // Ensure the network ID is correct.
                self.network == N::ID
                    // Ensure the round is nonzero.
                    && self.round != 0u64
                    // Ensure the height is nonzero.
                    && self.height != 0u32
                    // Ensure the round is at least as large as the height.
                    && self.round >= self.height as u64
                    // Ensure the coinbase target is at or above the minimum.
                    && self.coinbase_target >= N::GENESIS_COINBASE_TARGET
                    // Ensure the proof target is at or above the minimum.
                    && self.proof_target >= N::GENESIS_PROOF_TARGET
                    // Ensure the coinbase target is larger than the proof target.
                    && self.coinbase_target > self.proof_target
                    // Ensure the last coinbase target is at or above the minimum.
                    && self.last_coinbase_target >= N::GENESIS_COINBASE_TARGET
                    // Ensure the last coinbase timestamp is after the genesis timestamp.
                    && self.last_coinbase_timestamp >= N::GENESIS_TIMESTAMP
                    // Ensure the timestamp in the block is after the genesis timestamp.
                    && self.timestamp > N::GENESIS_TIMESTAMP
            }
        }
    }
}

impl<N: Network> Metadata<N> {
    /// Returns the network ID of the block.
    pub const fn network(&self) -> u16 {
        self.network
    }

    /// Returns the round number of the block.
    pub const fn round(&self) -> u64 {
        self.round
    }

    /// Returns the height of the block.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the cumulative weight for this block.
    pub const fn cumulative_weight(&self) -> u128 {
        self.cumulative_weight
    }

    /// Returns the cumulative proof target for this block.
    pub const fn cumulative_proof_target(&self) -> u128 {
        self.cumulative_proof_target
    }

    /// Returns the coinbase target for this block.
    pub const fn coinbase_target(&self) -> u64 {
        self.coinbase_target
    }

    /// Returns the proof target for this block.
    pub const fn proof_target(&self) -> u64 {
        self.proof_target
    }

    /// Returns the coinbase target of the last coinbase.
    pub const fn last_coinbase_target(&self) -> u64 {
        self.last_coinbase_target
    }

    /// Returns the block timestamp of the last coinbase.
    pub const fn last_coinbase_timestamp(&self) -> i64 {
        self.last_coinbase_timestamp
    }

    /// Returns the Unix timestamp (UTC) for this block.
    pub const fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;

    type CurrentNetwork = console::network::MainnetV0;

    /// Samples a block metadata.
    pub(crate) fn sample_block_metadata(rng: &mut TestRng) -> Metadata<CurrentNetwork> {
        *crate::test_helpers::sample_genesis_block(rng).metadata()
    }
}
