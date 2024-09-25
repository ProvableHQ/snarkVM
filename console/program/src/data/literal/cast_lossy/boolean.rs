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

impl<E: Environment> CastLossy<Address<E>> for Boolean<E> {
    /// Casts a `Boolean` to an `Address`.
    /// This is safe because casting from a boolean to any other type is **always** lossless.
    ///
    /// If the boolean is true, the address is the generator of the prime-order subgroup.
    /// If the boolean is false, the address is the zero group element.
    #[inline]
    fn cast_lossy(&self) -> Address<E> {
        match **self {
            true => Address::new(Group::generator()),
            false => Address::zero(),
        }
    }
}

impl<E: Environment> CastLossy<Boolean<E>> for Boolean<E> {
    /// Casts a `Boolean` to a `Boolean`.
    /// This is an identity cast, so it is **always** lossless.
    #[inline]
    fn cast_lossy(&self) -> Boolean<E> {
        *self
    }
}

impl<E: Environment> CastLossy<Field<E>> for Boolean<E> {
    /// Casts a `Boolean` to a `Field`.
    /// This is safe because casting from a boolean to any other type is **always** lossless.
    #[inline]
    fn cast_lossy(&self) -> Field<E> {
        match **self {
            true => Field::one(),
            false => Field::zero(),
        }
    }
}

impl<E: Environment> CastLossy<Group<E>> for Boolean<E> {
    /// Casts a `Boolean` to a `Group`.
    /// This is safe because casting from a boolean to any other type is **always** lossless.
    ///
    /// If the boolean is true, the group element is the generator of the prime-order subgroup.
    /// If the boolean is false, the group element is the zero group element.
    #[inline]
    fn cast_lossy(&self) -> Group<E> {
        match **self {
            true => Group::generator(),
            false => Group::zero(),
        }
    }
}

impl<E: Environment, I: IntegerType> CastLossy<Integer<E, I>> for Boolean<E> {
    /// Casts a `Boolean` to an `Integer`.
    #[inline]
    fn cast_lossy(&self) -> Integer<E, I> {
        match **self {
            true => Integer::one(),
            false => Integer::zero(),
        }
    }
}

impl<E: Environment> CastLossy<Scalar<E>> for Boolean<E> {
    /// Casts a `Boolean` to a `Scalar`.
    /// This is safe because casting from a boolean to any other type is **always** lossless.
    #[inline]
    fn cast_lossy(&self) -> Scalar<E> {
        match **self {
            true => Scalar::one(),
            false => Scalar::zero(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type CurrentEnvironment = snarkvm_console_network::Console;

    #[test]
    fn test_boolean_to_address() {
        let boolean = Boolean::<CurrentEnvironment>::new(true);
        let address: Address<CurrentEnvironment> = boolean.cast_lossy();
        assert_eq!(address, Address::new(Group::generator()));
        assert_eq!(address.to_group(), &Group::generator());

        let boolean = Boolean::<CurrentEnvironment>::new(false);
        let address: Address<CurrentEnvironment> = boolean.cast_lossy();
        assert_eq!(address, Address::zero());
        assert_eq!(address.to_group(), &Group::zero());
    }

    #[test]
    fn test_boolean_to_boolean() {
        let boolean = Boolean::<CurrentEnvironment>::new(true);
        let boolean: Boolean<CurrentEnvironment> = boolean.cast_lossy();
        assert_eq!(boolean, Boolean::new(true));

        let boolean = Boolean::<CurrentEnvironment>::new(false);
        let boolean: Boolean<CurrentEnvironment> = boolean.cast_lossy();
        assert_eq!(boolean, Boolean::new(false));
    }

    #[test]
    fn test_boolean_to_field() {
        let boolean = Boolean::<CurrentEnvironment>::new(true);
        let field: Field<CurrentEnvironment> = boolean.cast_lossy();
        assert_eq!(field, Field::one());

        let boolean = Boolean::<CurrentEnvironment>::new(false);
        let field: Field<CurrentEnvironment> = boolean.cast_lossy();
        assert_eq!(field, Field::zero());
    }

    #[test]
    fn test_boolean_to_group() {
        let boolean = Boolean::<CurrentEnvironment>::new(true);
        let group: Group<CurrentEnvironment> = boolean.cast_lossy();
        assert_eq!(group, Group::generator());

        let boolean = Boolean::<CurrentEnvironment>::new(false);
        let group: Group<CurrentEnvironment> = boolean.cast_lossy();
        assert_eq!(group, Group::zero());
    }

    #[test]
    fn test_boolean_to_scalar() {
        let boolean = Boolean::<CurrentEnvironment>::new(true);
        let scalar: Scalar<CurrentEnvironment> = boolean.cast_lossy();
        assert_eq!(scalar, Scalar::one());

        let boolean = Boolean::<CurrentEnvironment>::new(false);
        let scalar: Scalar<CurrentEnvironment> = boolean.cast_lossy();
        assert_eq!(scalar, Scalar::zero());
    }

    macro_rules! check_boolean_to_integer {
        ($type:ty) => {
            let boolean = Boolean::<CurrentEnvironment>::new(true);
            let integer: $type = boolean.cast_lossy();
            assert_eq!(integer, <$type>::one());

            let boolean = Boolean::<CurrentEnvironment>::new(false);
            let integer: $type = boolean.cast_lossy();
            assert_eq!(integer, <$type>::zero());
        };
    }

    #[test]
    fn test_boolean_to_i8() {
        check_boolean_to_integer!(I8<CurrentEnvironment>);
    }

    #[test]
    fn test_boolean_to_i16() {
        check_boolean_to_integer!(I16<CurrentEnvironment>);
    }

    #[test]
    fn test_boolean_to_i32() {
        check_boolean_to_integer!(I32<CurrentEnvironment>);
    }

    #[test]
    fn test_boolean_to_i64() {
        check_boolean_to_integer!(I64<CurrentEnvironment>);
    }

    #[test]
    fn test_boolean_to_i128() {
        check_boolean_to_integer!(I128<CurrentEnvironment>);
    }

    #[test]
    fn test_boolean_to_u8() {
        check_boolean_to_integer!(U8<CurrentEnvironment>);
    }

    #[test]
    fn test_boolean_to_u16() {
        check_boolean_to_integer!(U16<CurrentEnvironment>);
    }

    #[test]
    fn test_boolean_to_u32() {
        check_boolean_to_integer!(U32<CurrentEnvironment>);
    }

    #[test]
    fn test_boolean_to_u64() {
        check_boolean_to_integer!(U64<CurrentEnvironment>);
    }

    #[test]
    fn test_boolean_to_u128() {
        check_boolean_to_integer!(U128<CurrentEnvironment>);
    }
}
