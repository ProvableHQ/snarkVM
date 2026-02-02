// Copyright (c) 2019-2026 Provable Inc.
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

impl<A: Aleo> ToFieldsRaw for Plaintext<A> {
    /// Returns this plaintext as a list of field elements using the raw bits.
    fn to_fields_raw(&self) -> Vec<Self::Field> {
        // Encode the data as little-endian bits without variant or identifier bits.
        let bits_le = self.to_bits_raw_le();
        // Pack the bits into field elements.
        let fields = bits_le.chunks(A::BaseField::size_in_data_bits()).map(Field::from_bits_le).collect::<Vec<_>>();
        // Ensure the number of field elements does not exceed the maximum allowed size.
        match fields.len() <= A::MAX_DATA_SIZE_IN_FIELDS as usize {
            true => fields,
            false => A::halt("Plaintext exceeds maximum allowed size"),
        }
    }
}
