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

use snarkvm_console_algorithms::{BHP, Keccak, Poseidon};
use snarkvm_console_types::prelude::*;

/// A trait for a key hash function.
/// Keys are hashed to field elements to determine their path through the sparse Merkle tree.
/// This ensures that keys can be efficiently represented in full to prevent collisions.
pub trait KeyHash: Clone + Send + Sync {
    type Hash: Copy
        + Clone
        + Debug
        + Default
        + PartialEq
        + Eq
        + Ord
        + PartialOrd
        + FromBytes
        + ToBytes
        + ToBits
        + Send
        + Sync;
    type Key: Clone + Send + Sync;

    /// Returns the hash of the given key.
    fn hash_key(&self, key: &Self::Key) -> Result<Self::Hash>;
}

impl<E: Environment, const RATE: usize> KeyHash for Poseidon<E, RATE> {
    type Hash = Field<E>;
    type Key = Field<E>;

    /// Returns the hash of the given key.
    /// For field element keys, we hash them to prevent under-traversing the path.
    fn hash_key(&self, key: &Self::Key) -> Result<Self::Hash> {
        // Hash the key with a domain separator.
        let mut input = Vec::with_capacity(2);
        // Use field element 2 as domain separator for keys.
        input.push(Field::<E>::from_u8(2));
        input.push(*key);
        Hash::hash(self, &input)
    }
}

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> KeyHash for BHP<E, NUM_WINDOWS, WINDOW_SIZE> {
    type Hash = Field<E>;
    type Key = Vec<bool>;

    /// Returns the hash of the given key.
    fn hash_key(&self, key: &Self::Key) -> Result<Self::Hash> {
        let mut input = Vec::with_capacity(2 + key.len());
        // Prepend with two `true` bits as domain separator for keys.
        input.push(true);
        input.push(true);
        input.extend(key);
        // Hash the input.
        Hash::hash(self, &input)
    }
}

impl<const TYPE: u8, const VARIANT: usize> KeyHash for Keccak<TYPE, VARIANT> {
    type Hash = Field<Console>;
    type Key = Vec<bool>;

    /// Returns the hash of the given key.
    fn hash_key(&self, key: &Self::Key) -> Result<Self::Hash> {
        let mut input = Vec::with_capacity(2 + key.len());
        // Prepend with two `true` bits as domain separator for keys.
        input.push(true);
        input.push(true);
        input.extend(key);
        // Hash the input, then convert bits to field element.
        // Use only 252 bits to ensure the result fits in the field.
        let output = Hash::hash(self, &input)?;
        let bits_to_use = std::cmp::min(252, output.len());
        Field::from_bits_le(&output[..bits_to_use])
    }
}
