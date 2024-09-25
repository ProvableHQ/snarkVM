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

impl<E: Environment> CastLossy<Address<E>> for Scalar<E> {
    /// Casts a `Scalar` to an `Address`.
    ///
    /// This operation converts the scalar into a field element, and then attempts to recover
    /// the group element to construct the address. See the documentation of `Field::cast_lossy`
    /// on the `Group` type for more details.
    #[inline]
    fn cast_lossy(&self) -> Address<E> {
        let field: Field<E> = self.cast_lossy();
        field.cast_lossy()
    }
}

impl<E: Environment> CastLossy<Boolean<E>> for Scalar<E> {
    /// Casts a `Scalar` to a `Boolean`, with lossy truncation.
    /// This operation returns the least significant bit of the field.
    #[inline]
    fn cast_lossy(&self) -> Boolean<E> {
        let bits_le = self.to_bits_le();
        debug_assert!(!bits_le.is_empty(), "An integer must have at least one bit");
        Boolean::new(bits_le[0])
    }
}

impl<E: Environment> CastLossy<Group<E>> for Scalar<E> {
    /// Casts a `Scalar` to a `Group`.
    ///
    /// This operation converts the scalar into a field element, and then attempts to recover
    /// the group element. See the documentation of `Field::cast_lossy` on the `Group` type
    /// for more details.
    #[inline]
    fn cast_lossy(&self) -> Group<E> {
        let field: Field<E> = self.cast_lossy();
        field.cast_lossy()
    }
}

impl<E: Environment> CastLossy<Field<E>> for Scalar<E> {
    /// Casts a `Scalar` to a `Field`.
    /// This operation is **always** lossless.
    #[inline]
    fn cast_lossy(&self) -> Field<E> {
        let result = self.to_field();
        debug_assert!(result.is_ok(), "A scalar should always be able to be converted to a field");
        result.unwrap()
    }
}

impl<E: Environment, I: IntegerType> CastLossy<Integer<E, I>> for Scalar<E> {
    /// Casts a `Scalar` to an `Integer`, with lossy truncation.
    #[inline]
    fn cast_lossy(&self) -> Integer<E, I> {
        // Note: We are reconstituting the integer from the scalar field.
        // This is safe as the number of bits in the integer is less than the scalar field modulus,
        // and thus will always fit within a single scalar field element.
        debug_assert!(I::BITS < Scalar::<E>::size_in_bits() as u64);

        // Truncate the field to the size of the integer domain.
        // Slicing here is safe as the base field is larger than the integer domain.
        let result = Integer::<E, I>::from_bits_le(&self.to_bits_le()[..usize::try_from(I::BITS).unwrap()]);
        debug_assert!(result.is_ok(), "A lossy integer should always be able to be constructed from scalar bits");
        result.unwrap()
    }
}

impl<E: Environment> CastLossy<Scalar<E>> for Scalar<E> {
    /// Casts a `Scalar` to a `Scalar`.
    /// This is an identity cast, so it is **always** lossless.
    #[inline]
    fn cast_lossy(&self) -> Scalar<E> {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 10_000;

    #[test]
    fn test_scalar_to_address() {
        let rng = &mut TestRng::default();

        let scalar = Scalar::<CurrentEnvironment>::one();
        let address: Address<CurrentEnvironment> = scalar.cast_lossy();
        assert_eq!(address, Address::new(Group::generator()));
        assert_eq!(address.to_group(), &Group::generator());

        let scalar = Scalar::<CurrentEnvironment>::zero();
        let address: Address<CurrentEnvironment> = scalar.cast_lossy();
        assert_eq!(address, Address::zero());
        assert_eq!(address.to_group(), &Group::zero());

        for _ in 0..ITERATIONS {
            // Sample a random scalar.
            let scalar = Scalar::<CurrentEnvironment>::rand(rng);
            // Perform the operation.
            let candidate = scalar.cast_lossy();
            // Compare the result against the group element. (This is the most we can do.)
            let expected: Group<CurrentEnvironment> = scalar.cast_lossy();
            assert_eq!(Address::new(expected), candidate);
        }
    }

    #[test]
    fn test_scalar_to_boolean() {
        let rng = &mut TestRng::default();

        let scalar = Scalar::<CurrentEnvironment>::one();
        let boolean: Boolean<CurrentEnvironment> = scalar.cast_lossy();
        assert_eq!(boolean, Boolean::new(true));

        let scalar = Scalar::<CurrentEnvironment>::zero();
        let boolean: Boolean<CurrentEnvironment> = scalar.cast_lossy();
        assert_eq!(boolean, Boolean::new(false));

        for _ in 0..ITERATIONS {
            // Sample a random scalar.
            let scalar = Scalar::<CurrentEnvironment>::rand(rng);
            // Perform the operation.
            let candidate = scalar.cast_lossy();
            // Compare the result against the least significant bit of the scalar.
            let expected = Boolean::new(scalar.to_bits_be().pop().unwrap());
            assert_eq!(expected, candidate);
        }
    }

    #[test]
    fn test_scalar_to_field() {
        let rng = &mut TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random scalar.
            let scalar = Scalar::<CurrentEnvironment>::rand(rng);
            // Perform the operation.
            let candidate = scalar.cast_lossy();
            assert_eq!(scalar.to_field().unwrap(), candidate);
        }
    }

    #[test]
    fn test_scalar_to_group() {
        let rng = &mut TestRng::default();

        let scalar = Scalar::<CurrentEnvironment>::one();
        let group: Group<CurrentEnvironment> = scalar.cast_lossy();
        assert_eq!(group, Group::generator());

        let scalar = Scalar::<CurrentEnvironment>::zero();
        let group: Group<CurrentEnvironment> = scalar.cast_lossy();
        assert_eq!(group, Group::zero());

        for _ in 0..ITERATIONS {
            // Sample a random scalar.
            let scalar = Scalar::<CurrentEnvironment>::rand(rng);
            // Perform the operation.
            let candidate: Group<CurrentEnvironment> = scalar.cast_lossy();
            // Compare the result against the address. (This is the most we can do.)
            let expected: Address<CurrentEnvironment> = scalar.cast_lossy();
            assert_eq!(expected.to_group(), &candidate);
        }
    }

    #[test]
    fn test_scalar_to_scalar() {
        let rng = &mut TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random scalar.
            let scalar = Scalar::<CurrentEnvironment>::rand(rng);
            // Perform the operation.
            let candidate: Scalar<CurrentEnvironment> = scalar.cast_lossy();
            assert_eq!(scalar, candidate);
        }
    }

    macro_rules! check_scalar_to_integer {
        ($type:ty) => {
            let rng = &mut TestRng::default();

            let scalar = Scalar::<CurrentEnvironment>::one();
            let integer: Integer<CurrentEnvironment, $type> = scalar.cast_lossy();
            assert_eq!(integer, Integer::<CurrentEnvironment, $type>::one());

            let scalar = Scalar::<CurrentEnvironment>::zero();
            let integer: Integer<CurrentEnvironment, $type> = scalar.cast_lossy();
            assert_eq!(integer, Integer::<CurrentEnvironment, $type>::zero());

            for _ in 0..ITERATIONS {
                // Sample a random scalar.
                let scalar = Scalar::<CurrentEnvironment>::rand(rng);
                // Perform the operation.
                let candidate: Integer<CurrentEnvironment, $type> = scalar.cast_lossy();
                // Compare the result against the least significant bits of the scalar.
                let expected = Integer::<CurrentEnvironment, $type>::from_bits_le(
                    &scalar.to_bits_le()[..usize::try_from(<$type>::BITS).unwrap()],
                )
                .unwrap();
                assert_eq!(expected, candidate);
            }
        };
    }

    #[test]
    fn test_scalar_to_i8() {
        check_scalar_to_integer!(i8);
    }

    #[test]
    fn test_scalar_to_i16() {
        check_scalar_to_integer!(i16);
    }

    #[test]
    fn test_scalar_to_i32() {
        check_scalar_to_integer!(i32);
    }

    #[test]
    fn test_scalar_to_i64() {
        check_scalar_to_integer!(i64);
    }

    #[test]
    fn test_scalar_to_i128() {
        check_scalar_to_integer!(i128);
    }

    #[test]
    fn test_scalar_to_u8() {
        check_scalar_to_integer!(u8);
    }

    #[test]
    fn test_scalar_to_u16() {
        check_scalar_to_integer!(u16);
    }

    #[test]
    fn test_scalar_to_u32() {
        check_scalar_to_integer!(u32);
    }

    #[test]
    fn test_scalar_to_u64() {
        check_scalar_to_integer!(u64);
    }

    #[test]
    fn test_scalar_to_u128() {
        check_scalar_to_integer!(u128);
    }
}
