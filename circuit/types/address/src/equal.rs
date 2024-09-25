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

impl<E: Environment> Equal<Self> for Address<E> {
    type Output = Boolean<E>;

    /// Returns `true` if `self` and `other` are equal.
    fn is_equal(&self, other: &Self) -> Self::Output {
        self.0.is_equal(&other.0)
    }

    /// Returns `true` if `self` and `other` are *not* equal.
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        self.0.is_not_equal(&other.0)
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 100;

    fn check_is_equal(
        mode_a: Mode,
        mode_b: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
        rng: &mut TestRng,
    ) {
        for i in 0..ITERATIONS {
            // Sample two random elements.
            let a = Address::<Circuit>::from_group(Group::new(mode_a, Uniform::rand(rng)));
            let b = Address::<Circuit>::from_group(Group::new(mode_b, Uniform::rand(rng)));

            Circuit::scope(&format!("{mode_a} {mode_a} {i}"), || {
                let equals = a.is_equal(&a);
                assert!(equals.eject_value());
            });

            Circuit::scope(&format!("{mode_a} {mode_b} {i}"), || {
                let equals = a.is_equal(&b);
                assert!(!equals.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
        }
    }

    fn check_is_not_equal(
        mode_a: Mode,
        mode_b: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
        rng: &mut TestRng,
    ) {
        for i in 0..ITERATIONS {
            // Sample two random elements.
            let a = Address::<Circuit>::from_group(Group::new(mode_a, Uniform::rand(rng)));
            let b = Address::<Circuit>::from_group(Group::new(mode_b, Uniform::rand(rng)));

            Circuit::scope(&format!("{mode_a} {mode_a} {i}"), || {
                let equals = a.is_not_equal(&a);
                assert!(!equals.eject_value());
            });

            Circuit::scope(&format!("{mode_a} {mode_b} {i}"), || {
                let equals = a.is_not_equal(&b);
                assert!(equals.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
        }
    }

    #[test]
    fn test_is_equal() {
        let mut rng = TestRng::default();

        check_is_equal(Mode::Constant, Mode::Constant, 2, 0, 0, 0, &mut rng);
        check_is_equal(Mode::Constant, Mode::Public, 0, 0, 5, 5, &mut rng);
        check_is_equal(Mode::Constant, Mode::Private, 0, 0, 5, 5, &mut rng);
        check_is_equal(Mode::Public, Mode::Constant, 0, 0, 5, 5, &mut rng);
        check_is_equal(Mode::Private, Mode::Constant, 0, 0, 5, 5, &mut rng);
        check_is_equal(Mode::Public, Mode::Public, 0, 0, 5, 5, &mut rng);
        check_is_equal(Mode::Public, Mode::Private, 0, 0, 5, 5, &mut rng);
        check_is_equal(Mode::Private, Mode::Public, 0, 0, 5, 5, &mut rng);
        check_is_equal(Mode::Private, Mode::Private, 0, 0, 5, 5, &mut rng);
    }

    #[test]
    fn test_is_not_equal() {
        let mut rng = TestRng::default();

        check_is_not_equal(Mode::Constant, Mode::Constant, 2, 0, 0, 0, &mut rng);
        check_is_not_equal(Mode::Constant, Mode::Public, 0, 0, 5, 5, &mut rng);
        check_is_not_equal(Mode::Constant, Mode::Private, 0, 0, 5, 5, &mut rng);
        check_is_not_equal(Mode::Public, Mode::Constant, 0, 0, 5, 5, &mut rng);
        check_is_not_equal(Mode::Private, Mode::Constant, 0, 0, 5, 5, &mut rng);
        check_is_not_equal(Mode::Public, Mode::Public, 0, 0, 5, 5, &mut rng);
        check_is_not_equal(Mode::Public, Mode::Private, 0, 0, 5, 5, &mut rng);
        check_is_not_equal(Mode::Private, Mode::Public, 0, 0, 5, 5, &mut rng);
        check_is_not_equal(Mode::Private, Mode::Private, 0, 0, 5, 5, &mut rng);
    }
}
