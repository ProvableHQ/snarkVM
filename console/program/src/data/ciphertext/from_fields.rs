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

impl<N: Network> TryFrom<Vec<Field<N>>> for Ciphertext<N> {
    type Error = Error;

    /// Initializes a ciphertext from a list of base field elements.
    fn try_from(fields: Vec<Field<N>>) -> Result<Self, Self::Error> {
        // Ensure the number of field elements does not exceed the maximum allowed size.
        match fields.len() <= N::MAX_DATA_SIZE_IN_FIELDS as usize {
            true => Ok(Self(fields)),
            false => bail!("Ciphertext exceeds maximum allowed size"),
        }
    }
}

impl<N: Network> TryFrom<&[Field<N>]> for Ciphertext<N> {
    type Error = Error;

    /// Initializes a ciphertext from a list of base field elements.
    fn try_from(fields: &[Field<N>]) -> Result<Self, Self::Error> {
        Self::from_fields(fields)
    }
}

impl<N: Network> FromFields for Ciphertext<N> {
    type Field = Field<N>;

    /// Initializes a ciphertext from a list of base field elements.
    fn from_fields(fields: &[Self::Field]) -> Result<Self> {
        Self::try_from(fields.to_vec())
    }
}
