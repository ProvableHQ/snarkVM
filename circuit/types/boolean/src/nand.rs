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

impl<E: Environment> Nand<Self> for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns `NOT (a AND b)`.
    fn nand(&self, other: &Self) -> Self::Output {
        // Constant `self`
        if self.is_constant() {
            match self.eject_value() {
                true => !other.clone(),
                false => !self.clone(),
            }
        }
        // Constant `other`
        else if other.is_constant() {
            match other.eject_value() {
                true => !self.clone(),
                false => !other.clone(),
            }
        }
        // Variable NAND Variable
        else {
            // Declare a new variable with the expected output as witness.
            // Note: The constraint below will ensure `output` is either 0 or 1,
            // assuming `self` and `other` are well-formed (they are either 0 or 1).
            let output = Boolean(
                E::new_variable(Mode::Private, match !(self.eject_value() & other.eject_value()) {
                    true => E::BaseField::one(),
                    false => E::BaseField::zero(),
                })
                .into(),
            );

            // Ensure `self` * `other` = (1 - `output`)
            // `output` is `1` iff `self` or `other` is `0`, otherwise `output` is `0`.
            E::enforce(|| (self, other, E::one() - &output.0));

            output
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    fn check_nand(
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
            let candidate = a.nand(&b);
            assert_eq!(expected, candidate.eject_value(), "({} NAND {})", a.eject_value(), b.eject_value());
            assert_scope!(num_constants, num_public, num_private, num_constraints);
        });
    }

    #[test]
    fn test_constant_nand_constant() {
        // false NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, false);
        let b = Boolean::<Circuit>::new(Mode::Constant, false);
        check_nand("false NAND false", expected, a, b, 0, 0, 0, 0);

        // false NAND true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, false);
        let b = Boolean::<Circuit>::new(Mode::Constant, true);
        check_nand("false NAND true", expected, a, b, 0, 0, 0, 0);

        // true NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, true);
        let b = Boolean::<Circuit>::new(Mode::Constant, false);
        check_nand("true NAND false", expected, a, b, 0, 0, 0, 0);

        // true NAND true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Constant, true);
        let b = Boolean::<Circuit>::new(Mode::Constant, true);
        check_nand("true NAND true", expected, a, b, 0, 0, 0, 0);
    }

    #[test]
    fn test_constant_nand_public() {
        // false NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, false);
        let b = Boolean::<Circuit>::new(Mode::Public, false);
        check_nand("false NAND false", expected, a, b, 0, 0, 0, 0);

        // false NAND true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, false);
        let b = Boolean::<Circuit>::new(Mode::Public, true);
        check_nand("false NAND true", expected, a, b, 0, 0, 0, 0);

        // true NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, true);
        let b = Boolean::<Circuit>::new(Mode::Public, false);
        check_nand("true NAND false", expected, a, b, 0, 0, 0, 0);

        // true NAND true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Constant, true);
        let b = Boolean::<Circuit>::new(Mode::Public, true);
        check_nand("true NAND true", expected, a, b, 0, 0, 0, 0);
    }

    #[test]
    fn test_constant_nand_private() {
        // false NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, false);
        let b = Boolean::<Circuit>::new(Mode::Private, false);
        check_nand("false NAND false", expected, a, b, 0, 0, 0, 0);

        // false NAND true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, false);
        let b = Boolean::<Circuit>::new(Mode::Private, true);
        check_nand("false NAND true", expected, a, b, 0, 0, 0, 0);

        // true NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Constant, true);
        let b = Boolean::<Circuit>::new(Mode::Private, false);
        check_nand("true NAND false", expected, a, b, 0, 0, 0, 0);

        // true NAND true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Constant, true);
        let b = Boolean::<Circuit>::new(Mode::Private, true);
        check_nand("true NAND true", expected, a, b, 0, 0, 0, 0);
    }

    #[test]
    fn test_public_nand_constant() {
        // false NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, false);
        let b = Boolean::<Circuit>::new(Mode::Constant, false);
        check_nand("false NAND false", expected, a, b, 0, 0, 0, 0);

        // false NAND true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, false);
        let b = Boolean::<Circuit>::new(Mode::Constant, true);
        check_nand("false NAND true", expected, a, b, 0, 0, 0, 0);

        // true NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, true);
        let b = Boolean::<Circuit>::new(Mode::Constant, false);
        check_nand("true NAND false", expected, a, b, 0, 0, 0, 0);

        // true NAND true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Public, true);
        let b = Boolean::<Circuit>::new(Mode::Constant, true);
        check_nand("true NAND true", expected, a, b, 0, 0, 0, 0);
    }

    #[test]
    fn test_public_nand_public() {
        // false NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, false);
        let b = Boolean::<Circuit>::new(Mode::Public, false);
        check_nand("false NAND false", expected, a, b, 0, 0, 1, 1);

        // false NAND true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, false);
        let b = Boolean::<Circuit>::new(Mode::Public, true);
        check_nand("false NAND true", expected, a, b, 0, 0, 1, 1);

        // true NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, true);
        let b = Boolean::<Circuit>::new(Mode::Public, false);
        check_nand("true NAND false", expected, a, b, 0, 0, 1, 1);

        // true NAND true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Public, true);
        let b = Boolean::<Circuit>::new(Mode::Public, true);
        check_nand("true NAND true", expected, a, b, 0, 0, 1, 1);
    }

    #[test]
    fn test_public_nand_private() {
        // false NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, false);
        let b = Boolean::<Circuit>::new(Mode::Private, false);
        check_nand("false NAND false", expected, a, b, 0, 0, 1, 1);

        // false NAND true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, false);
        let b = Boolean::<Circuit>::new(Mode::Private, true);
        check_nand("false NAND true", expected, a, b, 0, 0, 1, 1);

        // true NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Public, true);
        let b = Boolean::<Circuit>::new(Mode::Private, false);
        check_nand("true NAND false", expected, a, b, 0, 0, 1, 1);

        // true NAND true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Public, true);
        let b = Boolean::<Circuit>::new(Mode::Private, true);
        check_nand("true NAND true", expected, a, b, 0, 0, 1, 1);
    }

    #[test]
    fn test_private_nand_constant() {
        // false NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, false);
        let b = Boolean::<Circuit>::new(Mode::Constant, false);
        check_nand("false NAND false", expected, a, b, 0, 0, 0, 0);

        // false NAND true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, false);
        let b = Boolean::<Circuit>::new(Mode::Constant, true);
        check_nand("false NAND true", expected, a, b, 0, 0, 0, 0);

        // true NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, true);
        let b = Boolean::<Circuit>::new(Mode::Constant, false);
        check_nand("true NAND false", expected, a, b, 0, 0, 0, 0);

        // true NAND true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Private, true);
        let b = Boolean::<Circuit>::new(Mode::Constant, true);
        check_nand("true NAND true", expected, a, b, 0, 0, 0, 0);
    }

    #[test]
    fn test_private_nand_public() {
        // false NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, false);
        let b = Boolean::<Circuit>::new(Mode::Public, false);
        check_nand("false NAND false", expected, a, b, 0, 0, 1, 1);

        // false NAND true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, false);
        let b = Boolean::<Circuit>::new(Mode::Public, true);
        check_nand("false NAND true", expected, a, b, 0, 0, 1, 1);

        // true NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, true);
        let b = Boolean::<Circuit>::new(Mode::Public, false);
        check_nand("true NAND false", expected, a, b, 0, 0, 1, 1);

        // true NAND true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Private, true);
        let b = Boolean::<Circuit>::new(Mode::Public, true);
        check_nand("true NAND true", expected, a, b, 0, 0, 1, 1);
    }

    #[test]
    fn test_private_nand_private() {
        // false NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, false);
        let b = Boolean::<Circuit>::new(Mode::Private, false);
        check_nand("false NAND false", expected, a, b, 0, 0, 1, 1);

        // false NAND true
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, false);
        let b = Boolean::<Circuit>::new(Mode::Private, true);
        check_nand("false NAND true", expected, a, b, 0, 0, 1, 1);

        // true NAND false
        let expected = true;
        let a = Boolean::<Circuit>::new(Mode::Private, true);
        let b = Boolean::<Circuit>::new(Mode::Private, false);
        check_nand("true NAND false", expected, a, b, 0, 0, 1, 1);

        // true NAND true
        let expected = false;
        let a = Boolean::<Circuit>::new(Mode::Private, true);
        let b = Boolean::<Circuit>::new(Mode::Private, true);
        check_nand("true NAND true", expected, a, b, 0, 0, 1, 1);
    }
}
