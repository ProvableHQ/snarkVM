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

impl<E: Environment> ToField for Scalar<E> {
    type Field = Field<E>;

    /// Casts a scalar field element into a base field element.
    fn to_field(&self) -> Self::Field {
        self.field.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    fn check_to_field(name: &str, expected: &[bool], candidate: &Scalar<Circuit>) {
        Circuit::scope(name, || {
            // Perform the operation.
            let candidate = candidate.to_field();
            assert_scope!(0, 0, 0, 0);

            // Extract the bits from the base field representation.
            let candidate_bits_le = candidate.eject_value().to_bits_le();
            assert_eq!(<Circuit as Environment>::BaseField::size_in_bits(), candidate_bits_le.len());

            // Ensure all scalar bits match with the expected result.
            let num_scalar_bits = console::Scalar::<<Circuit as Environment>::Network>::size_in_bits();
            for (expected_bit, candidate_bit) in expected.iter().zip_eq(&candidate_bits_le[0..num_scalar_bits]) {
                assert_eq!(expected_bit, candidate_bit);
            }

            // Ensure all remaining bits are 0.
            for candidate_bit in &candidate_bits_le[num_scalar_bits..] {
                assert!(!candidate_bit);
            }
        });
        Circuit::reset();
    }

    #[test]
    fn test_to_field_constant() {
        let expected = Uniform::rand(&mut TestRng::default());
        let candidate = Scalar::<Circuit>::new(Mode::Constant, expected);
        check_to_field("Constant", &expected.to_bits_le(), &candidate);
    }

    #[test]
    fn test_to_field_public() {
        let expected = Uniform::rand(&mut TestRng::default());
        let candidate = Scalar::<Circuit>::new(Mode::Public, expected);
        check_to_field("Public", &expected.to_bits_le(), &candidate);
    }

    #[test]
    fn test_to_field_private() {
        let expected = Uniform::rand(&mut TestRng::default());
        let candidate = Scalar::<Circuit>::new(Mode::Private, expected);
        check_to_field("Private", &expected.to_bits_le(), &candidate);
    }

    #[test]
    fn test_one() {
        /// Checks that the `1` scalar field element, when converted to a base field, is well-formed.
        fn check_to_field_one(candidate: Scalar<Circuit>) {
            for (i, bit) in candidate.to_field().to_bits_le().iter().enumerate() {
                match i == 0 {
                    true => assert!(bit.eject_value()),
                    false => assert!(!bit.eject_value()),
                }
            }
        }

        let one = console::Scalar::<<Circuit as Environment>::Network>::one();

        // Constant
        check_to_field_one(Scalar::<Circuit>::new(Mode::Constant, one));
        // Public
        check_to_field_one(Scalar::<Circuit>::new(Mode::Public, one));
        // Private
        check_to_field_one(Scalar::<Circuit>::new(Mode::Private, one));
    }
}
