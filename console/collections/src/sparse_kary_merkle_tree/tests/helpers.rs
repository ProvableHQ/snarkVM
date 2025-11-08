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

/// Generates a deterministic key from an index to avoid collisions in sparse trees.
/// For a tree of depth D and arity A, we need D * log2(A) bits to uniquely identify positions.
/// This function creates a minimal key that directly encodes the index in the first bits,
/// ensuring that after hashing, it maps to a unique position.
pub fn generate_unique_bool_key<const DEPTH: u8, const ARITY: u8>(index: u64) -> Vec<bool> {
    let bits_needed = ((DEPTH as f64) * (ARITY as f64).log2()).ceil() as usize;
    // Create a key with just enough bits + some padding for hashing
    let key_size = bits_needed.max(10).min(253);
    let mut key = vec![false; key_size];
    
    // Set bits based on index to create unique keys
    // We set the bits directly in little-endian order
    for bit_idx in 0..bits_needed.min(64) {
        if bit_idx < key_size && (index >> bit_idx) & 1 == 1 {
            key[bit_idx] = true;
        }
    }
    
    key
}

/// Generates a deterministic Field key from an index.
pub fn generate_unique_field_key<E: Environment>(index: u128) -> Field<E> {
    // Use a simple formula to generate unique field elements
    Field::<E>::from_u128(index)
}

