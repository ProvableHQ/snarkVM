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

impl<E: Environment> ToBits for Scalar<E> {
    type Boolean = Boolean<E>;

    /// Outputs the little-endian bit representation of `self` *without* trailing zeros.
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        (&self).write_bits_le(vec);
    }

    /// Outputs the big-endian bit representation of `self` *without* leading zeros.
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        (&self).write_bits_be(vec);
    }
}

impl<E: Environment> ToBits for &Scalar<E> {
    type Boolean = Boolean<E>;

    /// Outputs the little-endian bit representation of `self` *without* trailing zeros.
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        // Compute the bits of the scalar.
        let bits = self.bits_le.get_or_init(|| {
            // Note: We are reconstituting the scalar field into a base field.
            // This is safe as the scalar field modulus is less than the base field modulus,
            // and thus will always fit within a single base field element.
            debug_assert!(console::Scalar::<E::Network>::size_in_bits() < console::Field::<E::Network>::size_in_bits());

            // Construct a vector of `Boolean`s comprising the bits of the scalar value.
            let bits_le = self.field.to_lower_bits_le(console::Scalar::<E::Network>::size_in_bits());

            // Ensure the bit representation is unique.
            {
                // Retrieve the modulus & subtract by 1 as we'll check `bits_le` is less than or *equal* to this value.
                // (For advanced users) ScalarField::MODULUS - 1 is equivalent to -1 in the field.
                let modulus_minus_one = -E::ScalarField::one();
                // Assert `bits_le <= (ScalarField::MODULUS - 1)`, which is equivalent to `bits_le < ScalarField::MODULUS`.
                Boolean::assert_less_than_or_equal_constant(&bits_le, &modulus_minus_one.to_bits_le());
            }

            bits_le
        });
        // Extend the vector with the bits of the scalar.
        vec.extend_from_slice(bits)
    }

    /// Outputs the big-endian bit representation of `self` *without* leading zeros.
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        let initial_len = vec.len();
        self.write_bits_le(vec);
        vec[initial_len..].reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    fn check_to_bits_le(
        name: &str,
        expected: &[bool],
        candidate: &Scalar<Circuit>,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        let expected_number_of_bits = console::Scalar::<<Circuit as Environment>::Network>::size_in_bits();

        Circuit::scope(name, || {
            let candidate_bits = candidate.to_bits_le();
            assert_eq!(expected_number_of_bits, candidate_bits.len());
            for (expected_bit, candidate_bit) in expected.iter().zip_eq(candidate_bits.iter()) {
                assert_eq!(*expected_bit, candidate_bit.eject_value());
            }
            assert_scope!(num_constants, num_public, num_private, num_constraints);

            // Ensure a second call incurs no additional cost.
            let _candidate_bits = candidate.to_bits_le();
            assert_scope!(num_constants, num_public, num_private, num_constraints);
        });
    }

    fn check_to_bits_be(
        name: &str,
        expected: &[bool],
        candidate: Scalar<Circuit>,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        let expected_number_of_bits = console::Scalar::<<Circuit as Environment>::Network>::size_in_bits();

        Circuit::scope(name, || {
            let candidate_bits = candidate.to_bits_be();
            assert_eq!(expected_number_of_bits, candidate_bits.len());
            for (expected_bit, candidate_bit) in expected.iter().zip_eq(candidate_bits.iter()) {
                assert_eq!(*expected_bit, candidate_bit.eject_value());
            }
            assert_scope!(num_constants, num_public, num_private, num_constraints);

            // Ensure a second call incurs no additional cost.
            let _candidate_bits = candidate.to_bits_be();
            assert_scope!(num_constants, num_public, num_private, num_constraints);
        });
    }

    #[test]
    fn test_to_bits_le_constant() {
        let expected = Uniform::rand(&mut TestRng::default());
        let candidate = Scalar::<Circuit>::new(Mode::Constant, expected);
        check_to_bits_le("Constant", &expected.to_bits_le(), &candidate, 251, 0, 0, 0);
    }

    #[test]
    fn test_to_bits_be_constant() {
        let expected = Uniform::rand(&mut TestRng::default());
        let candidate = Scalar::<Circuit>::new(Mode::Constant, expected);
        check_to_bits_be("Constant", &expected.to_bits_be(), candidate, 251, 0, 0, 0);
    }

    #[test]
    fn test_to_bits_le_public() {
        let expected = Uniform::rand(&mut TestRng::default());
        let candidate = Scalar::<Circuit>::new(Mode::Public, expected);
        check_to_bits_le("Public", &expected.to_bits_le(), &candidate, 0, 0, 501, 503);
    }

    #[test]
    fn test_to_bits_be_public() {
        let expected = Uniform::rand(&mut TestRng::default());
        let candidate = Scalar::<Circuit>::new(Mode::Public, expected);
        check_to_bits_be("Public", &expected.to_bits_be(), candidate, 0, 0, 501, 503);
    }

    #[test]
    fn test_to_bits_le_private() {
        let expected = Uniform::rand(&mut TestRng::default());
        let candidate = Scalar::<Circuit>::new(Mode::Private, expected);
        check_to_bits_le("Private", &expected.to_bits_le(), &candidate, 0, 0, 501, 503);
    }

    #[test]
    fn test_to_bits_be_private() {
        let expected = Uniform::rand(&mut TestRng::default());
        let candidate = Scalar::<Circuit>::new(Mode::Private, expected);
        check_to_bits_be("Private", &expected.to_bits_be(), candidate, 0, 0, 501, 503);
    }

    #[test]
    fn test_one() {
        /// Checks that the field element, when converted to little-endian bits, is well-formed.
        fn check_bits_le(candidate: Scalar<Circuit>) {
            for (i, bit) in candidate.to_bits_le().iter().enumerate() {
                match i == 0 {
                    true => assert!(bit.eject_value()),
                    false => assert!(!bit.eject_value()),
                }
            }
        }

        /// Checks that the field element, when converted to big-endian bits, is well-formed.
        fn check_bits_be(candidate: Scalar<Circuit>) {
            for (i, bit) in candidate.to_bits_be().iter().rev().enumerate() {
                match i == 0 {
                    true => assert!(bit.eject_value()),
                    false => assert!(!bit.eject_value()),
                }
            }
        }

        let one = console::Scalar::<<Circuit as Environment>::Network>::one();

        // Constant
        check_bits_le(Scalar::<Circuit>::new(Mode::Constant, one));
        check_bits_be(Scalar::<Circuit>::new(Mode::Constant, one));
        // Public
        check_bits_le(Scalar::<Circuit>::new(Mode::Public, one));
        check_bits_be(Scalar::<Circuit>::new(Mode::Public, one));
        // Private
        check_bits_le(Scalar::<Circuit>::new(Mode::Private, one));
        check_bits_be(Scalar::<Circuit>::new(Mode::Private, one));
    }
}
