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

impl<E: Environment> Square for Field<E> {
    type Output = Field<E>;

    fn square(&self) -> Self::Output {
        self * self
    }
}

impl<E: Environment> Metrics<dyn Square<Output = Field<E>>> for Field<E> {
    type Case = Mode;

    fn count(case: &Self::Case) -> Count {
        match case.is_constant() {
            true => Count::is(0, 0, 0, 0),
            false => Count::is(0, 0, 1, 1),
        }
    }
}

impl<E: Environment> OutputMode<dyn Square<Output = Field<E>>> for Field<E> {
    type Case = Mode;

    fn output_mode(input: &Self::Case) -> Mode {
        match input.is_constant() {
            true => Mode::Constant,
            false => Mode::Private,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 500;

    fn check_square(name: &str, expected: &console::Field<<Circuit as Environment>::Network>, a: &Field<Circuit>) {
        Circuit::scope(name, || {
            let result = a.square();
            assert_eq!(*expected, result.eject_value());
            assert_count!(Square(Field) => Field, &(a.eject_mode()));
            assert_output_mode!(Square(Field) => Field, &(a.eject_mode()), result);
        });
    }

    fn run_test(mode: Mode, rng: &mut TestRng) {
        for i in 0..ITERATIONS {
            // Sample a random element
            let first = Uniform::rand(rng);
            let a = Field::<Circuit>::new(mode, first);

            let name = format!("Square: {i}");
            check_square(&name, &first.square(), &a);
        }

        // Test zero case.
        let name = "Square Zero";
        let zero = console::Field::<<Circuit as Environment>::Network>::zero();
        check_square(name, &zero, &Field::new(mode, zero));

        // Test one case.
        let name = "Square One";
        let one = console::Field::<<Circuit as Environment>::Network>::one();
        check_square(name, &one, &Field::new(mode, one));
    }

    #[test]
    fn test_square() {
        let mut rng = TestRng::default();

        run_test(Mode::Constant, &mut rng);
        run_test(Mode::Public, &mut rng);
        run_test(Mode::Private, &mut rng);
    }

    #[test]
    fn test_0_square() {
        let zero = console::Field::<<Circuit as Environment>::Network>::zero();

        let candidate = Field::<Circuit>::new(Mode::Public, zero).square();
        assert_eq!(zero, candidate.eject_value());
    }

    #[test]
    fn test_1_double() {
        let one = console::Field::<<Circuit as Environment>::Network>::one();

        let candidate = Field::<Circuit>::new(Mode::Public, one).square();
        assert_eq!(one, candidate.eject_value());
    }

    #[test]
    fn test_2_double() {
        let one = console::Field::<<Circuit as Environment>::Network>::one();
        let two = one + one;
        let four = two.square();

        let candidate = (Field::<Circuit>::new(Mode::Public, one) + Field::new(Mode::Public, one)).square();
        assert_eq!(four, candidate.eject_value());
    }
}
