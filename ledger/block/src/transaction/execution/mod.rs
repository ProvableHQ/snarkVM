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

use crate::{Transaction, Transition};
use console::{account::Field, network::prelude::*, program::ProgramID};
use synthesizer_snark::Proof;

use indexmap::IndexMap;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Execution<N: Network> {
    /// The transitions.
    transitions: IndexMap<N::TransitionID, Transition<N>>,
    /// The global state root.
    global_state_root: N::StateRoot,
    /// The proof.
    proof: Option<Proof<N>>,
}

impl<N: Network> Execution<N> {
    /// Initialize a new `Execution` instance.
    pub fn new() -> Self {
        Self { transitions: Default::default(), global_state_root: Default::default(), proof: None }
    }

    /// Initializes a new `Execution` instance with the given transitions.
    pub fn from(
        transitions: impl Iterator<Item = Transition<N>>,
        global_state_root: N::StateRoot,
        proof: Option<Proof<N>>,
    ) -> Result<Self> {
        // Construct the execution.
        let execution = Self { transitions: transitions.map(|t| (*t.id(), t)).collect(), global_state_root, proof };
        // Ensure the transitions are not empty.
        ensure!(!execution.transitions.is_empty(), "Execution cannot initialize from empty list of transitions");
        // Return the new `Execution` instance.
        Ok(execution)
    }

    /// Returns the size in bytes.
    pub fn size_in_bytes(&self) -> Result<u64> {
        Ok(u64::try_from(self.to_bytes_le()?.len())?)
    }

    /// Returns the global state root.
    pub const fn global_state_root(&self) -> N::StateRoot {
        self.global_state_root
    }

    /// Returns the proof.
    pub const fn proof(&self) -> Option<&Proof<N>> {
        self.proof.as_ref()
    }

    /// Returns the execution ID.
    pub fn to_execution_id(&self) -> Result<Field<N>> {
        Ok(*Transaction::execution_tree(self, &None)?.root())
    }
}

impl<N: Network> Execution<N> {
    /// Returns `true` if the execution contains the transition for the given transition ID.
    pub fn contains_transition(&self, transition_id: &N::TransitionID) -> bool {
        self.transitions.contains_key(transition_id)
    }

    /// Returns the `Transition` corresponding to the given transition ID. This method is `O(1)`.
    pub fn get_transition(&self, transition_id: &N::TransitionID) -> Option<&Transition<N>> {
        self.transitions.get(transition_id)
    }

    /// Returns the program ID corresponding to the given transition ID. This method is `O(1)`.
    pub fn get_program_id(&self, transition_id: &N::TransitionID) -> Option<&ProgramID<N>> {
        self.transitions.get(transition_id).map(|t| t.program_id())
    }

    /// Returns the `Transition` at the given index.
    pub fn get(&self, index: usize) -> Result<&Transition<N>> {
        match self.transitions.get_index(index) {
            Some((_, transition)) => Ok(transition),
            None => bail!("Transition index {index} out of bounds in the execution object"),
        }
    }

    /// Returns the next `Transition` in the execution.
    pub fn peek(&self) -> Result<&Transition<N>> {
        self.get(self.len() - 1)
    }

    /// Appends the given `Transition` to the execution.
    pub fn push(&mut self, transition: Transition<N>) {
        self.transitions.insert(*transition.id(), transition);
    }

    /// Pops the last `Transition` from the execution.
    pub fn pop(&mut self) -> Result<Transition<N>> {
        match self.transitions.pop() {
            Some((_, transition)) => Ok(transition),
            None => bail!("Cannot pop a transition from an empty execution object"),
        }
    }

    /// Returns the number of transitions in the execution.
    pub fn len(&self) -> usize {
        self.transitions.len()
    }

    /// Returns `true` if the execution is empty.
    pub fn is_empty(&self) -> bool {
        self.transitions.is_empty()
    }
}

impl<N: Network> Execution<N> {
    /// Returns a consuming iterator over the underlying transitions.
    pub fn into_transitions(self) -> impl ExactSizeIterator + DoubleEndedIterator<Item = Transition<N>> {
        self.transitions.into_values()
    }

    /// Returns an iterator over the underlying transitions.
    pub fn transitions(&self) -> impl '_ + ExactSizeIterator + DoubleEndedIterator<Item = &Transition<N>> {
        self.transitions.values()
    }

    /// Returns an iterator over the commitments.
    pub fn commitments(&self) -> impl '_ + Iterator<Item = &Field<N>> {
        self.transitions.values().flat_map(Transition::commitments)
    }
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;

    type CurrentNetwork = console::network::MainnetV0;

    /// Samples a random execution.
    pub(crate) fn sample_execution(rng: &mut TestRng) -> Execution<CurrentNetwork> {
        // Sample the genesis block.
        let block = crate::test_helpers::sample_genesis_block(rng);
        // Retrieve a transaction.
        let transaction = block.transactions().iter().next().unwrap().deref().clone();
        // Retrieve the execution.
        if let Transaction::Execute(_, execution, _) = transaction { execution } else { unreachable!() }
    }
}
