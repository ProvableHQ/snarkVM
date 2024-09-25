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

impl<A: Aleo> FromBits for Ciphertext<A> {
    type Boolean = Boolean<A>;

    /// Returns this ciphertext as a list of **little-endian** bits.
    fn from_bits_le(bits_le: &[Self::Boolean]) -> Self {
        Self(bits_le.chunks(A::BaseField::size_in_bits()).map(Field::from_bits_le).collect())
    }

    /// Returns this ciphertext as a list of **big-endian** bits.
    fn from_bits_be(bits_be: &[Self::Boolean]) -> Self {
        Self(bits_be.chunks(A::BaseField::size_in_bits()).map(Field::from_bits_be).collect())
    }
}
