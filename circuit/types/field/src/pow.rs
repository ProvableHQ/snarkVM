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
use snarkvm_circuit_environment::{Circuit, CircuitType};

#[allow(clippy::only_used_in_recursion)]
impl<E: Environment> Pow<Field<E>> for Field<E> {
    type Output = Field<E>;

    fn pow(self, exponent: Field<E>) -> Self::Output {
        self.pow(&exponent)
    }
}

impl<E: Environment> Pow<Field<E>> for &Field<E> {
    type Output = Field<E>;

    fn pow(self, exponent: Field<E>) -> Self::Output {
        self.pow(&exponent)
    }
}

#[allow(clippy::needless_borrow)]
impl<E: Environment> Pow<&Field<E>> for Field<E> {
    type Output = Field<E>;

    fn pow(self, exponent: &Field<E>) -> Self::Output {
        (&self).pow(exponent)
    }
}

impl<E: Environment> Pow<&Field<E>> for &Field<E> {
    type Output = Field<E>;

    fn pow(self, exponent: &Field<E>) -> Self::Output {
        // Initialize the output.
        let mut output = Field::one();

        // If the exponent is a constant, eject its bits to determine whether to multiply in each iteration.
        if exponent.is_constant() {
            for bit in exponent.to_bits_be() {
                // Square the output.
                output = output.square();
                // If `bit` is `true, set the output to `output * self`.
                if bit.eject_value() {
                    output *= self;
                }
            }
        }
        // If the exponent is a variable, use a ternary to select whether to multiply in each iteration.
        else {
            for bit in exponent.to_bits_be() {
                // Square the output.
                output = output.square();
                // If `bit` is `true, set the output to `output * self`.
                output = Field::ternary(&bit, &(&output * self), &output);
            }
        }

        output
    }
}

impl<E: Environment> Metrics<dyn Pow<Field<E>, Output = Field<E>>> for Field<E> {
    type Case = (CircuitType<Field<E>>, CircuitType<Field<E>>);

    fn count(case: &Self::Case) -> Count {
        match (case.0.mode(), case.1.mode()) {
            (Mode::Constant, Mode::Constant) => Count::is(253, 0, 0, 0),
            (_, Mode::Constant) => match &case.1 {
                CircuitType::Constant(constant) => {
                    // Find the first instance (from the MSB) of a `true` bit.
                    let exponent_bits = constant.eject_value().to_bits_be();
                    let index = exponent_bits
                        .iter()
                        .position(|b| *b)
                        .unwrap_or(console::Field::<<Circuit as Environment>::Network>::size_in_bits() - 1);

                    // Calculate the number of squares and multiplications as follows:
                    //   `num_squares` := number of remaining bits after the first nonzero bit (from MSB -> LSB)
                    //   `num_multiplications` := number of `true` bits after the first nonzero bit (from MSB -> LSB)
                    let num_squares =
                        (console::Field::<<Circuit as Environment>::Network>::size_in_bits() - index - 1) as u64;
                    let num_multiplications = exponent_bits[index + 1..].iter().map(|bit| *bit as u64).sum::<u64>();

                    // The number of private variables, constraints, and gates are both: num_squares + num_multiplications
                    let num_private = num_squares + num_multiplications;
                    let num_constraints = num_private;
                    Count::is(253, 0, num_private, num_constraints)
                }
                _ => E::halt(format!(
                    "Constant is required to determine the `Count` for {} POW {}",
                    case.0.mode(),
                    case.1.mode()
                )),
            },
            (Mode::Constant, _) => Count::is(0, 0, 1009, 1011),
            (_, _) => Count::is(0, 0, 1262, 1264),
        }
    }
}

impl<E: Environment> OutputMode<dyn Pow<Field<E>, Output = Field<E>>> for Field<E> {
    type Case = (CircuitType<Field<E>>, CircuitType<Field<E>>);

    fn output_mode(case: &Self::Case) -> Mode {
        match (case.0.mode(), case.1.mode()) {
            (Mode::Constant, Mode::Constant) => Mode::Constant,
            (mode_a, Mode::Constant) => match &case.1 {
                CircuitType::Constant(constant) => match constant.eject_value() {
                    value if value.is_zero() => Mode::Constant,
                    value if value.is_one() => mode_a,
                    _ => Mode::Private,
                },
                _ => E::halt("The constant is required to determine the output mode of Public * Constant"),
            },
            (_, _) => Mode::Private,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 10;

    fn check_pow(
        name: &str,
        expected: &console::Field<<Circuit as Environment>::Network>,
        a: &Field<Circuit>,
        b: &Field<Circuit>,
    ) {
        Circuit::scope(name, || {
            let candidate = a.pow(b);
            assert_eq!(*expected, candidate.eject_value(), "({}^{})", a.eject_value(), b.eject_value());
            assert_count!(Pow(Field, Field) => Field, &(CircuitType::from(a), CircuitType::from(b)));
            assert_output_mode!(Pow(Field, Field) => Field, &(CircuitType::from(a), CircuitType::from(b)), candidate);
        });
    }

    fn run_test(mode_a: Mode, mode_b: Mode) {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            let first = Uniform::rand(&mut rng);
            let second = Uniform::rand(&mut rng);

            let a = Field::<Circuit>::new(mode_a, first);
            let b = Field::<Circuit>::new(mode_b, second);

            let expected = first.pow(second);

            let name = format!("Pow: a ^ b {i}");
            check_pow(&name, &expected, &a, &b);

            // Test one exponent.
            let name = format!("Pow: a ^ 1 {i}");
            let a = Field::<Circuit>::new(mode_a, first);
            let one = Field::<Circuit>::new(mode_b, console::Field::<<Circuit as Environment>::Network>::one());
            check_pow(&name, &first, &a, &one);

            // Test one base.
            let name = format!("Pow: 1 ^ b {i}");
            let one = Field::<Circuit>::new(mode_a, console::Field::<<Circuit as Environment>::Network>::one());
            let b = Field::<Circuit>::new(mode_b, second);
            check_pow(&name, &console::Field::<<Circuit as Environment>::Network>::one(), &one, &b);

            // Test zero exponent.
            let name = format!("Pow: a ^ 0 {i}");
            let a = Field::<Circuit>::new(mode_a, first);
            let zero = Field::<Circuit>::new(mode_b, console::Field::<<Circuit as Environment>::Network>::zero());
            check_pow(&name, &console::Field::<<Circuit as Environment>::Network>::one(), &a, &zero);

            // Test zero base.
            let name = format!("Mul: 0 ^ b {i}");
            let zero = Field::<Circuit>::new(mode_a, console::Field::<<Circuit as Environment>::Network>::zero());
            let b = Field::<Circuit>::new(mode_b, second);
            check_pow(&name, &console::Field::<<Circuit as Environment>::Network>::zero(), &zero, &b);
        }

        let zero = console::Field::<<Circuit as Environment>::Network>::zero();
        let one = console::Field::<<Circuit as Environment>::Network>::one();

        // Test 0 ^ 0.
        let name = "Mul: 0 ^ 0";
        check_pow(name, &one, &Field::<Circuit>::new(mode_a, zero), &Field::<Circuit>::new(mode_b, zero));

        // Test 1 ^ 0.
        let name = "Pow: 1 ^ 0";
        check_pow(name, &one, &Field::<Circuit>::new(mode_a, one), &Field::<Circuit>::new(mode_b, zero));

        // Test 0 ^ 1.
        let name = "Pow: 0 ^ 1";
        check_pow(name, &zero, &Field::<Circuit>::new(mode_a, zero), &Field::<Circuit>::new(mode_b, one));

        // Test 1 ^ 1.
        let name = "Pow: 1 ^ 1";
        check_pow(name, &one, &Field::<Circuit>::new(mode_a, one), &Field::<Circuit>::new(mode_b, one));
    }

    #[test]
    fn test_constant_pow_constant() {
        run_test(Mode::Constant, Mode::Constant);
    }

    #[test]
    fn test_constant_pow_public() {
        run_test(Mode::Constant, Mode::Public);
    }

    #[test]
    fn test_constant_pow_private() {
        run_test(Mode::Constant, Mode::Private);
    }

    #[test]
    fn test_public_pow_constant() {
        run_test(Mode::Public, Mode::Constant);
    }

    #[test]
    fn test_private_pow_constant() {
        run_test(Mode::Private, Mode::Constant);
    }

    #[test]
    fn test_public_pow_public() {
        run_test(Mode::Public, Mode::Public);
    }

    #[test]
    fn test_public_pow_private() {
        run_test(Mode::Public, Mode::Private);
    }

    #[test]
    fn test_private_pow_public() {
        run_test(Mode::Private, Mode::Public);
    }

    #[test]
    fn test_private_pow_private() {
        run_test(Mode::Private, Mode::Private)
    }
}
