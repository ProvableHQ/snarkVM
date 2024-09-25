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
    /// Converts an integer into a scalar.
    pub fn to_scalar(&self) -> Scalar<E> {
        // Note: We are reconstituting the integer as a scalar field.
        // This is safe as the number of bits in the integer is less than the scalar field modulus,
        // and thus will always fit within a single scalar field element.
        debug_assert!(I::BITS < Scalar::<E>::size_in_bits() as u64);

        // Reconstruct the bits as a linear combination representing the original value.
        let result = Scalar::from_bits_le(&self.to_bits_le());
        debug_assert!(result.is_ok(), "An integer should always be able to be constructed from scalar bits");
        result.unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 10_000;

    fn check_to_scalar<I: IntegerType>() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random integer.
            let expected = Integer::<CurrentEnvironment, I>::rand(&mut rng);

            // Perform the operation.
            let candidate = expected.to_scalar();

            // Extract the bits from the scalar field representation.
            let candidate_bits_le = candidate.to_bits_le();
            assert_eq!(Scalar::<CurrentEnvironment>::size_in_bits(), candidate_bits_le.len());

            // Ensure all integer bits match with the expected result.
            let i_bits = usize::try_from(I::BITS).unwrap();
            let expected_bits = expected.to_bits_le();
            for (expected_bit, candidate_bit) in expected_bits.iter().zip_eq(&candidate_bits_le[0..i_bits]) {
                assert_eq!(expected_bit, candidate_bit);
            }

            // Ensure all remaining bits are 0.
            for candidate_bit in &candidate_bits_le[i_bits..] {
                assert!(!candidate_bit);
            }
        }
        Ok(())
    }

    #[test]
    fn test_u8_to_scalar() -> Result<()> {
        type I = u8;
        check_to_scalar::<I>()
    }

    #[test]
    fn test_i8_to_scalar() -> Result<()> {
        type I = i8;
        check_to_scalar::<I>()
    }

    #[test]
    fn test_u16_to_scalar() -> Result<()> {
        type I = u16;
        check_to_scalar::<I>()
    }

    #[test]
    fn test_i16_to_scalar() -> Result<()> {
        type I = i16;
        check_to_scalar::<I>()
    }

    #[test]
    fn test_u32_to_scalar() -> Result<()> {
        type I = u32;
        check_to_scalar::<I>()
    }

    #[test]
    fn test_i32_to_scalar() -> Result<()> {
        type I = i32;
        check_to_scalar::<I>()
    }

    #[test]
    fn test_u64_to_scalar() -> Result<()> {
        type I = u64;
        check_to_scalar::<I>()
    }

    #[test]
    fn test_i64_to_scalar() -> Result<()> {
        type I = i64;
        check_to_scalar::<I>()
    }

    #[test]
    fn test_u128_to_scalar() -> Result<()> {
        type I = u128;
        check_to_scalar::<I>()
    }

    #[test]
    fn test_i128_to_scalar() -> Result<()> {
        type I = i128;
        check_to_scalar::<I>()
    }
}
