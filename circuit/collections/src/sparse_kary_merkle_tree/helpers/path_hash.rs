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
use snarkvm_circuit_algorithms::{BHP, Hash, Keccak, Poseidon};

/// A trait for a Merkle path hash function.
pub trait PathHash<E: Environment> {
    type Hash: Clone
        + Default
        + Inject<Primitive = <Self::Primitive as console::sparse_kary_merkle_tree::PathHash>::Hash>
        + Eject<Primitive = <Self::Primitive as console::sparse_kary_merkle_tree::PathHash>::Hash>
        + Equal<Output = Boolean<E>>
        + Ternary<Boolean = Boolean<E>, Output = Self::Hash>;
    type Primitive: console::sparse_kary_merkle_tree::PathHash;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, children: &[Self::Hash]) -> Self::Hash;

    /// Returns the empty hash.
    fn hash_empty<const ARITY: u8>(&self) -> Self::Hash {
        let children = vec![Self::Hash::default(); ARITY as usize];
        self.hash_children(&children)
    }
}

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> PathHash<E> for BHP<E, NUM_WINDOWS, WINDOW_SIZE> {
    type Hash = Field<E>;
    type Primitive = console::algorithms::BHP<E::Network, NUM_WINDOWS, WINDOW_SIZE>;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, children: &[Self::Hash]) -> Self::Hash {
        let mut input = Vec::new();
        // Prepend the nodes with a `true` bit.
        input.push(Boolean::constant(true));
        for child in children {
            child.write_bits_le(&mut input);
        }
        // Hash the input.
        Hash::hash(self, &input)
    }
}

impl<E: Environment, const RATE: usize> PathHash<E> for Poseidon<E, RATE> {
    type Hash = Field<E>;
    type Primitive = console::algorithms::Poseidon<E::Network, RATE>;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, children: &[Self::Hash]) -> Self::Hash {
        let mut input = Vec::with_capacity(1 + children.len());
        // Prepend the nodes with a `1field` byte.
        input.push(Self::Hash::one());
        for child in children {
            input.push(child.clone());
        }
        // Hash the input.
        Hash::hash(self, &input)
    }
}

impl<E: Environment, const TYPE: u8, const VARIANT: usize> PathHash<E> for Keccak<E, TYPE, VARIANT> {
    type Hash = BooleanHash<E, VARIANT>;
    type Primitive = console::algorithms::Keccak<TYPE, VARIANT>;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, children: &[Self::Hash]) -> Self::Hash {
        let mut input = Vec::new();
        // Prepend the nodes with a `true` bit.
        input.push(Boolean::constant(true));
        for child in children {
            child.write_bits_le(&mut input);
        }
        // Hash the input.
        let output = Hash::hash(self, &input);
        // Read the first VARIANT bits.
        let mut result = BooleanHash::default();
        result.0.clone_from_slice(&output[..VARIANT]);
        result
    }
}
