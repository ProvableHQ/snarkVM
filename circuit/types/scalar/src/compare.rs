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

impl<E: Environment> Compare<Scalar<E>> for Scalar<E> {
    type Output = Boolean<E>;

    /// Returns `true` if `self` is less than `other`.
    fn is_less_than(&self, other: &Self) -> Self::Output {
        // Case 1: Constant < Constant
        if self.is_constant() && other.is_constant() {
            Boolean::new(Mode::Constant, self.eject_value() < other.eject_value())
        }
        // Case 2: Constant < Variable | Variable < Constant | Variable < Variable
        else {
            // If all scalar field elements are less than or equal to (MODULUS - 1)/2 on the base field,
            // we can perform an optimized check for `is_less_than` by casting the scalars onto the base field.
            debug_assert!((-E::ScalarField::one()).to_bigint() <= E::BaseField::modulus_minus_one_div_two());

            // Intuition: Check the parity of 2 * (`self` - `other`) mod MODULUS.
            //   - If `self` < `other`, then 2 * (`self` - `other`) mod MODULUS is odd.
            //   - If `self` >= `other`, then 2 * (`self` - `other`) mod MODULUS is even.

            // Compute 2 * (`self` - `other`).
            let outcome = (self.to_field() - other.to_field()).double();
            // Retrieve the LSB from the computation to determine even / odd parity.
            outcome.to_bits_be().pop().unwrap_or_else(|| E::halt("Failed to retrieve the LSB from the field element."))
        }
    }

    /// Returns `true` if `self` is greater than `other`.
    fn is_greater_than(&self, other: &Self) -> Self::Output {
        other.is_less_than(self)
    }

    /// Returns `true` if `self` is less than or equal to `other`.
    fn is_less_than_or_equal(&self, other: &Self) -> Self::Output {
        other.is_greater_than_or_equal(self)
    }

    /// Returns `true` if `self` is greater than or equal to `other`.
    fn is_greater_than_or_equal(&self, other: &Self) -> Self::Output {
        !self.is_less_than(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 100;

    fn check_is_less_than(
        mode_a: Mode,
        mode_b: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element `a`.
            let expected_a = Uniform::rand(&mut rng);
            let candidate_a = Scalar::<Circuit>::new(mode_a, expected_a);

            // Sample a random element `b`.
            let expected_b = Uniform::rand(&mut rng);
            let candidate_b = Scalar::<Circuit>::new(mode_b, expected_b);

            // Perform the less than comparison.
            Circuit::scope(&format!("{mode_a} {mode_b} {i}"), || {
                let candidate = candidate_a.is_less_than(&candidate_b);
                assert_eq!(expected_a < expected_b, candidate.eject_value());
                assert_scope!(<=num_constants, <=num_public, <=num_private, <=num_constraints);
            });
            Circuit::reset();
        }
    }

    #[test]
    fn test_constant_is_less_than_constant() {
        check_is_less_than(Mode::Constant, Mode::Constant, 1, 0, 0, 0);
    }

    #[test]
    fn test_constant_is_less_than_public() {
        check_is_less_than(Mode::Constant, Mode::Public, 0, 0, 505, 507);
    }

    #[test]
    fn test_constant_is_less_than_private() {
        check_is_less_than(Mode::Constant, Mode::Private, 0, 0, 505, 507);
    }

    #[test]
    fn test_public_is_less_than_constant() {
        check_is_less_than(Mode::Public, Mode::Constant, 0, 0, 505, 507);
    }

    #[test]
    fn test_public_is_less_than_public() {
        check_is_less_than(Mode::Public, Mode::Public, 0, 0, 505, 507);
    }

    #[test]
    fn test_public_is_less_than_private() {
        check_is_less_than(Mode::Public, Mode::Private, 0, 0, 505, 507);
    }

    #[test]
    fn test_private_is_less_than_constant() {
        check_is_less_than(Mode::Private, Mode::Constant, 0, 0, 505, 507);
    }

    #[test]
    fn test_private_is_less_than_public() {
        check_is_less_than(Mode::Private, Mode::Public, 0, 0, 505, 507);
    }

    #[test]
    fn test_private_is_less_than_private() {
        check_is_less_than(Mode::Private, Mode::Private, 0, 0, 505, 507);
    }
}
