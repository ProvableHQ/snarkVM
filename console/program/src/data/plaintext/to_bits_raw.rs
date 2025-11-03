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

impl<N: Network> ToBitsRaw for Plaintext<N> {
    /// Returns this plaintext as a list of **little-endian** bits without variant or identifier bits.
    fn write_bits_raw_le(&self, vec: &mut Vec<bool>) {
        match self {
            Self::Literal(literal, _) => {
                // Extend the vector with the bits.
                vec.extend_from_slice(&literal.to_bits_le())
            }
            Self::Struct(struct_, _) => {
                // Write each value of the struct.
                for (_, value) in struct_ {
                    vec.extend_from_slice(&value.to_bits_raw_le());
                }
            }
            Self::Array(array, _) => {
                // Write each element of the array.
                for element in array {
                    vec.extend_from_slice(&element.to_bits_raw_le());
                }
            }
        }
    }

    /// Returns this plaintext as a list of **big-endian** bits without variant or identifier bits.
    fn write_bits_raw_be(&self, vec: &mut Vec<bool>) {
        match self {
            Self::Literal(literal, _) => {
                // Extend the vector with the bits.
                vec.extend_from_slice(&literal.to_bits_be())
            }
            Self::Struct(struct_, _) => {
                // Write each value of the struct.
                for (_, value) in struct_ {
                    vec.extend_from_slice(&value.to_bits_raw_be());
                }
            }
            Self::Array(array, _) => {
                // Write each element of the array.
                for element in array {
                    // Write the element.
                    vec.extend_from_slice(&element.to_bits_raw_be());
                }
            }
        }
    }
}
