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

impl<E: Environment, I: IntegerType> AbsChecked for Integer<E, I> {
    type Output = Integer<E, I>;

    fn abs_checked(self) -> Self::Output {
        (&self).abs_checked()
    }
}

impl<E: Environment, I: IntegerType> AbsChecked for &Integer<E, I> {
    type Output = Integer<E, I>;

    fn abs_checked(self) -> Self::Output {
        match I::is_signed() {
            true => Integer::ternary(self.msb(), &Integer::zero().sub_checked(self), self),
            false => self.clone(),
        }
    }
}

impl<E: Environment, I: IntegerType> Metrics<dyn AbsChecked<Output = Integer<E, I>>> for Integer<E, I> {
    type Case = Mode;

    fn count(case: &Self::Case) -> Count {
        match I::is_signed() {
            true => match case {
                Mode::Constant => Count::is(2 * I::BITS, 0, 0, 0),
                _ => Count::is(I::BITS, 0, (2 * I::BITS) + 3, (2 * I::BITS) + 5),
            },
            false => Count::is(0, 0, 0, 0),
        }
    }
}

impl<E: Environment, I: IntegerType> OutputMode<dyn AbsChecked<Output = Integer<E, I>>> for Integer<E, I> {
    type Case = Mode;

    fn output_mode(case: &Self::Case) -> Mode {
        match I::is_signed() {
            true => match case.is_constant() {
                true => Mode::Constant,
                false => Mode::Private,
            },
            false => *case,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    use test_utilities::*;

    use core::{ops::RangeInclusive, panic::UnwindSafe};

    const ITERATIONS: u64 = 128;

    fn check_abs<I: IntegerType + UnwindSafe>(
        name: &str,
        value: console::Integer<<Circuit as Environment>::Network, I>,
        mode: Mode,
    ) {
        let a = Integer::<Circuit, I>::new(mode, value);
        match value.checked_abs() {
            Some(expected) => Circuit::scope(name, || {
                let candidate = a.abs_checked();
                assert_eq!(expected, *candidate.eject_value());
                assert_eq!(console::Integer::new(expected), candidate.eject_value());
                assert_count!(AbsChecked(Integer<I>) => Integer<I>, &mode);
                assert_output_mode!(AbsChecked(Integer<I>) => Integer<I>, &mode, candidate);
            }),
            None => match mode {
                Mode::Constant => check_unary_operation_halts(a, |a: Integer<Circuit, I>| a.abs_checked()),
                _ => Circuit::scope(name, || {
                    let _candidate = a.abs_checked();
                    assert_count_fails!(AbsChecked(Integer<I>) => Integer<I>, &mode);
                }),
            },
        }
        Circuit::reset();
    }

    fn run_test<I: IntegerType + UnwindSafe>(mode: Mode) {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            let name = format!("Abs: {mode} {i}");
            let value = Uniform::rand(&mut rng);
            check_abs::<I>(&name, value, mode);
        }

        // Check the 0 case.
        let name = format!("Abs: {mode} zero");
        check_abs::<I>(&name, console::Integer::zero(), mode);

        // Check the 1 case.
        let name = format!("Abs: {mode} one");
        check_abs::<I>(&name, console::Integer::one(), mode);

        // Check the console::Integer::MIN (checked) case.
        let name = format!("Abs: {mode} one");
        check_abs::<I>(&name, console::Integer::MIN, mode);
    }

    fn run_exhaustive_test<I: IntegerType + UnwindSafe>(mode: Mode)
    where
        RangeInclusive<I>: Iterator<Item = I>,
    {
        for value in I::MIN..=I::MAX {
            let value = console::Integer::<_, I>::new(value);

            let name = format!("Abs: {mode}");
            check_abs::<I>(&name, value, mode);
        }
    }

    test_integer_unary!(run_test, i8, equals);
    test_integer_unary!(run_test, i16, equals);
    test_integer_unary!(run_test, i32, equals);
    test_integer_unary!(run_test, i64, equals);
    test_integer_unary!(run_test, i128, equals);

    test_integer_unary!(run_test, u8, equals);
    test_integer_unary!(run_test, u16, equals);
    test_integer_unary!(run_test, u32, equals);
    test_integer_unary!(run_test, u64, equals);
    test_integer_unary!(run_test, u128, equals);

    test_integer_unary!(#[ignore], run_exhaustive_test, u8, equals, exhaustive);
    test_integer_unary!(#[ignore], run_exhaustive_test, i8, equals, exhaustive);
}
