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

impl<E: Environment> From<Group<E>> for Address<E> {
    /// Initializes an address from an affine group element.
    fn from(value: Group<E>) -> Self {
        Self::from_group(value)
    }
}

impl<E: Environment> From<&Group<E>> for Address<E> {
    /// Initializes an address from an affine group element.
    fn from(value: &Group<E>) -> Self {
        Self::from_group(value.clone())
    }
}

impl<E: Environment> FromGroup for Address<E> {
    type Group = Group<E>;
    type Scalar = Scalar<E>;

    /// Initializes an address from an affine group element.
    fn from_group(group: Self::Group) -> Self {
        Self(group)
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    fn check_from_group(
        name: &str,
        expected: console::Address<<Circuit as Environment>::Network>,
        candidate: &Group<Circuit>,
    ) {
        Circuit::scope(name, || {
            // Perform the operation.
            let candidate = Address::from_group(candidate.clone());
            assert_eq!(expected, candidate.eject_value());
            assert_scope!(0, 0, 0, 0);
        });
    }

    #[test]
    fn test_from_group_constant() {
        let expected = console::Address::rand(&mut TestRng::default());
        let candidate = Group::<Circuit>::new(Mode::Constant, *expected);
        check_from_group("Constant", expected, &candidate);
    }

    #[test]
    fn test_from_group_public() {
        let expected = console::Address::rand(&mut TestRng::default());
        let candidate = Group::<Circuit>::new(Mode::Public, *expected);
        check_from_group("Public", expected, &candidate);
    }

    #[test]
    fn test_from_group_private() {
        let expected = console::Address::rand(&mut TestRng::default());
        let candidate = Group::<Circuit>::new(Mode::Private, *expected);
        check_from_group("Private", expected, &candidate);
    }
}
