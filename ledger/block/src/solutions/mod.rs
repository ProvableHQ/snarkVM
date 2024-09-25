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
mod merkle;
mod serialize;
mod string;

use console::{network::prelude::*, types::Field};
use ledger_committee::Committee;
use ledger_narwhal_batch_header::BatchHeader;
use ledger_puzzle::{PuzzleSolutions, SolutionID};

#[derive(Clone, Eq, PartialEq)]
pub struct Solutions<N: Network> {
    /// The prover solutions for the puzzle.
    solutions: Option<PuzzleSolutions<N>>,
}

impl<N: Network> Solutions<N> {
    /// The maximum number of aborted solutions allowed in a block.
    pub const MAX_ABORTED_SOLUTIONS: usize = BatchHeader::<N>::MAX_TRANSMISSIONS_PER_BATCH
        * BatchHeader::<N>::MAX_GC_ROUNDS
        * Committee::<N>::MAX_COMMITTEE_SIZE as usize;
}

impl<N: Network> From<Option<PuzzleSolutions<N>>> for Solutions<N> {
    /// Initializes a new instance of the solutions.
    fn from(solutions: Option<PuzzleSolutions<N>>) -> Self {
        // Return the solutions.
        Self { solutions }
    }
}

impl<N: Network> Solutions<N> {
    /// Initializes a new instance of the solutions.
    pub fn new(solutions: PuzzleSolutions<N>) -> Result<Self> {
        // Return the solutions.
        Ok(Self { solutions: Some(solutions) })
    }

    /// Returns `true` if the solutions are empty.
    pub fn is_empty(&self) -> bool {
        self.solutions.is_none()
    }

    /// Returns the number of solutions.
    pub fn len(&self) -> usize {
        match &self.solutions {
            Some(solutions) => solutions.len(),
            None => 0,
        }
    }
}

impl<N: Network> Solutions<N> {
    /// Returns an iterator over the solution IDs.
    pub fn solution_ids<'a>(&'a self) -> Box<dyn Iterator<Item = &'a SolutionID<N>> + 'a> {
        match &self.solutions {
            Some(solutions) => Box::new(solutions.keys()),
            None => Box::new(std::iter::empty::<&SolutionID<N>>()),
        }
    }
}

impl<N: Network> Deref for Solutions<N> {
    type Target = Option<PuzzleSolutions<N>>;

    /// Returns the solutions.
    fn deref(&self) -> &Self::Target {
        &self.solutions
    }
}
