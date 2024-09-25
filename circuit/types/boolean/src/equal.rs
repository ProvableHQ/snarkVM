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

impl<E: Environment> Equal<Self> for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns `true` if `self` and `other` are equal.
    fn is_equal(&self, other: &Self) -> Self::Output {
        !self.is_not_equal(other)
    }

    /// Returns `true` if `self` and `other` are *not* equal.
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        self ^ other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    fn check_is_equal(
        name: &str,
        expected: bool,
        a: Boolean<Circuit>,
        b: Boolean<Circuit>,
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
    }

    #[test]
    fn test_constant_equals_constant() {
        // false == false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, false);
        let b = Boolean::<Circuit>::new(Mode::Constant, false);
        check_is_equal("false == false", expected, a, b, 0, 0, 0, 0);

        // false == true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Constant, false);
        let b = Boolean::<Circuit>::new(Mode::Constant, true);
        check_is_equal("false == true", expected, a, b, 0, 0, 0, 0);

        // true == false
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Constant, true);
        let b = Boolean::<Circuit>::new(Mode::Constant, false);
        check_is_equal("true == false", expected, a, b, 0, 0, 0, 0);

        // true == true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, true);
        let b = Boolean::<Circuit>::new(Mode::Constant, true);
        check_is_equal("true == true", expected, a, b, 0, 0, 0, 0);
    }

    #[test]
    fn test_constant_equals_public() {
        // false == false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, false);
        let b = Boolean::<Circuit>::new(Mode::Public, false);
        check_is_equal("false == false", expected, a, b, 0, 0, 0, 0);

        // false == true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Constant, false);
        let b = Boolean::<Circuit>::new(Mode::Public, true);
        check_is_equal("false == true", expected, a, b, 0, 0, 0, 0);

        // true == false
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Constant, true);
        let b = Boolean::<Circuit>::new(Mode::Public, false);
        check_is_equal("true == false", expected, a, b, 0, 0, 0, 0);

        // true == true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, true);
        let b = Boolean::<Circuit>::new(Mode::Public, true);
        check_is_equal("true == true", expected, a, b, 0, 0, 0, 0);
    }

    #[test]
    fn test_constant_equal_private() {
        // false == false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, false);
        let b = Boolean::<Circuit>::new(Mode::Private, false);
        check_is_equal("false == false", expected, a, b, 0, 0, 0, 0);

        // false == true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Constant, false);
        let b = Boolean::<Circuit>::new(Mode::Private, true);
        check_is_equal("false == true", expected, a, b, 0, 0, 0, 0);

        // true == false
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Constant, true);
        let b = Boolean::<Circuit>::new(Mode::Private, false);
        check_is_equal("true == false", expected, a, b, 0, 0, 0, 0);

        // true == true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, true);
        let b = Boolean::<Circuit>::new(Mode::Private, true);
        check_is_equal("true == true", expected, a, b, 0, 0, 0, 0);
    }

    #[test]
    fn test_public_equals_constant() {
        // false == false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, false);
        let b = Boolean::<Circuit>::new(Mode::Constant, false);
        check_is_equal("false == false", expected, a, b, 0, 0, 0, 0);

        // false == true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Public, false);
        let b = Boolean::<Circuit>::new(Mode::Constant, true);
        check_is_equal("false == true", expected, a, b, 0, 0, 0, 0);

        // true == false
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Public, true);
        let b = Boolean::<Circuit>::new(Mode::Constant, false);
        check_is_equal("true == false", expected, a, b, 0, 0, 0, 0);

        // true == true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, true);
        let b = Boolean::<Circuit>::new(Mode::Constant, true);
        check_is_equal("true == true", expected, a, b, 0, 0, 0, 0);
    }

    #[test]
    fn test_public_equals_public() {
        // false == false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, false);
        let b = Boolean::<Circuit>::new(Mode::Public, false);
        check_is_equal("false == false", expected, a, b, 0, 0, 1, 1);

        // false == true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Public, false);
        let b = Boolean::<Circuit>::new(Mode::Public, true);
        check_is_equal("false == true", expected, a, b, 0, 0, 1, 1);

        // true == false
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Public, true);
        let b = Boolean::<Circuit>::new(Mode::Public, false);
        check_is_equal("true == false", expected, a, b, 0, 0, 1, 1);

        // true == true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, true);
        let b = Boolean::<Circuit>::new(Mode::Public, true);
        check_is_equal("true == true", expected, a, b, 0, 0, 1, 1);
    }

    #[test]
    fn test_public_equals_private() {
        // false == false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, false);
        let b = Boolean::<Circuit>::new(Mode::Private, false);
        check_is_equal("false == false", expected, a, b, 0, 0, 1, 1);

        // false == true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Public, false);
        let b = Boolean::<Circuit>::new(Mode::Private, true);
        check_is_equal("false == true", expected, a, b, 0, 0, 1, 1);

        // true == false
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Public, true);
        let b = Boolean::<Circuit>::new(Mode::Private, false);
        check_is_equal("true == false", expected, a, b, 0, 0, 1, 1);

        // true == true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, true);
        let b = Boolean::<Circuit>::new(Mode::Private, true);
        check_is_equal("true == true", expected, a, b, 0, 0, 1, 1);
    }

    #[test]
    fn test_private_equals_constant() {
        // false == false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, false);
        let b = Boolean::<Circuit>::new(Mode::Constant, false);
        check_is_equal("false == false", expected, a, b, 0, 0, 0, 0);

        // false == true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Private, false);
        let b = Boolean::<Circuit>::new(Mode::Constant, true);
        check_is_equal("false == true", expected, a, b, 0, 0, 0, 0);

        // true == false
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Private, true);
        let b = Boolean::<Circuit>::new(Mode::Constant, false);
        check_is_equal("true == false", expected, a, b, 0, 0, 0, 0);

        // true == true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, true);
        let b = Boolean::<Circuit>::new(Mode::Constant, true);
        check_is_equal("true == true", expected, a, b, 0, 0, 0, 0);
    }

    #[test]
    fn test_private_equal_public() {
        // false == false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, false);
        let b = Boolean::<Circuit>::new(Mode::Public, false);
        check_is_equal("false == false", expected, a, b, 0, 0, 1, 1);

        // false == true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Private, false);
        let b = Boolean::<Circuit>::new(Mode::Public, true);
        check_is_equal("false == true", expected, a, b, 0, 0, 1, 1);

        // true == false
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Private, true);
        let b = Boolean::<Circuit>::new(Mode::Public, false);
        check_is_equal("true == false", expected, a, b, 0, 0, 1, 1);

        // true == true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, true);
        let b = Boolean::<Circuit>::new(Mode::Public, true);
        check_is_equal("true == true", expected, a, b, 0, 0, 1, 1);
    }

    #[test]
    fn test_private_equals_private() {
        // false == false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, false);
        let b = Boolean::<Circuit>::new(Mode::Private, false);
        check_is_equal("false == false", expected, a, b, 0, 0, 1, 1);

        // false == true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Private, false);
        let b = Boolean::<Circuit>::new(Mode::Private, true);
        check_is_equal("false == true", expected, a, b, 0, 0, 1, 1);

        // true == false
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Private, true);
        let b = Boolean::<Circuit>::new(Mode::Private, false);
        check_is_equal("true == false", expected, a, b, 0, 0, 1, 1);

        // true == true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, true);
        let b = Boolean::<Circuit>::new(Mode::Private, true);
        check_is_equal("true == true", expected, a, b, 0, 0, 1, 1);
    }
}
