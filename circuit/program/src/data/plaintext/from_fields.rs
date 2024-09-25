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

impl<A: Aleo> From<Vec<Field<A>>> for Plaintext<A> {
    /// Initializes a plaintext from a list of base field elements.
    fn from(fields: Vec<Field<A>>) -> Self {
        Self::from_fields(&fields)
    }
}

impl<A: Aleo> From<&[Field<A>]> for Plaintext<A> {
    /// Initializes a plaintext from a list of base field elements.
    fn from(fields: &[Field<A>]) -> Self {
        Self::from_fields(fields)
    }
}

impl<A: Aleo> FromFields for Plaintext<A> {
    type Field = Field<A>;

    /// Initializes a plaintext from a list of base field elements.
    fn from_fields(fields: &[Self::Field]) -> Self {
        // Ensure the number of field elements does not exceed the maximum allowed size.
        if fields.len() > A::MAX_DATA_SIZE_IN_FIELDS as usize {
            A::halt("Plaintext exceeds maximum allowed size")
        }

        // Unpack the field elements into little-endian bits, and reverse the list for popping the terminus bit off.
        let mut bits_le = fields
            .iter()
            .flat_map(|field| field.to_bits_le().into_iter().take(A::BaseField::size_in_data_bits()))
            .rev();
        // Remove the terminus bit that was added during encoding.
        for boolean in bits_le.by_ref() {
            // Drop all extraneous `0` bits, in addition to the final `1` bit.
            if boolean.eject_value() {
                // This case will always be reached, since the terminus bit is always `1`.
                break;
            }
        }
        // Reverse the bits back and recover the data from the bits.
        Self::from_bits_le(&bits_le.rev().collect::<Vec<_>>())
    }
}
