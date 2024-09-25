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

mod hash;

#[cfg(all(test, console))]
use snarkvm_circuit_types::environment::assert_scope;
#[cfg(test)]
use snarkvm_utilities::{TestRng, Uniform};

use crate::Hash;
use snarkvm_circuit_types::{environment::prelude::*, Boolean, U64};

/// The Keccak-224 hash function.
pub type Keccak224<E> = Keccak<E, { KeccakType::Keccak as u8 }, 224>;
/// The Keccak-256 hash function.
pub type Keccak256<E> = Keccak<E, { KeccakType::Keccak as u8 }, 256>;
/// The Keccak-384 hash function.
pub type Keccak384<E> = Keccak<E, { KeccakType::Keccak as u8 }, 384>;
/// The Keccak-512 hash function.
pub type Keccak512<E> = Keccak<E, { KeccakType::Keccak as u8 }, 512>;

/// The SHA3-224 hash function.
pub type Sha3_224<E> = Keccak<E, { KeccakType::Sha3 as u8 }, 224>;
/// The SHA3-256 hash function.
pub type Sha3_256<E> = Keccak<E, { KeccakType::Sha3 as u8 }, 256>;
/// The SHA3-384 hash function.
pub type Sha3_384<E> = Keccak<E, { KeccakType::Sha3 as u8 }, 384>;
/// The SHA3-512 hash function.
pub type Sha3_512<E> = Keccak<E, { KeccakType::Sha3 as u8 }, 512>;

/// A helper to specify the hash type.
enum KeccakType {
    Keccak,
    Sha3,
}

/// The rows and columns are 5-bit lanes.
const MODULO: usize = 5;
/// The permutation type `l`.
const L: usize = 6;
/// The number of rounds in a full-round operation.
const NUM_ROUNDS: usize = 12 + 2 * L;
/// The permutation width `b`.
const PERMUTATION_WIDTH: usize = 1600;

/// The sponge construction `Sponge[f, pad, r]` is a function that takes a variable-length input
/// and produces a fixed-length output (the hash value).
///
/// The permutation `f` is a function that takes a fixed-length input and produces a fixed-length output,
/// defined as `f = Keccak-f[b]`, where `b := 25 * 2^l` is the width of the permutation,
/// and `l` is the log width of the permutation.
/// For our case, `l = 6`, thus `b = 1600`.
///
/// The padding rule `pad` is a function that takes a variable-length input and produces a fixed-length output.
/// In Keccak, `pad` is a multi-rate padding, defined as `pad(M) = M || 0x01 || 0x00…0x00 || 0x80`,
/// where `M` is the input data, and `0x01 || 0x00…0x00 || 0x80` is the padding.
/// In SHA-3, `pad` is a SHAKE, defined as `pad(M) = M || 0x06 || 0x00…0x00 || 0x80`,
/// where `M` is the input data, and `0x06 || 0x00…0x00 || 0x80` is the padding.
///
/// The bitrate `r` is the number of bits that are absorbed into the sponge state in each iteration
/// of the absorbing phase.
///
/// In addition, the capacity is defined as `c := b - r`.
#[derive(Clone, Debug, Default)]
pub struct Keccak<E: Environment, const TYPE: u8, const VARIANT: usize> {
    /// The round constants `RC[t] ∈ GF(2)` are defined as the
    /// output of a linear feedback shift register (LFSR).
    round_constants: Vec<U64<E>>,
    /// Precomputations for the ρ step.
    rotl: [usize; MODULO * MODULO],
}

impl<E: Environment, const TYPE: u8, const VARIANT: usize> Keccak<E, TYPE, VARIANT> {
    /// Initializes a new Keccak hash function.
    pub fn new() -> Self {
        Self {
            round_constants: Self::ROUND_CONSTANTS.into_iter().map(|e| U64::constant(console::U64::new(e))).collect(),
            rotl: Self::rotl_offsets(),
        }
    }
}

impl<E: Environment, const TYPE: u8, const VARIANT: usize> Keccak<E, TYPE, VARIANT> {
    /// The values `ROUND_CONSTANTS[t] ∈ GF(2)` are defined as the
    /// output of a binary linear feedback shift register (LFSR):
    /// ```text
    /// ROUND_CONSTANTS[t] = (x^t) mod (x^8 + x^6 + x^5 + x^4 + 1) mod x in GF(2)[x]
    /// ```
    /// where `t ∈ {0, 1, …, NUM_ROUNDS}`.
    const ROUND_CONSTANTS: [u64; NUM_ROUNDS] = [
        0x0000000000000001,
        0x0000000000008082,
        0x800000000000808A,
        0x8000000080008000,
        0x000000000000808B,
        0x0000000080000001,
        0x8000000080008081,
        0x8000000000008009,
        0x000000000000008A,
        0x0000000000000088,
        0x0000000080008009,
        0x000000008000000A,
        0x000000008000808B,
        0x800000000000008B,
        0x8000000000008089,
        0x8000000000008003,
        0x8000000000008002,
        0x8000000000000080,
        0x000000000000800A,
        0x800000008000000A,
        0x8000000080008081,
        0x8000000000008080,
        0x0000000080000001,
        0x8000000080008008,
    ];

    /// Returns the ROTL offsets for the ρ step.
    ///
    /// The offsets are defined as follows:
    /// ```text
    /// for t = 0 to 23 do
    ///   offset[t] = (t + 1)(t + 2)/2
    /// end for
    /// ```
    ///
    /// This method transposes the offsets to match the access pattern (i.e. for y, then for x).
    fn rotl_offsets() -> [usize; MODULO * MODULO] {
        let mut rotl = [0; MODULO * MODULO];
        let mut x: usize = 1;
        let mut y: usize = 0;

        for t in 0..=23 {
            let rotr = ((t + 1) * (t + 2) / 2) % 64;
            rotl[x + (y * MODULO)] = (64 - rotr) % 64;

            // Update x and y.
            let x_old = x;
            x = y;
            y = (2 * x_old + 3 * y) % MODULO;
        }
        rotl
    }
}
