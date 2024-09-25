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

use super::*;

impl<E: Environment> Boolean<E> {
    /// Returns `true` if `circuit_bits_le <= constant_bits_le`.
    /// This *internal* function assumes the inputs are in **little-endian** representation.
    #[doc(hidden)]
    pub fn is_less_than_or_equal_constant(circuit_bits_le: &[Boolean<E>], constant_bits_le: &[bool]) -> Boolean<E> {
        // Ensure the length matches.
        if circuit_bits_le.len() != constant_bits_le.len() {
            E::halt(format!("Mismatching length of bits ({} != {})", circuit_bits_le.len(), constant_bits_le.len()))
        }

        // Compute `!(constant_bits_le < circuit_bits_le)`, equivalent to `constant_bits_le >= circuit_bits_le`.
        !constant_bits_le.iter().zip_eq(circuit_bits_le).fold(Boolean::constant(false), |rest_is_less, (this, that)| {
            if *this { that.bitand(&rest_is_less) } else { that.bitor(&rest_is_less) }
        })
    }

    /// Asserts that `circuit_bits_le <= constant_bits_le`.
    /// This *internal* function assumes the inputs are in **little-endian** representation.
    #[doc(hidden)]
    pub fn assert_less_than_or_equal_constant(circuit_bits_le: &[Boolean<E>], constant_bits_le: &[bool]) {
        // Compute `!(constant_bits_le < circuit_bits_le)`, equivalent to `constant_bits_le >= circuit_bits_le`.
        let is_less_than_or_equal = Boolean::is_less_than_or_equal_constant(circuit_bits_le, constant_bits_le);
        // Assert that `circuit_bits_le <= constant_bits_le`.
        E::assert(is_less_than_or_equal);
    }
}
