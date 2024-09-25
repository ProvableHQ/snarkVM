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

impl<A: Aleo> ToFields for Ciphertext<A> {
    type Field = Field<A>;

    /// Returns this ciphertext as a list of field elements.
    fn to_fields(&self) -> Vec<Self::Field> {
        // Ensure the number of field elements does not exceed the maximum allowed size.
        match self.0.len() <= A::MAX_DATA_SIZE_IN_FIELDS as usize {
            true => self.0.clone(),
            false => A::halt("Ciphertext exceeds maximum allowed size"),
        }
    }
}
