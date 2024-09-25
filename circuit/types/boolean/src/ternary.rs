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

impl<E: Environment> Ternary for Boolean<E> {
    type Boolean = Boolean<E>;
    type Output = Self;

    /// Returns `first` if `condition` is `true`, otherwise returns `second`.
    fn ternary(condition: &Self::Boolean, first: &Self, second: &Self) -> Self::Output {
        // Constant `condition`
        if condition.is_constant() {
            match condition.eject_value() {
                true => first.clone(),
                false => second.clone(),
            }
        }
        // Constant `first`
        else if first.is_constant() {
            match first.eject_value() {
                true => condition | second,
                false => !condition & second,
            }
        }
        // Constant `second`
        else if second.is_constant() {
            match second.eject_value() {
                true => !condition | first,
                false => condition & first,
            }
        }
        // Variables
        else {
            // Compute the witness value, based on the condition.
            let witness = match condition.eject_value() {
                true => first.eject_value(),
                false => second.eject_value(),
            };

            // Declare a new variable with the expected output as witness.
            // Note: The constraint below will ensure `output` is either 0 or 1,
            // assuming `self` and `other` are well-formed (they are either 0 or 1).
            let output = Boolean(
                E::new_variable(Mode::Private, match witness {
                    true => E::BaseField::one(),
                    false => E::BaseField::zero(),
                })
                .into(),
            );

            //
            // Ternary Enforcement
            // -------------------------------------------------------
            //    output = condition * a + (1 - condition) * b
            // => output = b + condition * (a - b)
            // => condition * (a - b) = output - b
            //
            // See `Field::ternary()` for the proof of correctness.
            //
            E::enforce(|| (condition, (&first.0 - &second.0), (&output.0 - &second.0)));

            output
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    fn check_ternary(
        name: &str,
        expected: bool,
        condition: Boolean<Circuit>,
        a: Boolean<Circuit>,
        b: Boolean<Circuit>,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        Circuit::scope(name, || {
            let case = format!("({} ? {} : {})", condition.eject_value(), a.eject_value(), b.eject_value());
            let candidate = Boolean::ternary(&condition, &a, &b);
            assert_eq!(expected, candidate.eject_value(), "{case}");
            assert_scope!(num_constants, num_public, num_private, num_constraints);
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
        for flag in [true, false] {
            for first in [true, false] {
                for second in [true, false] {
                    let condition = Boolean::<Circuit>::new(mode_condition, flag);
                    let a = Boolean::<Circuit>::new(mode_a, first);
                    let b = Boolean::<Circuit>::new(mode_b, second);

                    let name = format!("{mode_condition} ? {mode_a} : {mode_b}");
                    check_ternary(
                        &name,
                        if flag { first } else { second },
                        condition,
                        a,
                        b,
                        num_constants,
                        num_public,
                        num_private,
                        num_constraints,
                    );
                }
            }
        }
    }

    #[test]
    fn test_if_constant_then_constant_else_constant() {
        run_test(Mode::Constant, Mode::Constant, Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_constant_else_public() {
        run_test(Mode::Constant, Mode::Constant, Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_constant_else_private() {
        run_test(Mode::Constant, Mode::Constant, Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_public_else_constant() {
        run_test(Mode::Constant, Mode::Public, Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_public_else_public() {
        run_test(Mode::Constant, Mode::Public, Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_public_else_private() {
        run_test(Mode::Constant, Mode::Public, Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_private_else_constant() {
        run_test(Mode::Constant, Mode::Private, Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_if_constant_then_private_else_public() {
        run_test(Mode::Constant, Mode::Private, Mode::Public, 0, 0, 0, 0);
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
