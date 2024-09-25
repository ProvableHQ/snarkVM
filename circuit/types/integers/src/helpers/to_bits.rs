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

impl<E: Environment, I: IntegerType> ToBits for Integer<E, I> {
    type Boolean = Boolean<E>;

    /// Outputs the little-endian bit representation of `self` *with* trailing zeros.
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        (&self).write_bits_le(vec);
    }

    /// Outputs the big-endian bit representation of `self` *with* leading zeros.
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        (&self).write_bits_be(vec);
    }
}

impl<E: Environment, I: IntegerType> ToBits for &Integer<E, I> {
    type Boolean = Boolean<E>;

    /// Outputs the little-endian bit representation of `self` *with* trailing zeros.
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        vec.extend_from_slice(&self.bits_le);
    }

    /// Outputs the big-endian bit representation of `self` *with* leading zeros.
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        let initial_len = vec.len();
        self.write_bits_le(vec);
        vec[initial_len..].reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 128;

    fn check_to_bits_le<I: IntegerType>(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected = Uniform::rand(&mut rng);
            let candidate = Integer::<Circuit, I>::new(mode, expected);

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = candidate.to_bits_le();
                assert_eq!(I::BITS, candidate.len() as u64);

                // Ensure every bit matches.
                let mut expected = *expected;
                for candidate_bit in candidate {
                    assert_eq!(expected & I::one() == I::one(), candidate_bit.eject_value());
                    expected = expected.wrapping_shr(1);
                }
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
        }
    }

    fn check_to_bits_be<I: IntegerType>(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected = Uniform::rand(&mut rng);
            let candidate = Integer::<Circuit, I>::new(mode, expected);

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = candidate.to_bits_be();
                assert_eq!(I::BITS, candidate.len() as u64);

                // Ensure every bit matches.
                let mut expected = *expected;
                for candidate_bit in candidate.iter().rev() {
                    assert_eq!(expected & I::one() == I::one(), candidate_bit.eject_value());
                    expected = expected.wrapping_shr(1);
                }
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
        }
    }

    /// Checks that the field element, when converted to little-endian bits, is well-formed.
    fn check_individual_bits_le<I: IntegerType>(candidate: Integer<Circuit, I>) {
        for (i, bit) in candidate.to_bits_le().iter().enumerate() {
            match i == 0 {
                true => assert!(bit.eject_value()),
                false => assert!(!bit.eject_value()),
            }
        }
    }

    /// Checks that the field element, when converted to big-endian bits, is well-formed.
    fn check_individual_bits_be<I: IntegerType>(candidate: Integer<Circuit, I>) {
        for (i, bit) in candidate.to_bits_be().iter().rev().enumerate() {
            match i == 0 {
                true => assert!(bit.eject_value()),
                false => assert!(!bit.eject_value()),
            }
        }
    }

    fn test_individual_bits<I: IntegerType>(value: console::Integer<<Circuit as Environment>::Network, I>) {
        // Constant
        check_individual_bits_le(Integer::<Circuit, I>::new(Mode::Constant, value));
        check_individual_bits_be(Integer::<Circuit, I>::new(Mode::Constant, value));
        // Public
        check_individual_bits_le(Integer::<Circuit, I>::new(Mode::Public, value));
        check_individual_bits_be(Integer::<Circuit, I>::new(Mode::Public, value));
        // Private
        check_individual_bits_le(Integer::<Circuit, I>::new(Mode::Private, value));
        check_individual_bits_be(Integer::<Circuit, I>::new(Mode::Private, value));
    }

    // Tests for u8.

    #[test]
    fn test_u8_to_bits_le_constant() {
        type I = u8;
        check_to_bits_le::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_u8_to_bits_le_public() {
        type I = u8;
        check_to_bits_le::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_u8_to_bits_le_private() {
        type I = u8;
        check_to_bits_le::<I>(Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_u8_to_bits_be_constant() {
        type I = u8;
        check_to_bits_be::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_u8_to_bits_be_public() {
        type I = u8;
        check_to_bits_be::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_u8_to_bits_be_private() {
        type I = u8;
        check_to_bits_be::<I>(Mode::Private, 0, 0, 0, 0);
    }

    // Tests for i8.

    #[test]
    fn test_i8_to_bits_le_constant() {
        type I = i8;
        check_to_bits_le::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_i8_to_bits_le_public() {
        type I = i8;
        check_to_bits_le::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_i8_to_bits_le_private() {
        type I = i8;
        check_to_bits_le::<I>(Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_i8_to_bits_be_constant() {
        type I = i8;
        check_to_bits_be::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_i8_to_bits_be_public() {
        type I = i8;
        check_to_bits_be::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_i8_to_bits_be_private() {
        type I = i8;
        check_to_bits_be::<I>(Mode::Private, 0, 0, 0, 0);
    }

    // Tests for u16.

    #[test]
    fn test_u16_to_bits_le_constant() {
        type I = u16;
        check_to_bits_le::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_u16_to_bits_le_public() {
        type I = u16;
        check_to_bits_le::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_u16_to_bits_le_private() {
        type I = u16;
        check_to_bits_le::<I>(Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_u16_to_bits_be_constant() {
        type I = u16;
        check_to_bits_be::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_u16_to_bits_be_public() {
        type I = u16;
        check_to_bits_be::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_u16_to_bits_be_private() {
        type I = u16;
        check_to_bits_be::<I>(Mode::Private, 0, 0, 0, 0);
    }

    // Tests for i16.

    #[test]
    fn test_i16_to_bits_le_constant() {
        type I = i16;
        check_to_bits_le::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_i16_to_bits_le_public() {
        type I = i16;
        check_to_bits_le::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_i16_to_bits_le_private() {
        type I = i16;
        check_to_bits_le::<I>(Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_i16_to_bits_be_constant() {
        type I = i16;
        check_to_bits_be::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_i16_to_bits_be_public() {
        type I = i16;
        check_to_bits_be::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_i16_to_bits_be_private() {
        type I = i16;
        check_to_bits_be::<I>(Mode::Private, 0, 0, 0, 0);
    }

    // Tests for u32.

    #[test]
    fn test_u32_to_bits_le_constant() {
        type I = u32;
        check_to_bits_le::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_u32_to_bits_le_public() {
        type I = u32;
        check_to_bits_le::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_u32_to_bits_le_private() {
        type I = u32;
        check_to_bits_le::<I>(Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_u32_to_bits_be_constant() {
        type I = u32;
        check_to_bits_be::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_u32_to_bits_be_public() {
        type I = u32;
        check_to_bits_be::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_u32_to_bits_be_private() {
        type I = u32;
        check_to_bits_be::<I>(Mode::Private, 0, 0, 0, 0);
    }

    // Tests for i32.

    #[test]
    fn test_i32_to_bits_le_constant() {
        type I = i32;
        check_to_bits_le::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_i32_to_bits_le_public() {
        type I = i32;
        check_to_bits_le::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_i32_to_bits_le_private() {
        type I = i32;
        check_to_bits_le::<I>(Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_i32_to_bits_be_constant() {
        type I = i32;
        check_to_bits_be::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_i32_to_bits_be_public() {
        type I = i32;
        check_to_bits_be::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_i32_to_bits_be_private() {
        type I = i32;
        check_to_bits_be::<I>(Mode::Private, 0, 0, 0, 0);
    }

    // Tests for u64.

    #[test]
    fn test_u64_to_bits_le_constant() {
        type I = u64;
        check_to_bits_le::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_u64_to_bits_le_public() {
        type I = u64;
        check_to_bits_le::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_u64_to_bits_le_private() {
        type I = u64;
        check_to_bits_le::<I>(Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_u64_to_bits_be_constant() {
        type I = u64;
        check_to_bits_be::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_u64_to_bits_be_public() {
        type I = u64;
        check_to_bits_be::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_u64_to_bits_be_private() {
        type I = u64;
        check_to_bits_be::<I>(Mode::Private, 0, 0, 0, 0);
    }

    // Tests for i64.

    #[test]
    fn test_i64_to_bits_le_constant() {
        type I = i64;
        check_to_bits_le::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_i64_to_bits_le_public() {
        type I = i64;
        check_to_bits_le::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_i64_to_bits_le_private() {
        type I = i64;
        check_to_bits_le::<I>(Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_i64_to_bits_be_constant() {
        type I = i64;
        check_to_bits_be::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_i64_to_bits_be_public() {
        type I = i64;
        check_to_bits_be::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_i64_to_bits_be_private() {
        type I = i64;
        check_to_bits_be::<I>(Mode::Private, 0, 0, 0, 0);
    }

    // Tests for u128.

    #[test]
    fn test_u128_to_bits_le_constant() {
        type I = u128;
        check_to_bits_le::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_u128_to_bits_le_public() {
        type I = u128;
        check_to_bits_le::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_u128_to_bits_le_private() {
        type I = u128;
        check_to_bits_le::<I>(Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_u128_to_bits_be_constant() {
        type I = u128;
        check_to_bits_be::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_u128_to_bits_be_public() {
        type I = u128;
        check_to_bits_be::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_u128_to_bits_be_private() {
        type I = u128;
        check_to_bits_be::<I>(Mode::Private, 0, 0, 0, 0);
    }

    // Tests for i128.

    #[test]
    fn test_i128_to_bits_le_constant() {
        type I = i128;
        check_to_bits_le::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_i128_to_bits_le_public() {
        type I = i128;
        check_to_bits_le::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_i128_to_bits_le_private() {
        type I = i128;
        check_to_bits_le::<I>(Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_i128_to_bits_be_constant() {
        type I = i128;
        check_to_bits_be::<I>(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_i128_to_bits_be_public() {
        type I = i128;
        check_to_bits_be::<I>(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_i128_to_bits_be_private() {
        type I = i128;
        check_to_bits_be::<I>(Mode::Private, 0, 0, 0, 0);
    }

    #[test]
    fn test_u8_one() {
        type I = u8;
        test_individual_bits::<I>(console::Integer::one());
    }

    #[test]
    fn test_i8_one() {
        type I = i8;
        test_individual_bits::<I>(console::Integer::one());
    }

    #[test]
    fn test_u16_one() {
        type I = u16;
        test_individual_bits::<I>(console::Integer::one());
    }

    #[test]
    fn test_i16_one() {
        type I = i16;
        test_individual_bits::<I>(console::Integer::one());
    }

    #[test]
    fn test_u32_one() {
        type I = u32;
        test_individual_bits::<I>(console::Integer::one());
    }

    #[test]
    fn test_i32_one() {
        type I = i32;
        test_individual_bits::<I>(console::Integer::one());
    }

    #[test]
    fn test_u64_one() {
        type I = u64;
        test_individual_bits::<I>(console::Integer::one());
    }

    #[test]
    fn test_i64_one() {
        type I = i64;
        test_individual_bits::<I>(console::Integer::one());
    }

    #[test]
    fn test_u128_one() {
        type I = u128;
        test_individual_bits::<I>(console::Integer::one());
    }

    #[test]
    fn test_i128_one() {
        type I = i128;
        test_individual_bits::<I>(console::Integer::one());
    }
}
