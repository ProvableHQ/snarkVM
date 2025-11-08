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

// Re-export k-ary PathHash and LeafHash from kary_merkle_tree
pub use crate::kary_merkle_tree::{LeafHash, PathHash};

use snarkvm_circuit_algorithms::{BHP, Hash, Poseidon};
use snarkvm_circuit_types::{Boolean, Field, environment::prelude::*};

/// A trait for a Sparse K-ary Merkle key hash function.
pub trait KeyHash<E: Environment> {
    type Hash: FieldTrait;
    type Key;

    /// Returns the hash of the given key.
    fn hash_key(&self, key: &Self::Key) -> Self::Hash;
}

// Implement KeyHash for common key types using BHP.
impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> KeyHash<E> for BHP<E, NUM_WINDOWS, WINDOW_SIZE> {
    type Hash = Field<E>;
    type Key = Vec<Boolean<E>>;

    /// Returns the hash of the given key.
    fn hash_key(&self, key: &Self::Key) -> Self::Hash {
        let mut input = Vec::with_capacity(1 + key.len());
        // Prepend the key with a `false` bit to distinguish from leaves.
        input.push(Boolean::constant(false));
        input.extend_from_slice(key);
        // Hash the input.
        Hash::hash(self, &input)
    }
}

// Implement KeyHash for Field keys using Poseidon.
impl<E: Environment, const RATE: usize> KeyHash<E> for Poseidon<E, RATE> {
    type Hash = Field<E>;
    type Key = Field<E>;

    /// Returns the hash of the given key.
    fn hash_key(&self, key: &Self::Key) -> Self::Hash {
        // For Field keys, we can use the field value directly or hash it.
        // Here we hash it to ensure uniform distribution.
        let input = &[Self::Hash::zero(), key.clone()];
        Hash::hash(self, input)
    }
}
