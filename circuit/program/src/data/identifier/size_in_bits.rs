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

impl<A: Aleo> Identifier<A> {
    /// Returns the number of bits of this identifier.
    pub fn size_in_bits(&self) -> U8<A> {
        match self.1.checked_mul(8) {
            Some(num_bits) => U8::constant(console::U8::new(num_bits)),
            None => A::halt("Identifier exceeds maximum allowed size"),
        }
    }
}
