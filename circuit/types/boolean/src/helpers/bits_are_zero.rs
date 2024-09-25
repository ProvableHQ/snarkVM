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

impl<E: Environment> Boolean<E> {
    /// Asserts that all bits in `bits_le` are zero.
    #[doc(hidden)]
    pub fn assert_bits_are_zero(bits_le: &[Boolean<E>]) {
        let mut sum = Self::constant(false).0;
        for bit in bits_le {
            sum += &**bit;
        }
        E::assert_eq(sum, E::zero());
    }
}
