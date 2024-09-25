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

impl<E: Environment, I: IntegerType> CastLossy<Address<E>> for Integer<E, I> {
    /// Casts an `Integer` to an `Address`.
    ///
    /// This operation converts the integer into a field element, and then attempts to recover
    /// the group element to construct the address. See the documentation of `Field::cast_lossy`
    /// on the `Group` type for more details.
    #[inline]
    fn cast_lossy(&self) -> Address<E> {
        let field: Field<E> = self.cast_lossy();
        field.cast_lossy()
    }
}

impl<E: Environment, I: IntegerType> CastLossy<Boolean<E>> for Integer<E, I> {
    /// Casts an `Integer` to a `Boolean`, with lossy truncation.
    /// This operation returns the least significant bit of the field.
    #[inline]
    fn cast_lossy(&self) -> Boolean<E> {
        let bits_le = self.to_bits_le();
        debug_assert!(!bits_le.is_empty(), "An integer must have at least one bit");
        Boolean::new(bits_le[0])
    }
}

impl<E: Environment, I: IntegerType> CastLossy<Field<E>> for Integer<E, I> {
    /// Casts an `Integer` to a `Field`.
    /// This is safe because casting from an integer to a field is **always** lossless.
    #[inline]
    fn cast_lossy(&self) -> Field<E> {
        let result = self.to_field();
        debug_assert!(result.is_ok(), "Casting an integer to field cannot fail");
        result.unwrap()
    }
}

impl<E: Environment, I: IntegerType> CastLossy<Group<E>> for Integer<E, I> {
    /// Casts an `Integer` to a `Group`.
    ///
    /// This operation converts the integer into a field element, and then attempts to recover
    /// the group element. See the documentation of `Field::cast_lossy` on the `Group` type
    /// for more details.
    #[inline]
    fn cast_lossy(&self) -> Group<E> {
        let field: Field<E> = self.cast_lossy();
        field.cast_lossy()
    }
}

impl<E: Environment, I0: IntegerType + AsPrimitive<I1>, I1: IntegerType> CastLossy<Integer<E, I1>> for Integer<E, I0> {
    /// Casts an `Integer` to an `Integer` of a different type, with lossy truncation.
    #[inline]
    fn cast_lossy(&self) -> Integer<E, I1> {
        Integer::new((**self).as_())
    }
}

impl<E: Environment, I: IntegerType> CastLossy<Scalar<E>> for Integer<E, I> {
    /// Casts an `Integer` to a `Scalar`.
    /// This is safe because casting from an integer to a scalar is **always** lossless.
    #[inline]
    fn cast_lossy(&self) -> Scalar<E> {
        self.to_scalar()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type CurrentEnvironment = snarkvm_console_network::Console;

    const ITERATIONS: u64 = 1_000;

    #[test]
    fn test_integer_to_address() {
        macro_rules! check_integer_to_address {
            ($type:ty) => {
                let rng = &mut TestRng::default();

                let integer = Integer::<CurrentEnvironment, $type>::one();
                let address: Address<CurrentEnvironment> = integer.cast_lossy();
                assert_eq!(address, Address::new(Group::generator()));
                assert_eq!(address.to_group(), &Group::generator());

                let integer = Integer::<CurrentEnvironment, $type>::zero();
                let address: Address<CurrentEnvironment> = integer.cast_lossy();
                assert_eq!(address, Address::zero());
                assert_eq!(address.to_group(), &Group::zero());

                for _ in 0..ITERATIONS {
                    // Sample a random integer.
                    let integer = Integer::<CurrentEnvironment, $type>::rand(rng);
                    // Perform the operation.
                    let candidate: Address<CurrentEnvironment> = integer.cast_lossy();
                    // Compare the result against the group element. (This is the most we can do.)
                    let expected: Group<CurrentEnvironment> = integer.cast_lossy();
                    assert_eq!(Address::new(expected), candidate);
                }
            };
        }

        check_integer_to_address!(i8);
        check_integer_to_address!(i16);
        check_integer_to_address!(i32);
        check_integer_to_address!(i64);
        check_integer_to_address!(i128);
        check_integer_to_address!(u8);
        check_integer_to_address!(u16);
        check_integer_to_address!(u32);
        check_integer_to_address!(u64);
        check_integer_to_address!(u128);
    }

    #[test]
    fn test_integer_to_boolean() {
        macro_rules! check_integer_to_boolean {
            ($type:ty) => {
                let rng = &mut TestRng::default();

                let integer = Integer::<CurrentEnvironment, $type>::one();
                let boolean: Boolean<CurrentEnvironment> = integer.cast_lossy();
                assert_eq!(boolean, Boolean::new(true));

                let integer = Integer::<CurrentEnvironment, $type>::zero();
                let boolean: Boolean<CurrentEnvironment> = integer.cast_lossy();
                assert_eq!(boolean, Boolean::new(false));

                for _ in 0..ITERATIONS {
                    // Sample a random integer.
                    let integer = Integer::<CurrentEnvironment, $type>::rand(rng);
                    // Perform the operation.
                    let candidate: Boolean<CurrentEnvironment> = integer.cast_lossy();
                    // Compare the result against the least significant bit of the integer.
                    let expected = Boolean::new(integer.to_bits_be().pop().unwrap());
                    assert_eq!(expected, candidate);
                }
            };
        }

        check_integer_to_boolean!(i8);
        check_integer_to_boolean!(i16);
        check_integer_to_boolean!(i32);
        check_integer_to_boolean!(i64);
        check_integer_to_boolean!(i128);
        check_integer_to_boolean!(u8);
        check_integer_to_boolean!(u16);
        check_integer_to_boolean!(u32);
        check_integer_to_boolean!(u64);
        check_integer_to_boolean!(u128);
    }

    #[test]
    fn test_integer_to_field() {
        macro_rules! check_integer_to_field {
            ($type:ty) => {
                let rng = &mut TestRng::default();

                for _ in 0..ITERATIONS {
                    // Sample a random integer.
                    let integer = Integer::<CurrentEnvironment, $type>::rand(rng);
                    // Perform the operation.
                    let candidate: Field<CurrentEnvironment> = integer.cast_lossy();
                    // Compare the result against the field representation of the integer.
                    let expected = integer.to_field().unwrap();
                    assert_eq!(expected, candidate);
                }
            };
        }

        check_integer_to_field!(i8);
        check_integer_to_field!(i16);
        check_integer_to_field!(i32);
        check_integer_to_field!(i64);
        check_integer_to_field!(i128);
        check_integer_to_field!(u8);
        check_integer_to_field!(u16);
        check_integer_to_field!(u32);
        check_integer_to_field!(u64);
        check_integer_to_field!(u128);
    }

    #[test]
    fn test_integer_to_group() {
        macro_rules! check_integer_to_group {
            ($type:ty) => {
                let rng = &mut TestRng::default();

                let integer = Integer::<CurrentEnvironment, $type>::one();
                let group: Group<CurrentEnvironment> = integer.cast_lossy();
                assert_eq!(group, Group::generator());

                let integer = Integer::<CurrentEnvironment, $type>::zero();
                let group: Group<CurrentEnvironment> = integer.cast_lossy();
                assert_eq!(group, Group::zero());

                for _ in 0..ITERATIONS {
                    // Sample a random integer.
                    let integer = Integer::<CurrentEnvironment, $type>::rand(rng);
                    // Perform the operation.
                    let candidate: Group<CurrentEnvironment> = integer.cast_lossy();
                    // Compare the result against the group representation of the integer.
                    let expected: Group<CurrentEnvironment> = integer.to_field().unwrap().cast_lossy();
                    assert_eq!(expected, candidate);
                }
            };
        }

        check_integer_to_group!(i8);
        check_integer_to_group!(i16);
        check_integer_to_group!(i32);
        check_integer_to_group!(i64);
        check_integer_to_group!(i128);
        check_integer_to_group!(u8);
        check_integer_to_group!(u16);
        check_integer_to_group!(u32);
        check_integer_to_group!(u64);
        check_integer_to_group!(u128);
    }

    #[test]
    fn test_integer_to_scalar() {
        macro_rules! check_integer_to_scalar {
            ($type:ty) => {
                let rng = &mut TestRng::default();

                for _ in 0..ITERATIONS {
                    // Sample a random integer.
                    let integer = Integer::<CurrentEnvironment, $type>::rand(rng);
                    // Perform the operation.
                    let candidate: Scalar<CurrentEnvironment> = integer.cast_lossy();
                    // Compare the result against the scalar representation of the integer.
                    let expected = integer.to_scalar();
                    assert_eq!(expected, candidate);
                }
            };
        }

        check_integer_to_scalar!(i8);
        check_integer_to_scalar!(i16);
        check_integer_to_scalar!(i32);
        check_integer_to_scalar!(i64);
        check_integer_to_scalar!(i128);
        check_integer_to_scalar!(u8);
        check_integer_to_scalar!(u16);
        check_integer_to_scalar!(u32);
        check_integer_to_scalar!(u64);
        check_integer_to_scalar!(u128);
    }

    #[test]
    fn test_integer_to_integer() {
        macro_rules! check_integer_to_integer {
            ($type_a:ty, $type_b:ty) => {
                let rng = &mut TestRng::default();

                println!("Checking {} -> {}", stringify!($type_a), stringify!($type_b));

                for _ in 0..ITERATIONS {
                    // Sample a random integer.
                    let integer = Integer::<CurrentEnvironment, $type_a>::rand(rng);
                    // Perform the operation.
                    let candidate: Integer<CurrentEnvironment, $type_b> = integer.cast_lossy();

                    // Retrieve the lesser number of bits of the two types.
                    let data_bits = std::cmp::min(<$type_a>::BITS, <$type_b>::BITS) as usize;
                    // Compare the data bits of the candidate against the data bits of the integer.
                    for (expected_bit, candidate_bit) in
                        integer.to_bits_le()[0..data_bits].iter().zip_eq(&candidate.to_bits_le()[0..data_bits])
                    {
                        assert_eq!(
                            expected_bit, candidate_bit,
                            "Data bits do not match - ({:b} != {:b})",
                            *integer, *candidate
                        );
                    }
                    // Ensure the remaining bits are 0 (unsigned) or MSB clones (signed).
                    for candidate_bit in &candidate.to_bits_le()[data_bits..] {
                        let expected_bit = match <$type_a>::is_signed() {
                            true => integer.to_bits_le()[data_bits - 1],
                            false => false,
                        };
                        assert_eq!(
                            expected_bit, *candidate_bit,
                            "Remaining bits are not correct - ({:b} != {:b})",
                            *integer, *candidate
                        );
                    }
                }
            };
        }
        {
            check_integer_to_integer!(i8, i8);
            check_integer_to_integer!(i8, i16);
            check_integer_to_integer!(i8, i32);
            check_integer_to_integer!(i8, i64);
            check_integer_to_integer!(i8, i128);
            check_integer_to_integer!(i8, u8);
            check_integer_to_integer!(i8, u16);
            check_integer_to_integer!(i8, u32);
            check_integer_to_integer!(i8, u64);
            check_integer_to_integer!(i8, u128);
        }
        {
            check_integer_to_integer!(i16, i8);
            check_integer_to_integer!(i16, i16);
            check_integer_to_integer!(i16, i32);
            check_integer_to_integer!(i16, i64);
            check_integer_to_integer!(i16, i128);
            check_integer_to_integer!(i16, u8);
            check_integer_to_integer!(i16, u16);
            check_integer_to_integer!(i16, u32);
            check_integer_to_integer!(i16, u64);
            check_integer_to_integer!(i16, u128);
        }
        {
            check_integer_to_integer!(i32, i8);
            check_integer_to_integer!(i32, i16);
            check_integer_to_integer!(i32, i32);
            check_integer_to_integer!(i32, i64);
            check_integer_to_integer!(i32, i128);
            check_integer_to_integer!(i32, u8);
            check_integer_to_integer!(i32, u16);
            check_integer_to_integer!(i32, u32);
            check_integer_to_integer!(i32, u64);
            check_integer_to_integer!(i32, u128);
        }
        {
            check_integer_to_integer!(i64, i8);
            check_integer_to_integer!(i64, i16);
            check_integer_to_integer!(i64, i32);
            check_integer_to_integer!(i64, i64);
            check_integer_to_integer!(i64, i128);
            check_integer_to_integer!(i64, u8);
            check_integer_to_integer!(i64, u16);
            check_integer_to_integer!(i64, u32);
            check_integer_to_integer!(i64, u64);
            check_integer_to_integer!(i64, u128);
        }
        {
            check_integer_to_integer!(i128, i8);
            check_integer_to_integer!(i128, i16);
            check_integer_to_integer!(i128, i32);
            check_integer_to_integer!(i128, i64);
            check_integer_to_integer!(i128, i128);
            check_integer_to_integer!(i128, u8);
            check_integer_to_integer!(i128, u16);
            check_integer_to_integer!(i128, u32);
            check_integer_to_integer!(i128, u64);
            check_integer_to_integer!(i128, u128);
        }
        {
            check_integer_to_integer!(u8, i8);
            check_integer_to_integer!(u8, i16);
            check_integer_to_integer!(u8, i32);
            check_integer_to_integer!(u8, i64);
            check_integer_to_integer!(u8, i128);
            check_integer_to_integer!(u8, u8);
            check_integer_to_integer!(u8, u16);
            check_integer_to_integer!(u8, u32);
            check_integer_to_integer!(u8, u64);
            check_integer_to_integer!(u8, u128);
        }
        {
            check_integer_to_integer!(u16, i8);
            check_integer_to_integer!(u16, i16);
            check_integer_to_integer!(u16, i32);
            check_integer_to_integer!(u16, i64);
            check_integer_to_integer!(u16, i128);
            check_integer_to_integer!(u16, u8);
            check_integer_to_integer!(u16, u16);
            check_integer_to_integer!(u16, u32);
            check_integer_to_integer!(u16, u64);
            check_integer_to_integer!(u16, u128);
        }
        {
            check_integer_to_integer!(u32, i8);
            check_integer_to_integer!(u32, i16);
            check_integer_to_integer!(u32, i32);
            check_integer_to_integer!(u32, i64);
            check_integer_to_integer!(u32, i128);
            check_integer_to_integer!(u32, u8);
            check_integer_to_integer!(u32, u16);
            check_integer_to_integer!(u32, u32);
            check_integer_to_integer!(u32, u64);
            check_integer_to_integer!(u32, u128);
        }
        {
            check_integer_to_integer!(u64, i8);
            check_integer_to_integer!(u64, i16);
            check_integer_to_integer!(u64, i32);
            check_integer_to_integer!(u64, i64);
            check_integer_to_integer!(u64, i128);
            check_integer_to_integer!(u64, u8);
            check_integer_to_integer!(u64, u16);
            check_integer_to_integer!(u64, u32);
            check_integer_to_integer!(u64, u64);
            check_integer_to_integer!(u64, u128);
        }
    }
}
