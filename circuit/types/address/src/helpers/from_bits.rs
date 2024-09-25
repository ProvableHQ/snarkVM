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

impl<E: Environment> FromBits for Address<E> {
    type Boolean = Boolean<E>;

    /// Initializes an address from a list of little-endian bits *without* trailing zeros.
    fn from_bits_le(bits_le: &[Self::Boolean]) -> Self {
        Self(Group::from_bits_le(bits_le))
    }

    /// Initializes an address from a list of big-endian bits *without* leading zeros.
    fn from_bits_be(bits_be: &[Self::Boolean]) -> Self {
        Self(Group::from_bits_be(bits_be))
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 100;

    fn check_from_bits_le(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected: console::Group<<Circuit as Environment>::Network> = Uniform::rand(&mut rng);
            let candidate = Group::<Circuit>::new(mode, expected).to_bits_le();

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = Address::<Circuit>::from_bits_le(&candidate);
                assert_eq!(expected, *candidate.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
    }

    fn check_from_bits_be(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected: console::Group<<Circuit as Environment>::Network> = Uniform::rand(&mut rng);
            let candidate = Group::<Circuit>::new(mode, expected).to_bits_be();

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = Address::<Circuit>::from_bits_be(&candidate);
                assert_eq!(expected, *candidate.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
    }

    #[test]
    fn test_from_bits_le_constant() {
        check_from_bits_le(Mode::Constant, 9, 0, 0, 0);
    }

    #[test]
    fn test_from_bits_le_public() {
        check_from_bits_le(Mode::Public, 4, 0, 265, 266);
    }

    #[test]
    fn test_from_bits_le_private() {
        check_from_bits_le(Mode::Private, 4, 0, 265, 266);
    }

    #[test]
    fn test_from_bits_be_constant() {
        check_from_bits_be(Mode::Constant, 9, 0, 0, 0);
    }

    #[test]
    fn test_from_bits_be_public() {
        check_from_bits_be(Mode::Public, 4, 0, 265, 266);
    }

    #[test]
    fn test_from_bits_be_private() {
        check_from_bits_be(Mode::Private, 4, 0, 265, 266);
    }
}
