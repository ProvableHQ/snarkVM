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

impl<E: Environment> FromField for Scalar<E> {
    type Field = Field<E>;

    /// Casts a scalar from a base field element.
    ///
    /// This method guarantees the following:
    ///   1. If the field element is larger than the scalar field modulus, then the operation will fail.
    ///   2. If the field element is smaller than the scalar field modulus, then the operation will succeed.
    ///     - This is particularly useful for the case where a user called, `Scalar::from_field(scalar.to_field())`,
    ///       and the scalar bit representation is between `size_in_data_bits < bits.len() < size_in_bits`.
    fn from_field(field: Self::Field) -> Self {
        // Note: We are reconstituting the integer from the base field.
        // This is safe as the number of bits in the integer is less than the base field modulus,
        // and thus will always fit within a single base field element.
        debug_assert!(E::ScalarField::size_in_bits() < E::BaseField::size_in_bits());

        // Do not truncate the field bits, which provides the following features:
        //   1. If the field element is larger than the scalar field modulus, then the operation will fail.
        //   2. If the field element is smaller than the scalar field modulus, then the operation will succeed.
        //     - This is particularly useful for the case where a user called, `Scalar::from_field(scalar.to_field())`,
        //       and the scalar bit representation is between `size_in_data_bits < bits.len() < size_in_bits`.
        Scalar::from_bits_le(&field.to_bits_le())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 128;

    fn check_from_field(mode: Mode, rng: &mut TestRng) {
        for i in 0..ITERATIONS {
            // Sample a random scalar.
            let expected = Uniform::rand(rng);
            let candidate = Scalar::<Circuit>::new(mode, expected).to_field();

            Circuit::scope(format!("{mode} {expected} {i}"), || {
                // Perform the operation.
                let candidate = Scalar::<Circuit>::from_field(candidate);
                assert_eq!(expected, candidate.eject_value());
                match mode {
                    Mode::Constant => assert_scope!(253, 0, 0, 0),
                    _ => assert_scope!(0, 0, 755, 759),
                }
            });
            Circuit::reset();

            // Sample a random field.
            let expected = Field::<Circuit>::new(mode, Uniform::rand(rng));
            // Filter for field elements that exceed the scalar field modulus.
            if expected.eject_value()
                > console::ToField::to_field(&-console::Scalar::<<Circuit as Environment>::Network>::one()).unwrap()
            {
                // Perform the operation.
                let result = std::panic::catch_unwind(|| {
                    Scalar::from_field(expected); // This should fail.
                });
                assert!(result.is_err() || !Circuit::is_satisfied());
            } else {
                // Perform the operation.
                Scalar::from_field(expected); // This should not fail.
            }

            Circuit::reset();
        }
    }

    #[test]
    fn test_from_field() {
        let mut rng = TestRng::default();

        check_from_field(Mode::Constant, &mut rng);
        check_from_field(Mode::Public, &mut rng);
        check_from_field(Mode::Private, &mut rng);
    }
}
