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

impl<E: Environment, I: IntegerType> Modulo<Self> for Integer<E, I> {
    type Output = Self;

    #[inline]
    fn modulo(&self, other: &Integer<E, I>) -> Self::Output {
        match I::is_signed() {
            true => E::halt("Attempted to take the modulus of a signed integer."),
            // For unsigned integers, the modulo operation is equivalent to the remainder operation.
            false => self.rem_wrapped(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    use test_utilities::*;

    use core::{ops::RangeInclusive, panic::RefUnwindSafe};

    const ITERATIONS: u64 = 32;

    fn check_modulo<I: IntegerType + RefUnwindSafe>(
        name: &str,
        first: console::Integer<<Circuit as Environment>::Network, I>,
        second: console::Integer<<Circuit as Environment>::Network, I>,
        mode_a: Mode,
        mode_b: Mode,
    ) {
        let a = Integer::<Circuit, I>::new(mode_a, first);
        let b = Integer::<Circuit, I>::new(mode_b, second);

        match I::is_signed() {
            true => check_operation_halts(&a, &b, Integer::modulo),
            false => {
                if second == console::Integer::zero() {
                    match mode_b {
                        Mode::Constant => check_operation_halts(&a, &b, Integer::modulo),
                        _ => Circuit::scope(name, || {
                            let _candidate = a.modulo(&b);
                            assert!(!Circuit::is_satisfied_in_scope(), "(!is_satisfied_in_scope)");
                        }),
                    }
                } else {
                    let expected = (*first).modulo(&second);
                    Circuit::scope(name, || {
                        let candidate = a.modulo(&b);
                        assert_eq!(expected, *candidate.eject_value());
                        assert_eq!(console::Integer::new(expected), candidate.eject_value());
                        assert!(Circuit::is_satisfied_in_scope(), "(is_satisfied_in_scope)");
                    })
                }
            }
        }
        Circuit::reset();
    }

    fn run_test<I: IntegerType + RefUnwindSafe>(mode_a: Mode, mode_b: Mode) {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let first = Uniform::rand(&mut rng);
            let second = Uniform::rand(&mut rng);

            let name = format!("Mod: {first} MOD {second}");
            check_modulo::<I>(&name, first, second, mode_a, mode_b);

            let name = format!("Mod by One: {first} MOD 1");
            check_modulo::<I>(&name, first, console::Integer::one(), mode_a, mode_b);

            let name = format!("Mod by Self: {first} MOD {first}");
            check_modulo::<I>(&name, first, first, mode_a, mode_b);

            let name = format!("Mod by Zero: {first} MOD 0");
            check_modulo::<I>(&name, first, console::Integer::zero(), mode_a, mode_b);
        }

        // Check corner cases.
        check_modulo::<I>("MAX MOD 1", console::Integer::MAX, console::Integer::one(), mode_a, mode_b);
        check_modulo::<I>("MIN MOD 1", console::Integer::MIN, console::Integer::one(), mode_a, mode_b);
        check_modulo::<I>("1 MOD 1", console::Integer::one(), console::Integer::one(), mode_a, mode_b);
        check_modulo::<I>("0 MOD 1", console::Integer::zero(), console::Integer::one(), mode_a, mode_b);
        check_modulo::<I>("MAX MOD 0", console::Integer::MAX, console::Integer::zero(), mode_a, mode_b);
        check_modulo::<I>("MIN MOD 0", console::Integer::MIN, console::Integer::zero(), mode_a, mode_b);
        check_modulo::<I>("1 MOD 0", console::Integer::one(), console::Integer::zero(), mode_a, mode_b);
        check_modulo::<I>("0 MOD 0", console::Integer::zero(), console::Integer::zero(), mode_a, mode_b);

        // Check some additional corner cases for signed integers.
        if I::is_signed() {
            check_modulo::<I>("MAX MOD -1", console::Integer::MAX, -console::Integer::one(), mode_a, mode_b);
            check_modulo::<I>("MIN MOD -1", console::Integer::MIN, -console::Integer::one(), mode_a, mode_b);
            check_modulo::<I>("1 MOD -1", console::Integer::one(), -console::Integer::one(), mode_a, mode_b);
        }
    }

    fn run_exhaustive_test<I: IntegerType + RefUnwindSafe>(mode_a: Mode, mode_b: Mode)
    where
        RangeInclusive<I>: Iterator<Item = I>,
    {
        for first in I::MIN..=I::MAX {
            for second in I::MIN..=I::MAX {
                let first = console::Integer::<_, I>::new(first);
                let second = console::Integer::<_, I>::new(second);

                let name = format!("Mod: ({first} MOD {second})");
                check_modulo::<I>(&name, first, second, mode_a, mode_b);
            }
        }
    }

    test_integer_binary!(run_test, i8, modulo);
    test_integer_binary!(run_test, i16, modulo);
    test_integer_binary!(run_test, i32, modulo);
    test_integer_binary!(run_test, i64, modulo);
    test_integer_binary!(run_test, i128, modulo);

    test_integer_binary!(run_test, u8, modulo);
    test_integer_binary!(run_test, u16, modulo);
    test_integer_binary!(run_test, u32, modulo);
    test_integer_binary!(run_test, u64, modulo);
    test_integer_binary!(run_test, u128, modulo);

    test_integer_binary!(#[ignore], run_exhaustive_test, u8, modulo, exhaustive);
    test_integer_binary!(#[ignore], run_exhaustive_test, i8, modulo, exhaustive);
}
