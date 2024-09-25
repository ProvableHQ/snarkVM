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
use std::rc::Rc;

impl<E: Environment> Not for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns `(NOT a)`.
    fn not(self) -> Self::Output {
        (&self).not()
    }
}

impl<E: Environment> Not for &Boolean<E> {
    type Output = Boolean<E>;

    /// Returns `(NOT a)`.
    fn not(self) -> Self::Output {
        // The `NOT` operation behaves as follows:
        //     Case 1: If `(self == 0)`, then `(1 - self) == 1`.
        //     Case 2: If `(self == 1)`, then `(1 - self) == 0`.
        match self.is_constant() {
            // Constant case.
            true => Boolean(E::one() - &self.0),
            // Public and private cases.
            // Note: We directly instantiate a public variable to correctly represent a boolean in a linear combination.
            // For more information, see `LinearCombination::is_boolean_type`.
            false => Boolean(Variable::Public(Rc::new((0, E::BaseField::one()))) - &self.0),
        }
    }
}

impl<E: Environment> Metrics<dyn Not<Output = Boolean<E>>> for Boolean<E> {
    type Case = Mode;

    fn count(_case: &Self::Case) -> Count {
        Count::is(0, 0, 0, 0)
    }
}

impl<E: Environment> OutputMode<dyn Not<Output = Boolean<E>>> for Boolean<E> {
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

    fn check_not(name: &str, expected: bool, candidate_input: Boolean<Circuit>) {
        Circuit::scope(name, || {
            let mode = candidate_input.mode();
            let candidate_output = !candidate_input;
            assert_eq!(expected, candidate_output.eject_value());
            assert_count!(Not(Boolean) => Boolean, &mode);
            assert_output_mode!(Not(Boolean) => Boolean, &mode, candidate_output);
        });
    }

    #[test]
    fn test_not_constant() {
        // NOT false
        let expected = true;
        let candidate_input = Boolean::<Circuit>::new(Mode::Constant, false);
        check_not("NOT false", expected, candidate_input);

        // NOT true
        let expected = false;
        let candidate_input = Boolean::<Circuit>::new(Mode::Constant, true);
        check_not("NOT true", expected, candidate_input);
    }

    #[test]
    fn test_not_public() {
        // NOT false
        let expected = true;
        let candidate_input = Boolean::<Circuit>::new(Mode::Public, false);
        check_not("NOT false", expected, candidate_input);

        // NOT true
        let expected = false;
        let candidate_input = Boolean::<Circuit>::new(Mode::Public, true);
        check_not("NOT true", expected, candidate_input);
    }

    #[test]
    fn test_not_private() {
        // NOT false
        let expected = true;
        let candidate_input = Boolean::<Circuit>::new(Mode::Private, false);
        check_not("NOT false", expected, candidate_input);

        // NOT true
        let expected = false;
        let candidate_input = Boolean::<Circuit>::new(Mode::Private, true);
        check_not("NOT true", expected, candidate_input);
    }
}
