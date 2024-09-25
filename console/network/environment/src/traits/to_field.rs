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

use super::FieldTrait;

use anyhow::Result;

/// Unary operator for converting to a base field.
pub trait ToField {
    type Field: FieldTrait;

    /// Returns the object as a base field element.
    fn to_field(&self) -> Result<Self::Field>;
}

/// Unary operator for converting to a list of base fields.
pub trait ToFields {
    type Field: FieldTrait;

    /// Returns the object as a list of base field elements.
    fn to_fields(&self) -> Result<Vec<Self::Field>>;
}
