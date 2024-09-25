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

pub mod hasher;
use hasher::BHPHasher;

mod commit;
mod commit_uncompressed;
mod hash;
mod hash_uncompressed;

use snarkvm_console_types::prelude::*;

use std::sync::Arc;

const BHP_CHUNK_SIZE: usize = 3;

/// BHP256 is a collision-resistant hash function that processes 256-bit chunks.
pub type BHP256<E> = BHP<E, 3, 57>; // Supports inputs up to 261 bits (1 u8 + 1 Fq).
/// BHP512 is a collision-resistant hash function that processes inputs in 512-bit chunks.
pub type BHP512<E> = BHP<E, 6, 43>; // Supports inputs up to 522 bits (2 u8 + 2 Fq).
/// BHP768 is a collision-resistant hash function that processes inputs in 768-bit chunks.
pub type BHP768<E> = BHP<E, 15, 23>; // Supports inputs up to 783 bits (3 u8 + 3 Fq).
/// BHP1024 is a collision-resistant hash function that processes inputs in 1024-bit chunks.
pub type BHP1024<E> = BHP<E, 8, 54>; // Supports inputs up to 1044 bits (4 u8 + 4 Fq).

/// BHP is a collision-resistant hash function that takes a variable-length input.
/// The BHP hash function does *not* behave like a random oracle, see Poseidon for one.
///
/// ## Design
/// The BHP hash function splits the given input into blocks, and processes them iteratively.
///
/// The first iteration is initialized as follows:
/// ```text
/// DIGEST_0 = BHP([ 0...0 || DOMAIN || LENGTH(INPUT) || INPUT[0..BLOCK_SIZE] ]);
/// ```
/// Each subsequent iteration is initialized as follows:
/// ```text
/// DIGEST_N+1 = BHP([ DIGEST_N[0..DATA_BITS] || INPUT[(N+1)*BLOCK_SIZE..(N+2)*BLOCK_SIZE] ]);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct BHP<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> {
    /// The domain separator for the BHP hash function.
    domain: Vec<bool>,
    /// The internal BHP hasher used to process one iteration.
    hasher: BHPHasher<E, NUM_WINDOWS, WINDOW_SIZE>,
}

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> BHP<E, NUM_WINDOWS, WINDOW_SIZE> {
    /// Initializes a new instance of BHP with the given domain.
    pub fn setup(domain: &str) -> Result<Self> {
        // Ensure the given domain is within the allowed size in bits.
        let num_bits = domain.len().saturating_mul(8);
        let max_bits = Field::<E>::size_in_data_bits() - 64; // 64 bits encode the length.
        ensure!(num_bits <= max_bits, "Domain cannot exceed {max_bits} bits, found {num_bits} bits");

        // Initialize the BHP hasher.
        let hasher = BHPHasher::<E, NUM_WINDOWS, WINDOW_SIZE>::setup(domain)?;

        // Convert the domain into a boolean vector.
        let mut domain = domain.as_bytes().to_bits_le();
        // Pad the domain with zeros up to the maximum size in bits.
        domain.resize(max_bits, false);
        // Reverse the domain so that it is: [ 0...0 || DOMAIN ].
        // (For advanced users): This optimizes the initial costs during hashing.
        domain.reverse();

        Ok(Self { domain, hasher })
    }

    /// Returns the domain separator for the BHP hash function.
    pub fn domain(&self) -> &[bool] {
        &self.domain
    }

    /// Returns the bases.
    pub fn bases(&self) -> &Arc<Vec<Vec<Group<E>>>> {
        self.hasher.bases()
    }

    /// Returns the random base window.
    pub fn random_base(&self) -> &Arc<Vec<Group<E>>> {
        self.hasher.random_base()
    }

    /// Returns the number of windows.
    pub fn num_windows(&self) -> u8 {
        NUM_WINDOWS
    }

    /// Returns the window size.
    pub fn window_size(&self) -> u8 {
        WINDOW_SIZE
    }
}
