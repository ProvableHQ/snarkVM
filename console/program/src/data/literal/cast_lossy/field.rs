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

impl<E: Environment> CastLossy<Address<E>> for Field<E> {
    /// Casts a `Field` to an `Address`.
    ///
    /// This operation attempts to recover the group element from the given field,
    /// which is then used to construct the address. See the documentation of `Field::cast_lossy`
    /// on the `Group` type for more details.
    #[inline]
    fn cast_lossy(&self) -> Address<E> {
        // Perform a lossy cast to a group element.
        let group: Group<E> = self.cast_lossy();
        // Convert the group element to an address.
        Address::new(group)
    }
}

impl<E: Environment> CastLossy<Boolean<E>> for Field<E> {
    /// Casts a `Field` to a `Boolean`, with lossy truncation.
    /// This operation returns the least significant bit of the field.
    #[inline]
    fn cast_lossy(&self) -> Boolean<E> {
        let bits_le = self.to_bits_le();
        debug_assert!(!bits_le.is_empty(), "An integer must have at least one bit");
        Boolean::new(bits_le[0])
    }
}

impl<E: Environment> CastLossy<Field<E>> for Field<E> {
    /// Casts a `Field` to a `Field`.
    /// This is an identity cast, so it is **always** lossless.
    #[inline]
    fn cast_lossy(&self) -> Field<E> {
        *self
    }
}

impl<E: Environment> CastLossy<Group<E>> for Field<E> {
    /// Casts a `Field` to a `Group`.
    ///
    /// This operation attempts to recover the group element from the given field.
    ///
    /// If the field is a valid x-coordinate, then the group element is returned.
    /// If the field is not a valid x-coordinate, then if the field is the one element,
    /// the generator of the prime-order subgroup is returned.
    /// Otherwise, Elligator-2 is applied to the field element to recover a group element.
    #[inline]
    fn cast_lossy(&self) -> Group<E> {
        match Group::from_x_coordinate(*self) {
            Ok(group) => group,
            Err(_) => match self.is_one() {
                true => Group::generator(),
                false => {
                    // Perform Elligator-2 on the field element, to recover a group element.
                    let result = Elligator2::encode(self);
                    debug_assert!(result.is_ok(), "Elligator-2 should never fail to encode a field element");
                    result.unwrap().0
                }
            },
        }
    }
}

impl<E: Environment, I: IntegerType> CastLossy<Integer<E, I>> for Field<E> {
    /// Casts a `Field` to an `Integer`, with lossy truncation.
    /// This operation truncates the field to an integer.
    #[inline]
    fn cast_lossy(&self) -> Integer<E, I> {
        Integer::from_field_lossy(self)
    }
}

impl<E: Environment> CastLossy<Scalar<E>> for Field<E> {
    /// Casts a `Field` to a `Scalar`, with lossy truncation.
    /// This operation truncates the field to a scalar.
    #[inline]
    fn cast_lossy(&self) -> Scalar<E> {
        Scalar::from_field_lossy(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 10_000;

    #[test]
    fn test_field_to_address() {
        let rng = &mut TestRng::default();

        let field = Field::<CurrentEnvironment>::one();
        let address: Address<CurrentEnvironment> = field.cast_lossy();
        assert_eq!(address, Address::new(Group::generator()));
        assert_eq!(address.to_group(), &Group::generator());

        let field = Field::<CurrentEnvironment>::zero();
        let address: Address<CurrentEnvironment> = field.cast_lossy();
        assert_eq!(address, Address::zero());
        assert_eq!(address.to_group(), &Group::zero());

        for _ in 0..ITERATIONS {
            // Sample a random field.
            let field = Field::<CurrentEnvironment>::rand(rng);
            // Perform the operation.
            let candidate: Address<CurrentEnvironment> = field.cast_lossy();
            // Compare the result against the group element. (This is the most we can do.)
            let expected: Group<CurrentEnvironment> = field.cast_lossy();
            assert_eq!(Address::new(expected), candidate);
        }
    }

    #[test]
    fn test_field_to_boolean() {
        let rng = &mut TestRng::default();

        let field = Field::<CurrentEnvironment>::one();
        let boolean: Boolean<CurrentEnvironment> = field.cast_lossy();
        assert_eq!(boolean, Boolean::new(true));

        let field = Field::<CurrentEnvironment>::zero();
        let boolean: Boolean<CurrentEnvironment> = field.cast_lossy();
        assert_eq!(boolean, Boolean::new(false));

        for _ in 0..ITERATIONS {
            // Sample a random field.
            let field = Field::<CurrentEnvironment>::rand(rng);
            // Perform the operation.
            let candidate: Boolean<CurrentEnvironment> = field.cast_lossy();
            // Compare the result against the least significant bit of the field.
            let expected = Boolean::new(field.to_bits_be().pop().unwrap());
            assert_eq!(expected, candidate);
        }
    }

    #[test]
    fn test_field_to_field() {
        let rng = &mut TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random field.
            let field = Field::<CurrentEnvironment>::rand(rng);
            // Perform the operation.
            let candidate: Field<CurrentEnvironment> = field.cast_lossy();
            assert_eq!(field, candidate);
        }
    }

    #[test]
    fn test_field_to_group() {
        let rng = &mut TestRng::default();

        let field = Field::<CurrentEnvironment>::one();
        let group: Group<CurrentEnvironment> = field.cast_lossy();
        assert_eq!(group, Group::generator());

        let field = Field::<CurrentEnvironment>::zero();
        let group: Group<CurrentEnvironment> = field.cast_lossy();
        assert_eq!(group, Group::zero());

        for _ in 0..ITERATIONS {
            // Sample a random field.
            let field = Field::<CurrentEnvironment>::rand(rng);
            // Perform the operation.
            let candidate: Group<CurrentEnvironment> = field.cast_lossy();
            // Compare the result against the address. (This is the most we can do.)
            let expected: Address<CurrentEnvironment> = field.cast_lossy();
            assert_eq!(expected.to_group(), &candidate);
        }
    }

    #[test]
    fn test_field_to_scalar() {
        let rng = &mut TestRng::default();

        let field = Field::<CurrentEnvironment>::one();
        let scalar: Scalar<CurrentEnvironment> = field.cast_lossy();
        assert_eq!(scalar, Scalar::one());

        let field = Field::<CurrentEnvironment>::zero();
        let scalar: Scalar<CurrentEnvironment> = field.cast_lossy();
        assert_eq!(scalar, Scalar::zero());

        for _ in 0..ITERATIONS {
            // Sample a random field.
            let field = Field::<CurrentEnvironment>::rand(rng);
            // Perform the operation.
            let candidate: Scalar<CurrentEnvironment> = field.cast_lossy();
            assert_eq!(Scalar::from_field_lossy(&field), candidate);
        }
    }

    macro_rules! check_field_to_integer {
        ($type:ty) => {
            let rng = &mut TestRng::default();

            let field = Field::<CurrentEnvironment>::one();
            let integer: $type = field.cast_lossy();
            assert_eq!(integer, <$type>::one());

            let field = Field::<CurrentEnvironment>::zero();
            let integer: $type = field.cast_lossy();
            assert_eq!(integer, <$type>::zero());

            for _ in 0..ITERATIONS {
                // Sample a random field.
                let field = Field::<CurrentEnvironment>::rand(rng);
                // Perform the operation.
                let candidate: $type = field.cast_lossy();
                assert_eq!(<$type>::from_field_lossy(&field), candidate);
            }
        };
    }

    #[test]
    fn test_field_to_i8() {
        check_field_to_integer!(I8<CurrentEnvironment>);
    }

    #[test]
    fn test_field_to_i16() {
        check_field_to_integer!(I16<CurrentEnvironment>);
    }

    #[test]
    fn test_field_to_i32() {
        check_field_to_integer!(I32<CurrentEnvironment>);
    }

    #[test]
    fn test_field_to_i64() {
        check_field_to_integer!(I64<CurrentEnvironment>);
    }

    #[test]
    fn test_field_to_i128() {
        check_field_to_integer!(I128<CurrentEnvironment>);
    }

    #[test]
    fn test_field_to_u8() {
        check_field_to_integer!(U8<CurrentEnvironment>);
    }

    #[test]
    fn test_field_to_u16() {
        check_field_to_integer!(U16<CurrentEnvironment>);
    }

    #[test]
    fn test_field_to_u32() {
        check_field_to_integer!(U32<CurrentEnvironment>);
    }

    #[test]
    fn test_field_to_u64() {
        check_field_to_integer!(U64<CurrentEnvironment>);
    }

    #[test]
    fn test_field_to_u128() {
        check_field_to_integer!(U128<CurrentEnvironment>);
    }
}
