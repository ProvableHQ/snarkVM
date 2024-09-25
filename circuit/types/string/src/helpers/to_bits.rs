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

impl<E: Environment> ToBits for StringType<E> {
    type Boolean = Boolean<E>;

    /// Outputs the little-endian bit representation of `self` *with* trailing zeros (to byte-alignment).
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        (&self).write_bits_le(vec);
    }

    /// Outputs the big-endian bit representation of `self` *with* leading zeros (to byte-alignment).
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        (&self).write_bits_be(vec);
    }
}

impl<E: Environment> ToBits for &StringType<E> {
    type Boolean = Boolean<E>;

    /// Outputs the little-endian bit representation of `self` *with* trailing zeros (to byte-alignment).
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        self.bytes.write_bits_le(vec);
    }

    /// Outputs the big-endian bit representation of `self` *with* leading zeros (to byte-alignment).
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        self.bytes.write_bits_be(vec);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u32 = 128;

    fn check_to_bits_le(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random string. Take 1/4th to ensure we fit for all code points.
            let expected = rng.next_string(Circuit::MAX_STRING_BYTES / 4, false);
            let expected_num_bytes = expected.len();
            assert!(expected_num_bytes <= Circuit::MAX_STRING_BYTES as usize);

            let candidate = StringType::<Circuit>::new(mode, console::StringType::new(&expected));

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = candidate.to_bits_le();
                assert_eq!(expected_num_bytes * 8, candidate.len());

                // Ensure every bit matches.
                for (expected_bit, candidate_bit) in expected.to_bits_le().iter().zip_eq(candidate.iter()) {
                    assert_eq!(expected_bit, &candidate_bit.eject_value());
                }
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
    }

    fn check_to_bits_be(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random string. Take 1/4th to ensure we fit for all code points.
            let expected = rng.next_string(Circuit::MAX_STRING_BYTES / 4, false);
            let expected_num_bytes = expected.len();
            assert!(expected_num_bytes <= Circuit::MAX_STRING_BYTES as usize);

            let candidate = StringType::<Circuit>::new(mode, console::StringType::new(&expected));

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = candidate.to_bits_be();
                assert_eq!(expected_num_bytes * 8, candidate.len());

                // Ensure every bit matches.
                for (expected_bit, candidate_bit) in expected.to_bits_be().iter().zip_eq(candidate.iter()) {
                    assert_eq!(expected_bit, &candidate_bit.eject_value());
                }
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
    }

    #[test]
    fn test_string_to_bits_le_constant() {
        check_to_bits_le(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_string_to_bits_le_public() {
        check_to_bits_le(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_string_to_bits_le_private() {
        check_to_bits_le(Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_string_to_bits_be_constant() {
        check_to_bits_be(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_string_to_bits_be_public() {
        check_to_bits_be(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_string_to_bits_be_private() {
        check_to_bits_be(Mode::Private, 0, 0, 0, 0);
    }
}
