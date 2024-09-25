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

impl<E: Environment> Ternary for Scalar<E> {
    type Boolean = Boolean<E>;
    type Output = Self;

    /// Returns `first` if `condition` is `true`, otherwise returns `second`.
    fn ternary(condition: &Self::Boolean, first: &Self, second: &Self) -> Self::Output {
        // Compute the ternary over the field representation (for efficiency).
        let field = Field::ternary(condition, &first.field, &second.field);
        // Return the result.
        Self { field, bits_le: Default::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 32;

    fn check_ternary(
        name: &str,
        flag: bool,
        first: console::Scalar<<Circuit as Environment>::Network>,
        second: console::Scalar<<Circuit as Environment>::Network>,
        mode_condition: Mode,
        mode_a: Mode,
        mode_b: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        let expected = if flag { first } else { second };
        let condition = Boolean::<Circuit>::new(mode_condition, flag);
        let a = Scalar::<Circuit>::new(mode_a, first);
        let b = Scalar::<Circuit>::new(mode_b, second);

        Circuit::scope(name, || {
            let case = format!("({} ? {} : {})", condition.eject_value(), a.eject_value(), b.eject_value());
            let candidate = Scalar::ternary(&condition, &a, &b);
            assert_eq!(expected, candidate.eject_value(), "{case}");
            assert_scope!(num_constants, num_public, num_private, num_constraints);

            // Check that `candidate` has a valid mode.
            candidate.eject_mode()
        });
    }

    fn run_test(
        mode_condition: Mode,
        mode_a: Mode,
        mode_b: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        let check_ternary = |name: &str, flag, first, second| {
            check_ternary(
                name,
                flag,
                first,
                second,
                mode_condition,
                mode_a,
                mode_b,
                num_constants,
                num_public,
                num_private,
                num_constraints,
            )
        };

        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            for flag in [true, false] {
                let name = format!("{flag} ? {mode_a} : {mode_b}, {i}");

                let first = Uniform::rand(&mut rng);
                let second = Uniform::rand(&mut rng);

                check_ternary(&name, flag, first, second);
            }
        }

        let zero = console::Scalar::<<Circuit as Environment>::Network>::zero();
        let one = console::Scalar::<<Circuit as Environment>::Network>::one();

        check_ternary("true ? zero : zero", true, zero, zero);
        check_ternary("true ? zero : one", true, zero, one);
        check_ternary("true ? one : zero", true, one, zero);
        check_ternary("true ? one : one", true, one, one);

        check_ternary("false ? zero : zero", false, zero, zero);
        check_ternary("false ? zero : one", false, zero, one);
        check_ternary("false ? one : zero", false, one, zero);
        check_ternary("false ? one : one", false, one, one);
    }

    #[test]
    fn test_if_constant_then_constant_else_constant() {
        run_test(Mode::Constant, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_constant_else_public() {
        run_test(Mode::Constant, Mode::Public, Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_constant_else_private() {
        run_test(Mode::Constant, Mode::Private, Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_public_else_constant() {
        run_test(Mode::Constant, Mode::Constant, Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_public_else_public() {
        run_test(Mode::Constant, Mode::Public, Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_public_else_private() {
        run_test(Mode::Constant, Mode::Private, Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_private_else_constant() {
        run_test(Mode::Constant, Mode::Constant, Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_private_else_public() {
        run_test(Mode::Constant, Mode::Public, Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_private_else_private() {
        run_test(Mode::Constant, Mode::Private, Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_public_then_constant_else_constant() {
        run_test(Mode::Public, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_public_then_constant_else_public() {
        run_test(Mode::Public, Mode::Constant, Mode::Public, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_public_then_constant_else_private() {
        run_test(Mode::Public, Mode::Constant, Mode::Private, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_public_then_public_else_constant() {
        run_test(Mode::Public, Mode::Public, Mode::Constant, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_public_then_public_else_public() {
        run_test(Mode::Public, Mode::Public, Mode::Public, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_public_then_public_else_private() {
        run_test(Mode::Public, Mode::Public, Mode::Private, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_public_then_private_else_constant() {
        run_test(Mode::Public, Mode::Private, Mode::Constant, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_public_then_private_else_public() {
        run_test(Mode::Public, Mode::Private, Mode::Public, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_public_then_private_else_private() {
        run_test(Mode::Public, Mode::Private, Mode::Private, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_private_then_constant_else_constant() {
        run_test(Mode::Private, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_private_then_constant_else_public() {
        run_test(Mode::Private, Mode::Constant, Mode::Public, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_private_then_constant_else_private() {
        run_test(Mode::Private, Mode::Constant, Mode::Private, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_private_then_public_else_constant() {
        run_test(Mode::Private, Mode::Public, Mode::Constant, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_private_then_public_else_public() {
        run_test(Mode::Private, Mode::Public, Mode::Public, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_private_then_public_else_private() {
        run_test(Mode::Private, Mode::Public, Mode::Private, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_private_then_private_else_constant() {
        run_test(Mode::Private, Mode::Private, Mode::Constant, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_private_then_private_else_public() {
        run_test(Mode::Private, Mode::Private, Mode::Public, 0, 0, 1, 1);
    }

    #[test]
    fn test_if_private_then_private_else_private() {
        run_test(Mode::Private, Mode::Private, Mode::Private, 0, 0, 1, 1);
    }
}
