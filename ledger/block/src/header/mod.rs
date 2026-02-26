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

mod metadata;
pub use metadata::*;

mod bytes;
mod genesis;
mod merkle;
mod serialize;
mod string;
mod verify;

use crate::{Ratifications, Transactions};
use console::{
    network::prelude::*,
    program::{HEADER_DEPTH, HeaderLeaf, HeaderPath, HeaderTree},
    types::Field,
};
use snarkvm_synthesizer_program::FinalizeOperation;

use anyhow::Context;

/// The header for the block contains metadata that uniquely identifies the block.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Header<N: Network> {
    /// The Merkle root representing the blocks in the ledger up to the previous block.
    previous_state_root: N::StateRoot,
    /// The Merkle root representing the transactions in the block.
    transactions_root: Field<N>,
    /// The Merkle root representing the on-chain finalize including the current block.
    finalize_root: Field<N>,
    /// The Merkle root representing the ratifications in the block.
    ratifications_root: Field<N>,
    /// The solutions root of the puzzle.
    solutions_root: Field<N>,
    /// The subdag Merkle root of the authority.
    subdag_root: Field<N>,
    /// The metadata of the block.
    metadata: Metadata<N>,
}

impl<N: Network> Header<N> {
    /// Initializes a new block header with the given inputs.
    pub fn from(
        previous_state_root: N::StateRoot,
        transactions_root: Field<N>,
        finalize_root: Field<N>,
        ratifications_root: Field<N>,
        solutions_root: Field<N>,
        subdag_root: Field<N>,
        metadata: Metadata<N>,
    ) -> Result<Self> {
        // Construct a new block header.
        let header = Self {
            previous_state_root,
            transactions_root,
            finalize_root,
            ratifications_root,
            solutions_root,
            subdag_root,
            metadata,
        };
        // Ensure the header is valid.
        header.check_validity().with_context(|| "Invalid block header")?;
        Ok(header)
    }

    /// Returns `true` if the block header is well-formed.
    ///
    /// Consider using [`Self::check_validity`] to get more information on invalid block headers.
    pub fn is_valid(&self) -> bool {
        self.check_validity().is_ok()
    }

    /// Returns `Ok(())` if the block header is well-formed, and error describing (one of) the problem(s) in the block header.
    pub fn check_validity(&self) -> Result<()> {
        if self.height() == 0u32 {
            if !self.is_genesis()? {
                bail!("Block at height 0 is not a gensis block");
            }
            return Ok(());
        }

        self.metadata.check_validity().with_context(|| "Invalid metadata")?;

        ensure!(*self.previous_state_root != Field::zero(), "Previous state root cannot be zero");
        ensure!(self.transactions_root != Field::zero(), "Transactions root cannot be zero");
        ensure!(self.finalize_root != Field::zero(), "Finalize root cannot be zero");
        ensure!(self.ratifications_root != Field::zero(), "Ratifications root cannot be zero");

        Ok(())
    }

    /// Returns the previous state root from the block header.
    pub const fn previous_state_root(&self) -> N::StateRoot {
        self.previous_state_root
    }

    /// Returns the transactions root in the block header.
    pub const fn transactions_root(&self) -> Field<N> {
        self.transactions_root
    }

    /// Returns the finalize root in the block header.
    pub const fn finalize_root(&self) -> Field<N> {
        self.finalize_root
    }

    /// Returns the ratifications root in the block header.
    pub const fn ratifications_root(&self) -> Field<N> {
        self.ratifications_root
    }

    /// Returns the solutions root in the block header.
    pub const fn solutions_root(&self) -> Field<N> {
        self.solutions_root
    }

    /// Returns the subdag root in the block header.
    pub const fn subdag_root(&self) -> Field<N> {
        self.subdag_root
    }

    /// Returns the metadata in the block header.
    pub const fn metadata(&self) -> &Metadata<N> {
        &self.metadata
    }

    /// Returns the network ID of the block.
    pub const fn network(&self) -> u16 {
        self.metadata.network()
    }

    /// Returns the round number of the block.
    pub const fn round(&self) -> u64 {
        self.metadata.round()
    }

    /// Returns the height of the block.
    pub const fn height(&self) -> u32 {
        self.metadata.height()
    }

    /// Returns the cumulative weight for this block.
    pub const fn cumulative_weight(&self) -> u128 {
        self.metadata.cumulative_weight()
    }

    /// Returns the cumulative proof target for this block.
    pub const fn cumulative_proof_target(&self) -> u128 {
        self.metadata.cumulative_proof_target()
    }

    /// Returns the coinbase target for this block.
    pub const fn coinbase_target(&self) -> u64 {
        self.metadata.coinbase_target()
    }

    /// Returns the proof target for this block.
    pub const fn proof_target(&self) -> u64 {
        self.metadata.proof_target()
    }

    /// Returns the coinbase target of the last coinbase.
    pub const fn last_coinbase_target(&self) -> u64 {
        self.metadata.last_coinbase_target()
    }

    /// Returns the Unix timestamp (UTC) of the last coinbase.
    pub const fn last_coinbase_timestamp(&self) -> i64 {
        self.metadata.last_coinbase_timestamp()
    }

    /// Returns the Unix timestamp (UTC) for this block.
    pub const fn timestamp(&self) -> i64 {
        self.metadata.timestamp()
    }
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;

    type CurrentNetwork = console::network::MainnetV0;

    /// Samples a block header.
    pub(crate) fn sample_block_header(rng: &mut TestRng) -> Header<CurrentNetwork> {
        *crate::test_helpers::sample_genesis_block(rng).header()
    }
}
