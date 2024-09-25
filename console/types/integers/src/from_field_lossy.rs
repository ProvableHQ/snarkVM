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

impl<E: Environment, I: IntegerType> Integer<E, I> {
    /// Casts an integer from a base field, with lossy truncation.
    ///
    /// This method is commonly-used by hash-to-integer algorithms,
    /// where the hash output does not need to preserve the full base field.
    pub fn from_field_lossy(field: &Field<E>) -> Self {
        // Note: We are reconstituting the integer from the base field.
        // This is safe as the number of bits in the integer is less than the base field modulus,
        // and thus will always fit within a single base field element.
        debug_assert!(I::BITS < Field::<E>::size_in_bits() as u64);

        // Truncate the field to the size of the integer domain.
        // Slicing here is safe as the base field is larger than the integer domain.
        let result = Self::from_bits_le(&field.to_bits_le()[..usize::try_from(I::BITS).unwrap()]);
        debug_assert!(result.is_ok(), "A lossy integer should always be able to be constructed from field bits");
        result.unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 10_000;

    fn check_from_field_lossy<I: IntegerType>() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random integer.
            let expected = Integer::<CurrentEnvironment, I>::rand(&mut rng);

            // Perform the operation.
            let candidate = Integer::from_field_lossy(&expected.to_field()?);
            assert_eq!(expected, candidate);

            // Sample a random field.
            let expected = Field::<CurrentEnvironment>::rand(&mut rng);
            // Perform the operation.
            Integer::<_, I>::from_field_lossy(&expected);
        }
        Ok(())
    }

    #[test]
    fn test_u8_from_field_lossy() -> Result<()> {
        type I = u8;
        check_from_field_lossy::<I>()
    }

    #[test]
    fn test_i8_from_field_lossy() -> Result<()> {
        type I = i8;
        check_from_field_lossy::<I>()
    }

    #[test]
    fn test_u16_from_field_lossy() -> Result<()> {
        type I = u16;
        check_from_field_lossy::<I>()
    }

    #[test]
    fn test_i16_from_field_lossy() -> Result<()> {
        type I = i16;
        check_from_field_lossy::<I>()
    }

    #[test]
    fn test_u32_from_field_lossy() -> Result<()> {
        type I = u32;
        check_from_field_lossy::<I>()
    }

    #[test]
    fn test_i32_from_field_lossy() -> Result<()> {
        type I = i32;
        check_from_field_lossy::<I>()
    }

    #[test]
    fn test_u64_from_field_lossy() -> Result<()> {
        type I = u64;
        check_from_field_lossy::<I>()
    }

    #[test]
    fn test_i64_from_field_lossy() -> Result<()> {
        type I = i64;
        check_from_field_lossy::<I>()
    }

    #[test]
    fn test_u128_from_field_lossy() -> Result<()> {
        type I = u128;
        check_from_field_lossy::<I>()
    }

    #[test]
    fn test_i128_from_field_lossy() -> Result<()> {
        type I = i128;
        check_from_field_lossy::<I>()
    }
}
