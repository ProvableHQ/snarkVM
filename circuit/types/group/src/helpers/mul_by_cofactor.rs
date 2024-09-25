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

impl<E: Environment> Group<E> {
    /// Returns the product of the group element and the cofactor.
    pub fn mul_by_cofactor(&self) -> Group<E> {
        // (For advanced users) The cofactor for this curve is `4`. Thus doubling is used to be performant.
        // See unit tests below, which sanity check that this condition holds.
        debug_assert!(E::Affine::cofactor().len() == 1 && E::Affine::cofactor()[0] == 4);

        self.double().double()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 250;

    fn check_mul_by_cofactor(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected: console::Group<<Circuit as Environment>::Network> = Uniform::rand(&mut rng);

            // Multiply the point by the inverse of the cofactor.
            let input = expected.div_by_cofactor();
            assert_eq!(expected, input.mul_by_cofactor());

            // Initialize the input.
            let affine = Group::<Circuit>::new(mode, input);

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = affine.mul_by_cofactor();
                assert_eq!(expected, candidate.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
    }

    #[test]
    fn test_mul_by_cofactor_constant() {
        check_mul_by_cofactor(Mode::Constant, 6, 0, 0, 0);
    }

    #[test]
    fn test_mul_by_cofactor_public() {
        check_mul_by_cofactor(Mode::Public, 2, 0, 10, 10);
    }

    #[test]
    fn test_mul_by_cofactor_private() {
        check_mul_by_cofactor(Mode::Private, 2, 0, 10, 10);
    }

    /// This test shows that computing `mul_by_cofactor` using doubling is more cost-effective for our specific cofactor.
    #[test]
    fn test_mul_by_cofactor_matches() {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected: console::Group<<Circuit as Environment>::Network> = Uniform::rand(&mut rng);

            // Multiply the point by the inverse of the cofactor.
            let input = expected.div_by_cofactor();
            assert_eq!(expected, input.mul_by_cofactor());

            // Initialize the input.
            let affine = Group::<Circuit>::new(Mode::Private, input);

            Circuit::scope(&format!("Constant {i}"), || {
                let candidate =
                    affine * Scalar::constant(console::Scalar::new(<Circuit as Environment>::ScalarField::from(4u128)));
                assert_eq!(expected, candidate.eject_value());
                assert_scope!(258, 0, 22, 22);
            });
            Circuit::reset();
        }
    }
}
