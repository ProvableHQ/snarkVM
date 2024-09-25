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

impl<E: Environment> Adder for Boolean<E> {
    type Carry = Boolean<E>;
    type Sum = Boolean<E>;

    /// Returns the sum of `self` and `other` as a sum bit and carry bit.
    fn adder(&self, other: &Self, carry: &Self) -> (Self::Sum, Self::Carry) {
        // Compute the sum bit.
        let c0 = self ^ other;
        let sum = &c0 ^ carry;

        // Compute the carry bit.
        let c1 = self & other;
        let c2 = carry & c0;
        let carry = c1 | c2;

        (sum, carry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    fn check_adder(
        name: &str,
        expected_sum: bool,
        expected_carry: bool,
        a: Boolean<Circuit>,
        b: Boolean<Circuit>,
        c: Boolean<Circuit>,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        Circuit::scope(name, || {
            let case = format!("({} ADD {} WITH {})", a.eject_value(), b.eject_value(), c.eject_value());
            let (candidate_sum, candidate_carry) = a.adder(&b, &c);
            assert_eq!(expected_sum, candidate_sum.eject_value(), "SUM {case}");
            assert_eq!(expected_carry, candidate_carry.eject_value(), "CARRY {case}");
            assert_scope!(case, num_constants, num_public, num_private, num_constraints);
        });
    }

    #[rustfmt::skip]
    fn check_false_add_false_with_false(
        mode_a: Mode,
        mode_b: Mode,
        mode_c: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64
    ) {
        // false ADD false WITH false => (false, false)
        let expected_sum = false;
        let expected_carry = false;
        let a = Boolean::<Circuit>::new(mode_a, false);
        let b = Boolean::<Circuit>::new(mode_b, false);
        let c = Boolean::<Circuit>::new(mode_c, false);
        check_adder("false ADD false WITH false", expected_sum, expected_carry, a, b, c, num_constants, num_public, num_private, num_constraints);
    }

    #[rustfmt::skip]
    fn check_false_add_false_with_true(
        mode_a: Mode,
        mode_b: Mode,
        mode_c: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64
    ) {
        // false ADD false WITH true => (true, false)
        let expected_sum = true;
        let expected_carry = false;
        let a = Boolean::<Circuit>::new(mode_a, false);
        let b = Boolean::<Circuit>::new(mode_b, false);
        let c = Boolean::<Circuit>::new(mode_c, true);
        check_adder("false ADD false WITH true", expected_sum, expected_carry, a, b, c, num_constants, num_public, num_private, num_constraints);
    }

    #[rustfmt::skip]
    fn check_false_add_true_with_false(
        mode_a: Mode,
        mode_b: Mode,
        mode_c: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64
    ) {
        // false ADD true WITH false => (true, false)
        let expected_sum = true;
        let expected_carry = false;
        let a = Boolean::<Circuit>::new(mode_a, false);
        let b = Boolean::<Circuit>::new(mode_b, true);
        let c = Boolean::<Circuit>::new(mode_c, false);
        check_adder("false ADD true WITH false", expected_sum, expected_carry, a, b, c, num_constants, num_public, num_private, num_constraints);
    }

    #[rustfmt::skip]
    fn check_false_add_true_with_true(
        mode_a: Mode,
        mode_b: Mode,
        mode_c: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64
    ) {
        // false ADD true WITH true => (false, true)
        let expected_sum = false;
        let expected_carry = true;
        let a = Boolean::<Circuit>::new(mode_a, false);
        let b = Boolean::<Circuit>::new(mode_b, true);
        let c = Boolean::<Circuit>::new(mode_c, true);
        check_adder("false ADD true WITH true", expected_sum, expected_carry, a, b, c, num_constants, num_public, num_private, num_constraints);
    }

    #[rustfmt::skip]
    fn check_true_add_false_with_false(
        mode_a: Mode,
        mode_b: Mode,
        mode_c: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64
    ) {
        // true ADD false WITH false => (true, false)
        let expected_sum = true;
        let expected_carry = false;
        let a = Boolean::<Circuit>::new(mode_a, true);
        let b = Boolean::<Circuit>::new(mode_b, false);
        let c = Boolean::<Circuit>::new(mode_c, false);
        check_adder("true ADD false WITH false", expected_sum, expected_carry, a, b, c, num_constants, num_public, num_private, num_constraints);
    }

    #[rustfmt::skip]
    fn check_true_add_false_with_true(
        mode_a: Mode,
        mode_b: Mode,
        mode_c: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64
    ) {
        // true ADD false WITH true => (false, true)
        let expected_sum = false;
        let expected_carry = true;
        let a = Boolean::<Circuit>::new(mode_a, true);
        let b = Boolean::<Circuit>::new(mode_b, false);
        let c = Boolean::<Circuit>::new(mode_c, true);
        check_adder("true ADD false WITH true", expected_sum, expected_carry, a, b, c, num_constants, num_public, num_private, num_constraints);
    }

    #[rustfmt::skip]
    fn check_true_add_true_with_false(
        mode_a: Mode,
        mode_b: Mode,
        mode_c: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64
    ) {
        // true ADD true WITH false => (false, true)
        let expected_sum = false;
        let expected_carry = true;
        let a = Boolean::<Circuit>::new(mode_a, true);
        let b = Boolean::<Circuit>::new(mode_b, true);
        let c = Boolean::<Circuit>::new(mode_c, false);
        check_adder("true ADD true WITH false", expected_sum, expected_carry, a, b, c, num_constants, num_public, num_private, num_constraints);
    }

    #[rustfmt::skip]
    fn check_true_add_true_with_true(
        mode_a: Mode,
        mode_b: Mode,
        mode_c: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64
    ) {
        // true ADD true WITH true => (true, true)
        let expected_sum = true;
        let expected_carry = true;
        let a = Boolean::<Circuit>::new(mode_a, true);
        let b = Boolean::<Circuit>::new(mode_b, true);
        let c = Boolean::<Circuit>::new(mode_c, true);
        check_adder("true ADD true WITH true", expected_sum, expected_carry, a, b, c, num_constants, num_public, num_private, num_constraints);
    }

    #[test]
    fn test_constant_add_constant_with_constant() {
        check_false_add_false_with_false(Mode::Constant, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_false_add_false_with_true(Mode::Constant, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_false_add_true_with_false(Mode::Constant, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_false_add_true_with_true(Mode::Constant, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_true_add_false_with_false(Mode::Constant, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_true_add_false_with_true(Mode::Constant, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_true_add_true_with_false(Mode::Constant, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_true_add_true_with_true(Mode::Constant, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_constant_add_constant_with_public() {
        check_false_add_false_with_false(Mode::Constant, Mode::Constant, Mode::Public, 0, 0, 0, 0);
        check_false_add_false_with_true(Mode::Constant, Mode::Constant, Mode::Public, 0, 0, 0, 0);
        check_false_add_true_with_false(Mode::Constant, Mode::Constant, Mode::Public, 0, 0, 0, 0);
        check_false_add_true_with_true(Mode::Constant, Mode::Constant, Mode::Public, 0, 0, 0, 0);
        check_true_add_false_with_false(Mode::Constant, Mode::Constant, Mode::Public, 0, 0, 0, 0);
        check_true_add_false_with_true(Mode::Constant, Mode::Constant, Mode::Public, 0, 0, 0, 0);
        check_true_add_true_with_false(Mode::Constant, Mode::Constant, Mode::Public, 0, 0, 0, 0);
        check_true_add_true_with_true(Mode::Constant, Mode::Constant, Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_constant_add_constant_with_private() {
        check_false_add_false_with_false(Mode::Constant, Mode::Constant, Mode::Private, 0, 0, 0, 0);
        check_false_add_false_with_true(Mode::Constant, Mode::Constant, Mode::Private, 0, 0, 0, 0);
        check_false_add_true_with_false(Mode::Constant, Mode::Constant, Mode::Private, 0, 0, 0, 0);
        check_false_add_true_with_true(Mode::Constant, Mode::Constant, Mode::Private, 0, 0, 0, 0);
        check_true_add_false_with_false(Mode::Constant, Mode::Constant, Mode::Private, 0, 0, 0, 0);
        check_true_add_false_with_true(Mode::Constant, Mode::Constant, Mode::Private, 0, 0, 0, 0);
        check_true_add_true_with_false(Mode::Constant, Mode::Constant, Mode::Private, 0, 0, 0, 0);
        check_true_add_true_with_true(Mode::Constant, Mode::Constant, Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_constant_add_public_with_constant() {
        check_false_add_false_with_false(Mode::Constant, Mode::Public, Mode::Constant, 0, 0, 0, 0);
        check_false_add_false_with_true(Mode::Constant, Mode::Public, Mode::Constant, 0, 0, 0, 0);
        check_false_add_true_with_false(Mode::Constant, Mode::Public, Mode::Constant, 0, 0, 0, 0);
        check_false_add_true_with_true(Mode::Constant, Mode::Public, Mode::Constant, 0, 0, 0, 0);
        check_true_add_false_with_false(Mode::Constant, Mode::Public, Mode::Constant, 0, 0, 0, 0);
        check_true_add_false_with_true(Mode::Constant, Mode::Public, Mode::Constant, 0, 0, 1, 1); // <- Differs
        check_true_add_true_with_false(Mode::Constant, Mode::Public, Mode::Constant, 0, 0, 0, 0);
        check_true_add_true_with_true(Mode::Constant, Mode::Public, Mode::Constant, 0, 0, 1, 1);
        // <- Differs
    }

    #[test]
    fn test_constant_add_public_with_public() {
        check_false_add_false_with_false(Mode::Constant, Mode::Public, Mode::Public, 0, 0, 2, 2);
        check_false_add_false_with_true(Mode::Constant, Mode::Public, Mode::Public, 0, 0, 2, 2);
        check_false_add_true_with_false(Mode::Constant, Mode::Public, Mode::Public, 0, 0, 2, 2);
        check_false_add_true_with_true(Mode::Constant, Mode::Public, Mode::Public, 0, 0, 2, 2);
        check_true_add_false_with_false(Mode::Constant, Mode::Public, Mode::Public, 0, 0, 3, 3); // <- Differs
        check_true_add_false_with_true(Mode::Constant, Mode::Public, Mode::Public, 0, 0, 3, 3); // <- Differs
        check_true_add_true_with_false(Mode::Constant, Mode::Public, Mode::Public, 0, 0, 3, 3); // <- Differs
        check_true_add_true_with_true(Mode::Constant, Mode::Public, Mode::Public, 0, 0, 3, 3);
        // <- Differs
    }

    #[test]
    fn test_constant_add_public_with_private() {
        check_false_add_false_with_false(Mode::Constant, Mode::Public, Mode::Private, 0, 0, 2, 2);
        check_false_add_false_with_true(Mode::Constant, Mode::Public, Mode::Private, 0, 0, 2, 2);
        check_false_add_true_with_false(Mode::Constant, Mode::Public, Mode::Private, 0, 0, 2, 2);
        check_false_add_true_with_true(Mode::Constant, Mode::Public, Mode::Private, 0, 0, 2, 2);
        check_true_add_false_with_false(Mode::Constant, Mode::Public, Mode::Private, 0, 0, 3, 3); // <- Differs
        check_true_add_false_with_true(Mode::Constant, Mode::Public, Mode::Private, 0, 0, 3, 3); // <- Differs
        check_true_add_true_with_false(Mode::Constant, Mode::Public, Mode::Private, 0, 0, 3, 3); // <- Differs
        check_true_add_true_with_true(Mode::Constant, Mode::Public, Mode::Private, 0, 0, 3, 3);
        // <- Differs
    }

    #[test]
    fn test_constant_add_private_with_constant() {
        check_false_add_false_with_false(Mode::Constant, Mode::Private, Mode::Constant, 0, 0, 0, 0);
        check_false_add_false_with_true(Mode::Constant, Mode::Private, Mode::Constant, 0, 0, 0, 0);
        check_false_add_true_with_false(Mode::Constant, Mode::Private, Mode::Constant, 0, 0, 0, 0);
        check_false_add_true_with_true(Mode::Constant, Mode::Private, Mode::Constant, 0, 0, 0, 0);
        check_true_add_false_with_false(Mode::Constant, Mode::Private, Mode::Constant, 0, 0, 0, 0);
        check_true_add_false_with_true(Mode::Constant, Mode::Private, Mode::Constant, 0, 0, 1, 1); // <- Differs
        check_true_add_true_with_false(Mode::Constant, Mode::Private, Mode::Constant, 0, 0, 0, 0);
        check_true_add_true_with_true(Mode::Constant, Mode::Private, Mode::Constant, 0, 0, 1, 1);
        // <- Differs
    }

    #[test]
    fn test_constant_add_private_with_public() {
        check_false_add_false_with_false(Mode::Constant, Mode::Private, Mode::Public, 0, 0, 2, 2);
        check_false_add_false_with_true(Mode::Constant, Mode::Private, Mode::Public, 0, 0, 2, 2);
        check_false_add_true_with_false(Mode::Constant, Mode::Private, Mode::Public, 0, 0, 2, 2);
        check_false_add_true_with_true(Mode::Constant, Mode::Private, Mode::Public, 0, 0, 2, 2);
        check_true_add_false_with_false(Mode::Constant, Mode::Private, Mode::Public, 0, 0, 3, 3); // <- Differs
        check_true_add_false_with_true(Mode::Constant, Mode::Private, Mode::Public, 0, 0, 3, 3); // <- Differs
        check_true_add_true_with_false(Mode::Constant, Mode::Private, Mode::Public, 0, 0, 3, 3); // <- Differs
        check_true_add_true_with_true(Mode::Constant, Mode::Private, Mode::Public, 0, 0, 3, 3);
        // <- Differs
    }

    #[test]
    fn test_constant_add_private_with_private() {
        check_false_add_false_with_false(Mode::Constant, Mode::Private, Mode::Private, 0, 0, 2, 2);
        check_false_add_false_with_true(Mode::Constant, Mode::Private, Mode::Private, 0, 0, 2, 2);
        check_false_add_true_with_false(Mode::Constant, Mode::Private, Mode::Private, 0, 0, 2, 2);
        check_false_add_true_with_true(Mode::Constant, Mode::Private, Mode::Private, 0, 0, 2, 2);
        check_true_add_false_with_false(Mode::Constant, Mode::Private, Mode::Private, 0, 0, 3, 3); // <- Differs
        check_true_add_false_with_true(Mode::Constant, Mode::Private, Mode::Private, 0, 0, 3, 3); // <- Differs
        check_true_add_true_with_false(Mode::Constant, Mode::Private, Mode::Private, 0, 0, 3, 3); // <- Differs
        check_true_add_true_with_true(Mode::Constant, Mode::Private, Mode::Private, 0, 0, 3, 3);
        // <- Differs
    }

    #[test]
    fn test_public_add_constant_with_constant() {
        check_false_add_false_with_false(Mode::Public, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_false_add_false_with_true(Mode::Public, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_false_add_true_with_false(Mode::Public, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_false_add_true_with_true(Mode::Public, Mode::Constant, Mode::Constant, 0, 0, 1, 1); // <- Differs
        check_true_add_false_with_false(Mode::Public, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_true_add_false_with_true(Mode::Public, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_true_add_true_with_false(Mode::Public, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
        check_true_add_true_with_true(Mode::Public, Mode::Constant, Mode::Constant, 0, 0, 1, 1);
        // <- Differs
    }

    #[test]
    fn test_public_add_constant_with_public() {
        check_false_add_false_with_false(Mode::Public, Mode::Constant, Mode::Public, 0, 0, 2, 2);
        check_false_add_false_with_true(Mode::Public, Mode::Constant, Mode::Public, 0, 0, 2, 2);
        check_false_add_true_with_false(Mode::Public, Mode::Constant, Mode::Public, 0, 0, 3, 3); // <- Differs
        check_false_add_true_with_true(Mode::Public, Mode::Constant, Mode::Public, 0, 0, 3, 3); // <- Differs
        check_true_add_false_with_false(Mode::Public, Mode::Constant, Mode::Public, 0, 0, 2, 2);
        check_true_add_false_with_true(Mode::Public, Mode::Constant, Mode::Public, 0, 0, 2, 2);
        check_true_add_true_with_false(Mode::Public, Mode::Constant, Mode::Public, 0, 0, 3, 3); // <- Differs
        check_true_add_true_with_true(Mode::Public, Mode::Constant, Mode::Public, 0, 0, 3, 3);
        // <- Differs
    }

    #[test]
    fn test_public_add_constant_with_private() {
        check_false_add_false_with_false(Mode::Public, Mode::Constant, Mode::Private, 0, 0, 2, 2);
        check_false_add_false_with_true(Mode::Public, Mode::Constant, Mode::Private, 0, 0, 2, 2);
        check_false_add_true_with_false(Mode::Public, Mode::Constant, Mode::Private, 0, 0, 3, 3); // <- Differs
        check_false_add_true_with_true(Mode::Public, Mode::Constant, Mode::Private, 0, 0, 3, 3); // <- Differs
        check_true_add_false_with_false(Mode::Public, Mode::Constant, Mode::Private, 0, 0, 2, 2);
        check_true_add_false_with_true(Mode::Public, Mode::Constant, Mode::Private, 0, 0, 2, 2);
        check_true_add_true_with_false(Mode::Public, Mode::Constant, Mode::Private, 0, 0, 3, 3); // <- Differs
        check_true_add_true_with_true(Mode::Public, Mode::Constant, Mode::Private, 0, 0, 3, 3);
        // <- Differs
    }

    #[test]
    fn test_public_add_public_with_constant() {
        check_false_add_false_with_false(Mode::Public, Mode::Public, Mode::Constant, 0, 0, 2, 2);
        check_false_add_false_with_true(Mode::Public, Mode::Public, Mode::Constant, 0, 0, 3, 3); // <- Differs
        check_false_add_true_with_false(Mode::Public, Mode::Public, Mode::Constant, 0, 0, 2, 2);
        check_false_add_true_with_true(Mode::Public, Mode::Public, Mode::Constant, 0, 0, 3, 3); // <- Differs
        check_true_add_false_with_false(Mode::Public, Mode::Public, Mode::Constant, 0, 0, 2, 2);
        check_true_add_false_with_true(Mode::Public, Mode::Public, Mode::Constant, 0, 0, 3, 3); // <- Differs
        check_true_add_true_with_false(Mode::Public, Mode::Public, Mode::Constant, 0, 0, 2, 2);
        check_true_add_true_with_true(Mode::Public, Mode::Public, Mode::Constant, 0, 0, 3, 3);
        // <- Differs
    }

    #[test]
    fn test_public_add_public_with_public() {
        check_false_add_false_with_false(Mode::Public, Mode::Public, Mode::Public, 0, 0, 5, 5);
        check_false_add_false_with_true(Mode::Public, Mode::Public, Mode::Public, 0, 0, 5, 5);
        check_false_add_true_with_false(Mode::Public, Mode::Public, Mode::Public, 0, 0, 5, 5);
        check_false_add_true_with_true(Mode::Public, Mode::Public, Mode::Public, 0, 0, 5, 5);
        check_true_add_false_with_false(Mode::Public, Mode::Public, Mode::Public, 0, 0, 5, 5);
        check_true_add_false_with_true(Mode::Public, Mode::Public, Mode::Public, 0, 0, 5, 5);
        check_true_add_true_with_false(Mode::Public, Mode::Public, Mode::Public, 0, 0, 5, 5);
        check_true_add_true_with_true(Mode::Public, Mode::Public, Mode::Public, 0, 0, 5, 5);
    }

    #[test]
    fn test_public_add_public_with_private() {
        check_false_add_false_with_false(Mode::Public, Mode::Public, Mode::Private, 0, 0, 5, 5);
        check_false_add_false_with_true(Mode::Public, Mode::Public, Mode::Private, 0, 0, 5, 5);
        check_false_add_true_with_false(Mode::Public, Mode::Public, Mode::Private, 0, 0, 5, 5);
        check_false_add_true_with_true(Mode::Public, Mode::Public, Mode::Private, 0, 0, 5, 5);
        check_true_add_false_with_false(Mode::Public, Mode::Public, Mode::Private, 0, 0, 5, 5);
        check_true_add_false_with_true(Mode::Public, Mode::Public, Mode::Private, 0, 0, 5, 5);
        check_true_add_true_with_false(Mode::Public, Mode::Public, Mode::Private, 0, 0, 5, 5);
        check_true_add_true_with_true(Mode::Public, Mode::Public, Mode::Private, 0, 0, 5, 5);
    }

    #[test]
    fn test_public_add_private_with_constant() {
        check_false_add_false_with_false(Mode::Public, Mode::Private, Mode::Constant, 0, 0, 2, 2);
        check_false_add_false_with_true(Mode::Public, Mode::Private, Mode::Constant, 0, 0, 3, 3); // <- Differs
        check_false_add_true_with_false(Mode::Public, Mode::Private, Mode::Constant, 0, 0, 2, 2);
        check_false_add_true_with_true(Mode::Public, Mode::Private, Mode::Constant, 0, 0, 3, 3); // <- Differs
        check_true_add_false_with_false(Mode::Public, Mode::Private, Mode::Constant, 0, 0, 2, 2);
        check_true_add_false_with_true(Mode::Public, Mode::Private, Mode::Constant, 0, 0, 3, 3); // <- Differs
        check_true_add_true_with_false(Mode::Public, Mode::Private, Mode::Constant, 0, 0, 2, 2);
        check_true_add_true_with_true(Mode::Public, Mode::Private, Mode::Constant, 0, 0, 3, 3);
        // <- Differs
    }

    #[test]
    fn test_public_add_private_with_public() {
        check_false_add_false_with_false(Mode::Public, Mode::Private, Mode::Public, 0, 0, 5, 5);
        check_false_add_false_with_true(Mode::Public, Mode::Private, Mode::Public, 0, 0, 5, 5);
        check_false_add_true_with_false(Mode::Public, Mode::Private, Mode::Public, 0, 0, 5, 5);
        check_false_add_true_with_true(Mode::Public, Mode::Private, Mode::Public, 0, 0, 5, 5);
        check_true_add_false_with_false(Mode::Public, Mode::Private, Mode::Public, 0, 0, 5, 5);
        check_true_add_false_with_true(Mode::Public, Mode::Private, Mode::Public, 0, 0, 5, 5);
        check_true_add_true_with_false(Mode::Public, Mode::Private, Mode::Public, 0, 0, 5, 5);
        check_true_add_true_with_true(Mode::Public, Mode::Private, Mode::Public, 0, 0, 5, 5);
    }

    #[test]
    fn test_public_add_private_with_private() {
        check_false_add_false_with_false(Mode::Public, Mode::Private, Mode::Private, 0, 0, 5, 5);
        check_false_add_false_with_true(Mode::Public, Mode::Private, Mode::Private, 0, 0, 5, 5);
        check_false_add_true_with_false(Mode::Public, Mode::Private, Mode::Private, 0, 0, 5, 5);
        check_false_add_true_with_true(Mode::Public, Mode::Private, Mode::Private, 0, 0, 5, 5);
        check_true_add_false_with_false(Mode::Public, Mode::Private, Mode::Private, 0, 0, 5, 5);
        check_true_add_false_with_true(Mode::Public, Mode::Private, Mode::Private, 0, 0, 5, 5);
        check_true_add_true_with_false(Mode::Public, Mode::Private, Mode::Private, 0, 0, 5, 5);
        check_true_add_true_with_true(Mode::Public, Mode::Private, Mode::Private, 0, 0, 5, 5);
    }
}
