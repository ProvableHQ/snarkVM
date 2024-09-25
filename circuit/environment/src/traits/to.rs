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

/// Unary operator for converting to `k` number of bits.
pub trait ToLowerBits {
    type Boolean: BooleanTrait;

    ///
    /// Outputs the lower `k` bits of an `n`-bit element in little-endian representation.
    /// Enforces that the upper `n - k` bits are zero.
    ///
    fn to_lower_bits_le(&self, k: usize) -> Vec<Self::Boolean>;

    ///
    /// Outputs the lower `k` bits of an `n`-bit element in big-endian representation.
    /// Enforces that the upper `n - k` bits are zero.
    ///
    fn to_lower_bits_be(&self, k: usize) -> Vec<Self::Boolean>;
}

/// Unary operator for converting to `k` number of bits.
pub trait ToUpperBits {
    type Boolean: BooleanTrait;

    ///
    /// Outputs the upper `k` bits of an `n`-bit element in little-endian representation.
    /// Enforces that the lower `n - k` bits are zero.
    ///
    fn to_upper_bits_le(&self, k: usize) -> Vec<Self::Boolean>;

    ///
    /// Outputs the upper `k` bits of an `n`-bit element in big-endian representation.
    /// Enforces that the lower `n - k` bits are zero.
    ///
    fn to_upper_bits_be(&self, k: usize) -> Vec<Self::Boolean>;
}

/// Unary operator for converting to a base field.
pub trait ToField {
    type Field: FieldTrait;

    /// Returns a circuit as a base field element.
    fn to_field(&self) -> Self::Field;
}

/// Unary operator for converting to a list of base fields.
pub trait ToFields {
    type Field: FieldTrait;

    /// Returns the circuit as a list of base field elements.
    fn to_fields(&self) -> Vec<Self::Field>;
}

/// Unary operator for converting to an affine group.
pub trait ToGroup {
    type Group: GroupTrait<Self::Scalar>;
    type Scalar: ScalarTrait;

    /// Returns the circuit as a list of affine group elements.
    fn to_group(&self) -> Self::Group;
}
