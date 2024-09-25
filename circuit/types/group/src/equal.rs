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

impl<E: Environment> Equal<Self> for Group<E> {
    type Output = Boolean<E>;

    ///
    /// Returns `true` if `self` and `other` are equal.
    ///
    /// This method costs 8 constraints.
    ///
    fn is_equal(&self, other: &Self) -> Self::Output {
        let is_x_eq = self.x.is_equal(&other.x);
        let is_y_eq = self.y.is_equal(&other.y);
        is_x_eq & is_y_eq
    }

    ///
    /// Returns `true` if `self` and `other` are *not* equal.
    ///
    /// This method constructs a boolean that indicates if
    /// `self` and `other ` are *not* equal to each other.
    ///
    /// This method costs 8 constraints.
    ///
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        !self.is_equal(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 100;

    #[test]
    fn test_is_equal() {
        let mut rng = TestRng::default();

        // Constant == Constant
        for i in 0..ITERATIONS {
            // Sample two random elements.
            let a = Uniform::rand(&mut rng);
            let b = Uniform::rand(&mut rng);

            let a = Group::<Circuit>::new(Mode::Constant, a);
            let b = Group::<Circuit>::new(Mode::Constant, b);

            Circuit::scope(&format!("Constant Equals {i}"), || {
                let equals = a.is_equal(&b);
                assert!(!equals.eject_value());
                assert_scope!(2, 0, 0, 0);
            });
            Circuit::reset();

            Circuit::scope(&format!("Constant Not Equals {i}"), || {
                let equals = a.is_not_equal(&b);
                assert!(equals.eject_value());
                assert_scope!(2, 0, 0, 0);
            });
            Circuit::reset();
        }

        // Constant == Public
        for i in 0..ITERATIONS {
            // Sample two random elements.
            let a = Uniform::rand(&mut rng);
            let b = Uniform::rand(&mut rng);

            let a = Group::<Circuit>::new(Mode::Constant, a);
            let b = Group::<Circuit>::new(Mode::Public, b);

            Circuit::scope(&format!("Constant and Public Equals {i}"), || {
                let equals = a.is_equal(&b);
                assert!(!equals.eject_value());
                assert_scope!(0, 0, 5, 5);
            });
            Circuit::reset();

            Circuit::scope(&format!("Constant and Public Not Equals {i}"), || {
                let equals = a.is_not_equal(&b);
                assert!(equals.eject_value());
                assert_scope!(0, 0, 5, 5);
            });
            Circuit::reset();
        }

        // Public == Constant
        for i in 0..ITERATIONS {
            // Sample two random elements.
            let a = Uniform::rand(&mut rng);
            let b = Uniform::rand(&mut rng);

            let a = Group::<Circuit>::new(Mode::Public, a);
            let b = Group::<Circuit>::new(Mode::Constant, b);

            Circuit::scope(&format!("Public and Constant Equals {i}"), || {
                let equals = a.is_equal(&b);
                assert!(!equals.eject_value());
                assert_scope!(0, 0, 5, 5);
            });
            Circuit::reset();

            Circuit::scope(&format!("Public and Constant Not Equals {i}"), || {
                let equals = a.is_not_equal(&b);
                assert!(equals.eject_value());
                assert_scope!(0, 0, 5, 5);
            });
            Circuit::reset();
        }

        // Public == Public
        for i in 0..ITERATIONS {
            // Sample two random elements.
            let a = Uniform::rand(&mut rng);
            let b = Uniform::rand(&mut rng);

            let a = Group::<Circuit>::new(Mode::Public, a);
            let b = Group::<Circuit>::new(Mode::Public, b);

            Circuit::scope(&format!("Public Equals {i}"), || {
                let equals = a.is_equal(&b);
                assert!(!equals.eject_value());
                assert_scope!(0, 0, 5, 5);
            });
            Circuit::reset();

            Circuit::scope(&format!("Public Not Equals {i}"), || {
                let equals = a.is_not_equal(&b);
                assert!(equals.eject_value());
                assert_scope!(0, 0, 5, 5);
            });
            Circuit::reset();
        }

        // Private == Private
        for i in 0..ITERATIONS {
            // Sample two random elements.
            let a = Uniform::rand(&mut rng);
            let b = Uniform::rand(&mut rng);

            let a = Group::<Circuit>::new(Mode::Private, a);
            let b = Group::<Circuit>::new(Mode::Private, b);

            Circuit::scope(&format!("Private Equals {i}"), || {
                let equals = a.is_equal(&b);
                assert!(!equals.eject_value());
                assert_scope!(0, 0, 5, 5);
            });
            Circuit::reset();

            Circuit::scope(&format!("Private Not Equals {i}"), || {
                let equals = a.is_not_equal(&b);
                assert!(equals.eject_value());
                assert_scope!(0, 0, 5, 5);
            });
            Circuit::reset();
        }
    }
}
