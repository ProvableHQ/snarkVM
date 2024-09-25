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

use snarkvm_console_algorithms::{Poseidon, BHP};
use snarkvm_console_types::prelude::*;

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

/// A trait for a Merkle path hash function.
pub trait PathHash: Clone + Send + Sync {
    type Hash: FieldTrait;

    /// Returns the empty hash.
    fn hash_empty(&self) -> Result<Self::Hash> {
        self.hash_children(&Self::Hash::zero(), &Self::Hash::zero())
    }

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, left: &Self::Hash, right: &Self::Hash) -> Result<Self::Hash>;

    /// Returns the hash for each tuple of child nodes.
    fn hash_all_children(&self, child_nodes: &[(Self::Hash, Self::Hash)]) -> Result<Vec<Self::Hash>> {
        match child_nodes.len() {
            0 => Ok(vec![]),
            1..=100 => child_nodes.iter().map(|(left, right)| self.hash_children(left, right)).collect(),
            _ => cfg_iter!(child_nodes).map(|(left, right)| self.hash_children(left, right)).collect(),
        }
    }
}

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> PathHash for BHP<E, NUM_WINDOWS, WINDOW_SIZE> {
    type Hash = Field<E>;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, left: &Self::Hash, right: &Self::Hash) -> Result<Self::Hash> {
        let mut input = Vec::with_capacity(1 + <Self::Hash as SizeInBits>::size_in_bits() * 2);
        // Prepend the nodes with a `true` bit.
        input.push(true);
        left.write_bits_le(&mut input);
        right.write_bits_le(&mut input);
        // Hash the input.
        Hash::hash(self, &input)
    }
}

impl<E: Environment, const RATE: usize> PathHash for Poseidon<E, RATE> {
    type Hash = Field<E>;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, left: &Self::Hash, right: &Self::Hash) -> Result<Self::Hash> {
        // Prepend the nodes with a `1field` byte.
        let input = &[Self::Hash::one(), *left, *right];
        // Hash the input.
        Hash::hash(self, input)
    }
}
