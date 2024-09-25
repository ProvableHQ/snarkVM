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

#[cfg(test)]
use snarkvm_utilities::Uniform;

use crate::Hash;
use snarkvm_console_types::environment::prelude::*;

use tiny_keccak::{Hasher, Keccak as TinyKeccak, Sha3 as TinySha3};

/// The Keccak-224 hash function.
pub type Keccak224 = Keccak<{ KeccakType::Keccak as u8 }, 224>;
/// The Keccak-256 hash function.
pub type Keccak256 = Keccak<{ KeccakType::Keccak as u8 }, 256>;
/// The Keccak-384 hash function.
pub type Keccak384 = Keccak<{ KeccakType::Keccak as u8 }, 384>;
/// The Keccak-512 hash function.
pub type Keccak512 = Keccak<{ KeccakType::Keccak as u8 }, 512>;

/// The SHA3-224 hash function.
pub type Sha3_224 = Keccak<{ KeccakType::Sha3 as u8 }, 224>;
/// The SHA3-256 hash function.
pub type Sha3_256 = Keccak<{ KeccakType::Sha3 as u8 }, 256>;
/// The SHA3-384 hash function.
pub type Sha3_384 = Keccak<{ KeccakType::Sha3 as u8 }, 384>;
/// The SHA3-512 hash function.
pub type Sha3_512 = Keccak<{ KeccakType::Sha3 as u8 }, 512>;

/// A helper to specify the hash type.
enum KeccakType {
    Keccak,
    Sha3,
}

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
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Keccak<const TYPE: u8, const VARIANT: usize>;
