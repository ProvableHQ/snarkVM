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

use crate::{BooleanTrait, FieldTrait, GroupTrait, ScalarTrait};

/// Unary operator for instantiating from a boolean.
pub trait FromBoolean {
    type Boolean: BooleanTrait;

    fn from_boolean(boolean: &Self::Boolean) -> Self
    where
        Self: Sized;
}

/// Unary operator for instantiating from bits.
pub trait FromBits {
    type Boolean: BooleanTrait;

    fn from_bits_le(bits_le: &[Self::Boolean]) -> Self
    where
        Self: Sized;

    fn from_bits_be(bits_be: &[Self::Boolean]) -> Self
    where
        Self: Sized;
}

/// Unary operator for converting from a base field element.
pub trait FromField {
    type Field: FieldTrait;

    /// Casts a circuit from a base field element.
    fn from_field(field: Self::Field) -> Self;
}

/// Unary operator for converting from a list of base elements.
pub trait FromFields {
    type Field: FieldTrait;

    /// Casts a circuit from a list of base field elements.
    fn from_fields(fields: &[Self::Field]) -> Self;
}

/// Unary operator for converting from an affine group element.
pub trait FromGroup {
    type Group: GroupTrait<Self::Scalar>;
    type Scalar: ScalarTrait;

    /// Casts a circuit from an affine group element.
    fn from_group(group: Self::Group) -> Self;
}
