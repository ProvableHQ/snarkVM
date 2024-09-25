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

impl<E: Environment> Scalar<E> {
    /// Casts a scalar from a base field, with lossy truncation.
    ///
    /// This method is commonly-used by hash-to-scalar algorithms,
    /// where the hash output does not need to preserve the full base field.
    pub fn from_field_lossy(field: &Field<E>) -> Self {
        // Note: We are reconstituting the integer from the base field.
        // This is safe as the number of bits in the integer is less than the base field modulus,
        // and thus will always fit within a single base field element.
        debug_assert!(E::ScalarField::size_in_bits() < E::BaseField::size_in_bits());

        // Truncate the output to the size in data bits (1 bit less than the MODULUS) of the scalar.
        // Slicing here is safe as the base field is larger than the scalar field.
        Scalar::from_bits_le(&field.to_bits_le()[..E::ScalarField::size_in_data_bits()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 128;

    fn check_from_field_lossy(mode: Mode, rng: &mut TestRng) {
        for i in 0..ITERATIONS {
            // Sample a random scalar.
            let size_in_data_bits = console::Scalar::<<Circuit as Environment>::Network>::size_in_data_bits();
            let prepare = console::Scalar::<<Circuit as Environment>::Network>::rand(rng);
            let expected = console::Scalar::from_bits_le(&prepare.to_bits_le()[..size_in_data_bits]).unwrap();
            let candidate = Scalar::<Circuit>::new(mode, expected).to_field();

            Circuit::scope(format!("{mode} {expected} {i}"), || {
                // Perform the operation.
                let candidate = Scalar::<Circuit>::from_field_lossy(&candidate);
                assert_eq!(expected, candidate.eject_value());
                match mode {
                    Mode::Constant => assert_scope!(253, 0, 0, 0),
                    _ => assert_scope!(0, 0, 505, 507),
                }
            });
            Circuit::reset();

            // Sample a random field element.
            let expected = Field::<Circuit>::new(mode, Uniform::rand(rng));
            // Perform the operation.
            Scalar::from_field_lossy(&expected); // This should not fail.

            Circuit::reset();
        }
    }

    #[test]
    fn test_from_field_lossy() {
        let mut rng = TestRng::default();

        check_from_field_lossy(Mode::Constant, &mut rng);
        check_from_field_lossy(Mode::Public, &mut rng);
        check_from_field_lossy(Mode::Private, &mut rng);
    }
}
