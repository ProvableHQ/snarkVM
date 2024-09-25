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

use crate::{PartialSolution, SolutionID};
use console::{account::Address, network::prelude::*, prelude::DeserializeExt};

/// A helper struct around a puzzle solution.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Solution<N: Network> {
    /// The partial solution.
    partial_solution: PartialSolution<N>,
    /// The solution target.
    pub(super) target: u64,
}

impl<N: Network> Solution<N> {
    /// Initializes a new instance of the solution.
    pub fn new(partial_solution: PartialSolution<N>, target: u64) -> Self {
        Self { partial_solution, target }
    }

    /// Returns the partial solution..
    pub const fn partial_solution(&self) -> &PartialSolution<N> {
        &self.partial_solution
    }

    /// Returns the solution ID.
    pub const fn id(&self) -> SolutionID<N> {
        self.partial_solution.id()
    }

    /// Returns the epoch hash of the solution.
    pub const fn epoch_hash(&self) -> N::BlockHash {
        self.partial_solution.epoch_hash()
    }

    /// Returns the address of the prover.
    pub const fn address(&self) -> Address<N> {
        self.partial_solution.address()
    }

    /// Returns the counter for the solution.
    pub const fn counter(&self) -> u64 {
        self.partial_solution.counter()
    }

    /// Returns the target for the solution.
    pub const fn target(&self) -> u64 {
        self.target
    }
}
