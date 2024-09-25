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

impl<E: Environment> Equal<Self> for Scalar<E> {
    type Output = Boolean<E>;

    ///
    /// Returns `true` if `self` and `other` are equal.
    ///
    fn is_equal(&self, other: &Self) -> Self::Output {
        self.field.is_equal(&other.field)
    }

    ///
    /// Returns `true` if `self` and `other` are *not* equal.
    ///
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        !self.is_equal(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    fn check_is_equal(
        name: &str,
        expected: bool,
        a: Scalar<Circuit>,
        b: Scalar<Circuit>,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        Circuit::scope(name, || {
            let candidate = a.is_equal(&b);
            assert_eq!(expected, candidate.eject_value(), "({} == {})", a.eject_value(), b.eject_value());
            assert_scope!(num_constants, num_public, num_private, num_constraints);
        });
        Circuit::reset();
    }

    #[test]
    fn test_constant_equals_constant() {
        let mut rng = TestRng::default();

        let first = Uniform::rand(&mut rng);
        let second = Uniform::rand(&mut rng);

        // a == a
        let expected = true;
        let a = Scalar::<Circuit>::new(Mode::Constant, first);
        let b = Scalar::<Circuit>::new(Mode::Constant, first);
        check_is_equal("a == a", expected, a, b, 1, 0, 0, 0);

        // a != b
        let expected = false;
        let a = Scalar::<Circuit>::new(Mode::Constant, first);
        let b = Scalar::<Circuit>::new(Mode::Constant, second);
        check_is_equal("a != b", expected, a, b, 1, 0, 0, 0);
    }

    #[test]
    fn test_constant_equals_public() {
        let mut rng = TestRng::default();

        let first = Uniform::rand(&mut rng);
        let second = Uniform::rand(&mut rng);

        // a == a
        let expected = true;
        let a = Scalar::<Circuit>::new(Mode::Constant, first);
        let b = Scalar::<Circuit>::new(Mode::Public, first);
        check_is_equal("a == a", expected, a, b, 0, 0, 2, 2);

        // a != b
        let expected = false;
        let a = Scalar::<Circuit>::new(Mode::Constant, first);
        let b = Scalar::<Circuit>::new(Mode::Public, second);
        check_is_equal("a != b", expected, a, b, 0, 0, 2, 2);
    }

    #[test]
    fn test_public_equals_constant() {
        let mut rng = TestRng::default();

        let first = Uniform::rand(&mut rng);
        let second = Uniform::rand(&mut rng);

        // a == a
        let expected = true;
        let a = Scalar::<Circuit>::new(Mode::Public, first);
        let b = Scalar::<Circuit>::new(Mode::Constant, first);
        check_is_equal("a == a", expected, a, b, 0, 0, 2, 2);

        // a != b
        let expected = false;
        let a = Scalar::<Circuit>::new(Mode::Public, first);
        let b = Scalar::<Circuit>::new(Mode::Constant, second);
        check_is_equal("a != b", expected, a, b, 0, 0, 2, 2);
    }

    #[test]
    fn test_public_equals_public() {
        let mut rng = TestRng::default();

        let first = Uniform::rand(&mut rng);
        let second = Uniform::rand(&mut rng);

        // a == a
        let expected = true;
        let a = Scalar::<Circuit>::new(Mode::Public, first);
        let b = Scalar::<Circuit>::new(Mode::Public, first);
        check_is_equal("a == a", expected, a, b, 0, 0, 2, 2);

        // a != b
        let expected = false;
        let a = Scalar::<Circuit>::new(Mode::Public, first);
        let b = Scalar::<Circuit>::new(Mode::Public, second);
        check_is_equal("a != b", expected, a, b, 0, 0, 2, 2);
    }

    #[test]
    fn test_public_equals_private() {
        let mut rng = TestRng::default();

        let first = Uniform::rand(&mut rng);
        let second = Uniform::rand(&mut rng);

        // a == a
        let expected = true;
        let a = Scalar::<Circuit>::new(Mode::Public, first);
        let b = Scalar::<Circuit>::new(Mode::Private, first);
        check_is_equal("a == a", expected, a, b, 0, 0, 2, 2);

        // a != b
        let expected = false;
        let a = Scalar::<Circuit>::new(Mode::Public, first);
        let b = Scalar::<Circuit>::new(Mode::Private, second);
        check_is_equal("a != b", expected, a, b, 0, 0, 2, 2);
    }

    #[test]
    fn test_private_equals_private() {
        let mut rng = TestRng::default();

        let first = Uniform::rand(&mut rng);
        let second = Uniform::rand(&mut rng);

        // a == a
        let expected = true;
        let a = Scalar::<Circuit>::new(Mode::Private, first);
        let b = Scalar::<Circuit>::new(Mode::Private, first);
        check_is_equal("a == a", expected, a, b, 0, 0, 2, 2);

        // a != b
        let expected = false;
        let a = Scalar::<Circuit>::new(Mode::Private, first);
        let b = Scalar::<Circuit>::new(Mode::Private, second);
        check_is_equal("a != b", expected, a, b, 0, 0, 2, 2);
    }
}
