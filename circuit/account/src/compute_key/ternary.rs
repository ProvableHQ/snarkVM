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

impl<A: Aleo> Ternary for ComputeKey<A> {
    type Boolean = Boolean<A>;
    type Output = ComputeKey<A>;

    /// Returns `first` if `condition` is `true`, otherwise returns `second`.
    fn ternary(condition: &Self::Boolean, first: &Self, second: &Self) -> Self::Output {
        Self {
            pk_sig: Group::ternary(condition, &first.pk_sig, &second.pk_sig),
            pr_sig: Group::ternary(condition, &first.pr_sig, &second.pr_sig),
            sk_prf: Scalar::ternary(condition, &first.sk_prf, &second.sk_prf),
        }
    }
}

impl<A: Aleo> Metrics<dyn Ternary<Boolean = Boolean<A>, Output = ComputeKey<A>>> for ComputeKey<A> {
    type Case = (Mode, Mode, Mode);

    fn count(case: &Self::Case) -> Count {
        match case {
            (Mode::Constant, _, _)
            | (Mode::Public, Mode::Constant, Mode::Constant)
            | (Mode::Private, Mode::Constant, Mode::Constant) => Count::is(0, 0, 0, 0),
            _ => Count::is(0, 0, 5, 5),
        }
    }
}

impl<A: Aleo> OutputMode<dyn Ternary<Boolean = Boolean<A>, Output = Self>> for ComputeKey<A> {
    type Case = (CircuitType<Boolean<A>>, Mode, Mode);

    fn output_mode(parameter: &Self::Case) -> Mode {
        match parameter.0.mode().is_constant() {
            true => match &parameter.0 {
                CircuitType::Constant(constant) => match constant.eject_value() {
                    true => parameter.1,
                    false => parameter.2,
                },
                _ => A::halt("The constant condition is required to determine output mode."),
            },
            false => Mode::Private,
        }
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use crate::Circuit;
    use console::TestRng;

    fn check_ternary(
        name: &str,
        expected: console::ComputeKey<<Circuit as Environment>::Network>,
        condition: Boolean<Circuit>,
        a: ComputeKey<Circuit>,
        b: ComputeKey<Circuit>,
    ) {
        Circuit::scope(name, || {
            let case = format!("({} ? {:?} : {:?})", condition.eject_value(), a.eject_value(), b.eject_value());
            let candidate = ComputeKey::ternary(&condition, &a, &b);
            assert_eq!(expected, candidate.eject_value(), "{case}");
            assert_count!(Ternary(Boolean, ComputeKey, ComputeKey) => ComputeKey, &(condition.eject_mode(), a.eject_mode(), b.eject_mode()));
            assert_output_mode!(Ternary(Boolean, ComputeKey, ComputeKey) => ComputeKey, &(CircuitType::from(&condition), a.eject_mode(), b.eject_mode()), candidate);
        });
    }

    #[test]
    fn test_constant_condition() {
        let mut rng = TestRng::default();

        let first = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();
        let second = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();

        // false ? Constant : Constant
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Constant, false);
        let a = ComputeKey::<Circuit>::new(Mode::Constant, first);
        let b = ComputeKey::<Circuit>::new(Mode::Constant, second);
        check_ternary("false ? Constant : Constant", expected, condition, a, b);

        // false ? Constant : Public
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Constant, false);
        let a = ComputeKey::<Circuit>::new(Mode::Constant, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("false ? Constant : Public", expected, condition, a, b);

        // false ? Public : Constant
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Constant, false);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Constant, second);
        check_ternary("false ? Public : Constant", expected, condition, a, b);

        // false ? Public : Public
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Constant, false);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("false ? Public : Public", expected, condition, a, b);

        // false ? Public : Private
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Constant, false);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Private, second);
        check_ternary("false ? Public : Private", expected, condition, a, b);

        // false ? Private : Private
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Constant, false);
        let a = ComputeKey::<Circuit>::new(Mode::Private, first);
        let b = ComputeKey::<Circuit>::new(Mode::Private, second);
        check_ternary("false ? Private : Private", expected, condition, a, b);

        // true ? Constant : Constant
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Constant, true);
        let a = ComputeKey::<Circuit>::new(Mode::Constant, first);
        let b = ComputeKey::<Circuit>::new(Mode::Constant, second);
        check_ternary("true ? Constant : Constant", expected, condition, a, b);

        // true ? Constant : Public
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Constant, true);
        let a = ComputeKey::<Circuit>::new(Mode::Constant, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("true ? Constant : Public", expected, condition, a, b);

        // true ? Public : Constant
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Constant, true);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Constant, second);
        check_ternary("true ? Public : Constant", expected, condition, a, b);

        // true ? Public : Public
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Constant, true);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("true ? Public : Public", expected, condition, a, b);

        // true ? Public : Private
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Constant, true);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Private, second);
        check_ternary("true ? Public : Private", expected, condition, a, b);

        // true ? Private : Private
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Constant, true);
        let a = ComputeKey::<Circuit>::new(Mode::Private, first);
        let b = ComputeKey::<Circuit>::new(Mode::Private, second);
        check_ternary("true ? Private : Private", expected, condition, a, b);
    }

    #[test]
    fn test_public_condition_and_constant_inputs() {
        let mut rng = TestRng::default();

        let first = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();
        let second = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();

        // false ? Constant : Constant
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Public, false);
        let a = ComputeKey::<Circuit>::new(Mode::Constant, first);
        let b = ComputeKey::<Circuit>::new(Mode::Constant, second);
        check_ternary("false ? Constant : Constant", expected, condition, a, b);

        // true ? Constant : Constant
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Public, true);
        let a = ComputeKey::<Circuit>::new(Mode::Constant, first);
        let b = ComputeKey::<Circuit>::new(Mode::Constant, second);
        check_ternary("true ? Constant : Constant", expected, condition, a, b);
    }

    #[test]
    fn test_public_condition_and_mixed_inputs() {
        let mut rng = TestRng::default();

        let first = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();
        let second = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();

        // false ? Constant : Public
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Public, false);
        let a = ComputeKey::<Circuit>::new(Mode::Constant, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("false ? Constant : Public", expected, condition, a, b);

        // false ? Public : Constant
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Public, false);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Constant, second);
        check_ternary("false ? Public : Constant", expected, condition, a, b);

        // true ? Constant : Public
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Public, true);
        let a = ComputeKey::<Circuit>::new(Mode::Constant, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("true ? Constant : Public", expected, condition, a, b);

        // true ? Public : Constant
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Public, true);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Constant, second);
        check_ternary("true ? Public : Constant", expected, condition, a, b);
    }

    #[test]
    fn test_private_condition_and_constant_inputs() {
        let mut rng = TestRng::default();

        let first = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();
        let second = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();

        // false ? Constant : Constant
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Private, false);
        let a = ComputeKey::<Circuit>::new(Mode::Constant, first);
        let b = ComputeKey::<Circuit>::new(Mode::Constant, second);
        check_ternary("false ? Constant : Constant", expected, condition, a, b);

        // true ? Constant : Constant
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Private, true);
        let a = ComputeKey::<Circuit>::new(Mode::Constant, first);
        let b = ComputeKey::<Circuit>::new(Mode::Constant, second);
        check_ternary("true ? Constant : Constant", expected, condition, a, b);
    }

    #[test]
    fn test_private_condition_and_mixed_inputs() {
        let mut rng = TestRng::default();

        let first = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();
        let second = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();

        // false ? Constant : Public
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Private, false);
        let a = ComputeKey::<Circuit>::new(Mode::Constant, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("false ? Constant : Public", expected, condition, a, b);

        // false ? Public : Constant
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Private, false);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Constant, second);
        check_ternary("false ? Public : Constant", expected, condition, a, b);

        // true ? Constant : Public
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Private, true);
        let a = ComputeKey::<Circuit>::new(Mode::Constant, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("true ? Constant : Public", expected, condition, a, b);

        // true ? Public : Constant
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Private, true);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Constant, second);
        check_ternary("true ? Public : Constant", expected, condition, a, b);
    }

    #[test]
    fn test_public_condition_and_variable_inputs() {
        let mut rng = TestRng::default();

        let first = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();
        let second = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();

        // false ? Public : Public
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Public, false);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("false ? Public : Public", expected, condition, a, b);

        // false ? Public : Private
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Public, false);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Private, second);
        check_ternary("false ? Public : Private", expected, condition, a, b);

        // false ? Private : Public
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Public, false);
        let a = ComputeKey::<Circuit>::new(Mode::Private, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("false ? Private : Public", expected, condition, a, b);

        // false ? Private : Private
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Public, false);
        let a = ComputeKey::<Circuit>::new(Mode::Private, first);
        let b = ComputeKey::<Circuit>::new(Mode::Private, second);
        check_ternary("false ? Private : Private", expected, condition, a, b);

        // true ? Public : Public
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Public, true);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("true ? Public : Public", expected, condition, a, b);

        // true ? Public : Private
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Public, true);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Private, second);
        check_ternary("true ? Public : Private", expected, condition, a, b);

        // true ? Private : Public
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Public, true);
        let a = ComputeKey::<Circuit>::new(Mode::Private, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("true ? Private : Public", expected, condition, a, b);

        // true ? Private : Private
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Public, true);
        let a = ComputeKey::<Circuit>::new(Mode::Private, first);
        let b = ComputeKey::<Circuit>::new(Mode::Private, second);
        check_ternary("true ? Private : Private", expected, condition, a, b);
    }

    #[test]
    fn test_private_condition_and_variable_inputs() {
        let mut rng = TestRng::default();

        let first = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();
        let second = console::ComputeKey::try_from(console::PrivateKey::new(&mut rng).unwrap()).unwrap();

        // false ? Public : Public
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Private, false);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("false ? Public : Public", expected, condition, a, b);

        // false ? Public : Private
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Private, false);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Private, second);
        check_ternary("false ? Public : Private", expected, condition, a, b);

        // false ? Private : Public
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Private, false);
        let a = ComputeKey::<Circuit>::new(Mode::Private, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("false ? Private : Public", expected, condition, a, b);

        // false ? Private : Private
        let expected = second;
        let condition = Boolean::<Circuit>::new(Mode::Private, false);
        let a = ComputeKey::<Circuit>::new(Mode::Private, first);
        let b = ComputeKey::<Circuit>::new(Mode::Private, second);
        check_ternary("false ? Private : Private", expected, condition, a, b);

        // true ? Public : Public
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Private, true);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("true ? Public : Public", expected, condition, a, b);

        // true ? Public : Private
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Private, true);
        let a = ComputeKey::<Circuit>::new(Mode::Public, first);
        let b = ComputeKey::<Circuit>::new(Mode::Private, second);
        check_ternary("true ? Public : Private", expected, condition, a, b);

        // true ? Private : Public
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Private, true);
        let a = ComputeKey::<Circuit>::new(Mode::Private, first);
        let b = ComputeKey::<Circuit>::new(Mode::Public, second);
        check_ternary("true ? Private : Public", expected, condition, a, b);

        // true ? Private : Private
        let expected = first;
        let condition = Boolean::<Circuit>::new(Mode::Private, true);
        let a = ComputeKey::<Circuit>::new(Mode::Private, first);
        let b = ComputeKey::<Circuit>::new(Mode::Private, second);
        check_ternary("true ? Private : Private", expected, condition, a, b);
    }
}
