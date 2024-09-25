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

impl<A: Aleo, Private: Visibility<A>> Entry<A, Private> {
    /// Returns the number of field elements to encode `self`.
    pub(crate) fn num_randomizers(&self) -> u16 {
        match self {
            // Constant and public entries do not need to be encrypted.
            Self::Constant(..) | Self::Public(..) => 0u16,
            // Private entries need one randomizer per field element.
            Self::Private(private) => private.size_in_fields(),
        }
    }
}
