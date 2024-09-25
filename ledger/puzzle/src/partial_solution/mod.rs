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
mod serialize;
mod string;

use crate::SolutionID;
use console::{account::Address, network::prelude::*, prelude::DeserializeExt};

/// The partial solution for the puzzle from a prover.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct PartialSolution<N: Network> {
    /// The solution ID.
    solution_id: SolutionID<N>,
    /// The epoch hash.
    epoch_hash: N::BlockHash,
    /// The address of the prover.
    address: Address<N>,
    /// The counter for the solution.
    counter: u64,
}

impl<N: Network> PartialSolution<N> {
    /// Initializes a new instance of the partial solution.
    pub fn new(epoch_hash: N::BlockHash, address: Address<N>, counter: u64) -> Result<Self> {
        // Compute the solution ID.
        let solution_id = SolutionID::new(epoch_hash, address, counter)?;
        // Return the partial solution.
        Ok(Self { solution_id, epoch_hash, address, counter })
    }

    /// Returns the solution ID.
    pub const fn id(&self) -> SolutionID<N> {
        self.solution_id
    }

    /// Returns the epoch hash of the solution.
    pub const fn epoch_hash(&self) -> N::BlockHash {
        self.epoch_hash
    }

    /// Returns the address of the prover.
    pub const fn address(&self) -> Address<N> {
        self.address
    }

    /// Returns the counter for the solution.
    pub const fn counter(&self) -> u64 {
        self.counter
    }
}
