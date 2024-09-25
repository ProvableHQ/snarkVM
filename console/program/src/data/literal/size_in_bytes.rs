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

impl<N: Network> Literal<N> {
    /// Returns the size in bytes of this literal.
    #[allow(clippy::cast_possible_truncation)]
    pub fn size_in_bytes(&self) -> u16 {
        // Note: This upcast to u32 and downcast to u16 is safe because the size of a literal is
        // always less than or equal to u16::MAX bits, and we are dividing by 8, so the result will
        // always fit in a u16.
        (((self.size_in_bits() as u32) + 7) / 8) as u16
    }
}
