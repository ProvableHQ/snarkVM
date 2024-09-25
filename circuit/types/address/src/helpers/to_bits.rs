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

impl<E: Environment> ToBits for Address<E> {
    type Boolean = Boolean<E>;

    /// Outputs the little-endian bit representation of `self.x` *without* trailing zeros.
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        (&self).write_bits_le(vec);
    }

    /// Outputs the big-endian bit representation of `self.x` *without* leading zeros.
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        (&self).write_bits_be(vec);
    }
}

impl<E: Environment> ToBits for &Address<E> {
    type Boolean = Boolean<E>;

    /// Outputs the little-endian bit representation of `self.x` *without* trailing zeros.
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        self.0.write_bits_le(vec);
    }

    /// Outputs the big-endian bit representation of `self.x` *without* leading zeros.
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        self.0.write_bits_be(vec);
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 100;

    fn check_to_bits_le(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let expected_number_of_bits = <<Circuit as Environment>::BaseField as PrimeField>::size_in_bits();

        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected = Uniform::rand(&mut rng);
            let candidate = Address::<Circuit>::from_group(Group::new(mode, expected));

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = candidate.to_bits_le();
                assert_eq!(expected_number_of_bits, candidate.len());
                for (expected_bit, candidate_bit) in
                    expected.to_x_coordinate().to_bits_le().iter().zip_eq(candidate.iter())
                {
                    assert_eq!(*expected_bit, candidate_bit.eject_value());
                }
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
        }
    }

    fn check_to_bits_be(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let expected_number_of_bits = <<Circuit as Environment>::BaseField as PrimeField>::size_in_bits();

        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected = Uniform::rand(&mut rng);
            let candidate = Address::<Circuit>::from_group(Group::new(mode, expected));

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = candidate.to_bits_be();
                assert_eq!(expected_number_of_bits, candidate.len());
                for (expected_bit, candidate_bit) in
                    expected.to_x_coordinate().to_bits_be().iter().zip_eq(candidate.iter())
                {
                    assert_eq!(*expected_bit, candidate_bit.eject_value());
                }
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
        }
    }

    #[test]
    fn test_to_bits_le_constant() {
        check_to_bits_le(Mode::Constant, 253, 0, 0, 0);
    }

    #[test]
    fn test_to_bits_le_public() {
        check_to_bits_le(Mode::Public, 0, 0, 505, 507);
    }

    #[test]
    fn test_to_bits_le_private() {
        check_to_bits_le(Mode::Private, 0, 0, 505, 507);
    }

    #[test]
    fn test_to_bits_be_constant() {
        check_to_bits_be(Mode::Constant, 253, 0, 0, 0);
    }

    #[test]
    fn test_to_bits_be_public() {
        check_to_bits_be(Mode::Public, 0, 0, 505, 507);
    }

    #[test]
    fn test_to_bits_be_private() {
        check_to_bits_be(Mode::Private, 0, 0, 505, 507);
    }
}
