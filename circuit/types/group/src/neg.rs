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

impl<E: Environment> Neg for Group<E> {
    type Output = Self;

    /// Performs the unary `-` operation.
    fn neg(self) -> Self::Output {
        Group { x: -self.x, y: self.y }
    }
}

impl<E: Environment> Neg for &Group<E> {
    type Output = Group<E>;

    /// Performs the unary `-` operation.
    fn neg(self) -> Self::Output {
        -(self.clone())
    }
}

impl<E: Environment> Metrics<dyn Neg<Output = Group<E>>> for Group<E> {
    type Case = Mode;

    fn count(_case: &Self::Case) -> Count {
        Count::is(0, 0, 0, 0)
    }
}

impl<E: Environment> OutputMode<dyn Neg<Output = Group<E>>> for Group<E> {
    type Case = Mode;

    fn output_mode(case: &Self::Case) -> Mode {
        match case {
            Mode::Constant => Mode::Constant,
            _ => Mode::Private,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 100;

    fn check_neg(
        name: &str,
        expected: console::Group<<Circuit as Environment>::Network>,
        candidate_input: Group<Circuit>,
    ) {
        Circuit::scope(name, || {
            let mode = candidate_input.eject_mode();
            let candidate_output = -candidate_input;
            assert_eq!(expected, candidate_output.eject_value());
            assert_count!(Neg(Group) => Group, &mode);
            assert_output_mode!(Neg(Group) => Group, &mode, candidate_output);
        });
    }

    #[test]
    fn test_neg_constant() {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let point: console::Group<_> = Uniform::rand(&mut rng);
            let expected: console::Group<_> = -point;

            let candidate_input = Group::<Circuit>::new(Mode::Constant, point);
            check_neg(&format!("NEG Constant {i}"), expected, candidate_input);
        }
    }

    #[test]
    fn test_neg_public() {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let point: console::Group<_> = Uniform::rand(&mut rng);
            let expected: console::Group<_> = -point;

            let candidate_input = Group::<Circuit>::new(Mode::Public, point);
            check_neg(&format!("NEG Public {i}"), expected, candidate_input);
        }
    }

    #[test]
    fn test_neg_private() {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let point: console::Group<_> = Uniform::rand(&mut rng);
            let expected: console::Group<_> = -point;

            let candidate_input = Group::<Circuit>::new(Mode::Private, point);
            check_neg(&format!("NEG Private {i}"), expected, candidate_input);
        }
    }

    #[test]
    fn test_zero() {
        let expected = console::Group::<<Circuit as Environment>::Network>::zero();

        let candidate_input = Group::<Circuit>::zero();
        check_neg("NEG Constant Zero", expected, candidate_input);

        let candidate_input = Group::<Circuit>::new(Mode::Public, expected);
        check_neg("NEG Public Zero", expected, candidate_input);

        let candidate_input = Group::<Circuit>::new(Mode::Private, expected);
        check_neg("NEG Private Zero", expected, candidate_input);
    }
}
