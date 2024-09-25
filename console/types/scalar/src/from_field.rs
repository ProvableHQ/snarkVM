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
    fn from_field(field: &Self::Field) -> Result<Self> {
        // Note: We are reconstituting the base field into a scalar field.
        // This is safe as the scalar field modulus is less than the base field modulus,
        // and thus will always fit within a single base field element.
        debug_assert!(Scalar::<E>::size_in_bits() < Field::<E>::size_in_bits());

        // Do not truncate the field bits, which provides the following features:
        //   1. If the field element is larger than the scalar field modulus, then the operation will fail.
        //   2. If the field element is smaller than the scalar field modulus, then the operation will succeed.
        //     - This is particularly useful for the case where a user called, `Scalar::from_field(scalar.to_field())`,
        //       and the scalar bit representation is between `size_in_data_bits < bits.len() < size_in_bits`.
        Self::from_bits_le(&field.to_bits_le())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 10_000;

    #[test]
    fn test_from_field() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random scalar.
            let expected = Scalar::<CurrentEnvironment>::rand(&mut rng);
            // Perform the operation.
            let candidate = Scalar::from_field(&expected.to_field()?)?;
            assert_eq!(expected, candidate);

            // Sample a random field.
            let expected = Field::<CurrentEnvironment>::rand(&mut rng);
            // Filter for field elements that exceed the scalar field modulus.
            if expected > (-Scalar::<CurrentEnvironment>::one()).to_field()? {
                // Perform the operation.
                assert!(Scalar::from_field(&expected).is_err());
            } else {
                // Perform the operation.
                assert!(Scalar::from_field(&expected).is_ok());
            }
        }
        Ok(())
    }
}
