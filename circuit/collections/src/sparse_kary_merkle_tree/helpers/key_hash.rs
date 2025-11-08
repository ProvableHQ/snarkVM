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
use snarkvm_circuit_algorithms::{BHP, Hash, Poseidon};

/// A trait for a key hash function in circuits.
pub trait KeyHash<E: Environment> {
    type Hash: Clone + Default + Inject + Eject + ToBits<Boolean = Boolean<E>>;
    type Key: Clone;
    type Primitive: console::sparse_kary_merkle_tree::KeyHash;

    /// Returns the hash of the given key.
    fn hash_key(&self, key: &Self::Key) -> Self::Hash;
}

impl<E: Environment, const RATE: usize> KeyHash<E> for Poseidon<E, RATE> {
    type Hash = Field<E>;
    type Key = Field<E>;
    type Primitive = console::algorithms::Poseidon<E::Network, RATE>;

    /// Returns the hash of the given key.
    fn hash_key(&self, key: &Self::Key) -> Self::Hash {
        let mut input = Vec::with_capacity(2);
        // Use field element 2 as domain separator for keys.
        input.push(Field::<E>::one() + Field::<E>::one());
        input.push(key.clone());
        Hash::hash(self, &input)
    }
}

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> KeyHash<E> for BHP<E, NUM_WINDOWS, WINDOW_SIZE> {
    type Hash = Field<E>;
    type Key = Vec<Boolean<E>>;
    type Primitive = console::algorithms::BHP<E::Network, NUM_WINDOWS, WINDOW_SIZE>;

    /// Returns the hash of the given key.
    fn hash_key(&self, key: &Self::Key) -> Self::Hash {
        let mut input = Vec::with_capacity(2 + key.len());
        // Prepend with two `true` bits as domain separator for keys.
        input.push(Boolean::constant(true));
        input.push(Boolean::constant(true));
        input.extend_from_slice(key);
        // Hash the input.
        Hash::hash(self, &input)
    }
}
