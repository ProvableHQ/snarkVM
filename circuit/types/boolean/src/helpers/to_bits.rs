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

impl<E: Environment> ToBits for Boolean<E> {
    type Boolean = Boolean<E>;

    /// Outputs `self` as a single-element vector.
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        (&self).write_bits_le(vec);
    }

    /// Outputs `self` as a single-element vector.
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        (&self).write_bits_be(vec);
    }
}

impl<E: Environment> ToBits for &Boolean<E> {
    type Boolean = Boolean<E>;

    /// Outputs `self` as a single-element vector.
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        vec.push((*self).clone());
    }

    /// Outputs `self` as a single-element vector.
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        vec.push((*self).clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    fn check_to_bits_le(name: &str, expected: &[bool], candidate: &Boolean<Circuit>) {
        Circuit::scope(name, || {
            let candidate = candidate.to_bits_le();
            assert_eq!(1, candidate.len());
            for (expected_bit, candidate_bit) in expected.iter().zip_eq(candidate.iter()) {
                assert_eq!(*expected_bit, candidate_bit.eject_value());
            }
            assert_scope!(0, 0, 0, 0);
        });
    }

    fn check_to_bits_be(name: &str, expected: &[bool], candidate: Boolean<Circuit>) {
        Circuit::scope(name, || {
            let candidate = candidate.to_bits_be();
            assert_eq!(1, candidate.len());
            for (expected_bit, candidate_bit) in expected.iter().zip_eq(candidate.iter()) {
                assert_eq!(*expected_bit, candidate_bit.eject_value());
            }
            assert_scope!(0, 0, 0, 0);
        });
    }

    #[test]
    fn test_to_bits_constant() {
        let candidate = Boolean::<Circuit>::new(Mode::Constant, true);
        check_to_bits_le("Constant", &[true], &candidate);
        check_to_bits_be("Constant", &[true], candidate);

        let candidate = Boolean::<Circuit>::new(Mode::Constant, false);
        check_to_bits_le("Constant", &[false], &candidate);
        check_to_bits_be("Constant", &[false], candidate);
    }

    #[test]
    fn test_to_bits_public() {
        let candidate = Boolean::<Circuit>::new(Mode::Public, true);
        check_to_bits_le("Public", &[true], &candidate);
        check_to_bits_be("Public", &[true], candidate);

        let candidate = Boolean::<Circuit>::new(Mode::Public, false);
        check_to_bits_le("Public", &[false], &candidate);
        check_to_bits_be("Public", &[false], candidate);
    }

    #[test]
    fn test_to_bits_private() {
        let candidate = Boolean::<Circuit>::new(Mode::Private, true);
        check_to_bits_le("Private", &[true], &candidate);
        check_to_bits_be("Private", &[true], candidate);

        let candidate = Boolean::<Circuit>::new(Mode::Private, false);
        check_to_bits_le("Private", &[false], &candidate);
        check_to_bits_be("Private", &[false], candidate);
    }
}
