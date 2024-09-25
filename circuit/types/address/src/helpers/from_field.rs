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

impl<E: Environment> From<Field<E>> for Address<E> {
    /// Initializes an address from the **x-coordinate** of an affine group element.
    fn from(value: Field<E>) -> Self {
        Self::from_field(value)
    }
}

impl<E: Environment> From<&Field<E>> for Address<E> {
    /// Initializes an address from the **x-coordinate** of an affine group element.
    fn from(value: &Field<E>) -> Self {
        Self::from_field(value.clone())
    }
}

impl<E: Environment> FromField for Address<E> {
    type Field = Field<E>;

    /// Initializes an address from the **x-coordinate** of an affine group element.
    fn from_field(x_coordinate: Field<E>) -> Self {
        Self::from_group(Group::from_x_coordinate(x_coordinate))
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 100;

    fn check_from_field(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected: console::Group<<Circuit as Environment>::Network> = Uniform::rand(&mut rng);
            let candidate = Group::<Circuit>::new(mode, expected).to_x_coordinate();

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = Address::from_field(candidate);
                assert_eq!(expected.to_x_coordinate(), candidate.eject_value().to_x_coordinate());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
    }

    #[test]
    fn test_from_field_constant() {
        check_from_field(Mode::Constant, 9, 0, 0, 0);
    }

    #[test]
    fn test_from_field_public() {
        check_from_field(Mode::Public, 4, 0, 13, 13);
    }

    #[test]
    fn test_from_field_private() {
        check_from_field(Mode::Private, 4, 0, 13, 13);
    }
}
