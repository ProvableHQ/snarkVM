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

impl<A: Aleo, Private: Visibility<A>> Record<A, Private> {
    /// Returns the number of field elements to encode `self`.
    pub(crate) fn num_randomizers(&self) -> u16 {
        // Initialize an tracker for the number of randomizers.
        let mut num_randomizers: u16 = 0;

        // If the owner is private, increment the number of randomizers by 1.
        if self.owner.is_private().eject_value() {
            num_randomizers += 1;
        }

        // Increment the number of randomizers by the number of data randomizers.
        for (_, entry) in self.data.iter() {
            num_randomizers = match num_randomizers.checked_add(entry.num_randomizers()) {
                Some(num_randomizers) => num_randomizers,
                None => A::halt("Number of randomizers exceeds the maximum allowed size."),
            };
        }

        // Ensure the number of randomizers does not exceed the maximum allowed size.
        match num_randomizers as u32 <= A::MAX_DATA_SIZE_IN_FIELDS {
            true => num_randomizers,
            false => A::halt("Number of randomizers exceeds the maximum allowed size."),
        }
    }
}
