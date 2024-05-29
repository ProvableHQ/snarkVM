// Copyright (C) 2019-2023 Aleo Systems Inc.
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

impl<N: Network> ToBits for TransactionLeaf<N> {
    /// Returns the little-endian bits of the Merkle leaf.
    fn write_bits_le<T: VecLike>(&self, vec: &mut T) {
        // Construct the leaf as (variant || index || ID).
        self.variant.write_bits_le(vec);
        self.index.write_bits_le(vec);
        self.id.write_bits_le(vec);
    }

    /// Returns the big-endian bits of the Merkle leaf.
    fn write_bits_be(&self, vec: &mut Vec<bool>) {
        // Construct the leaf as (variant || index || ID).
        self.variant.write_bits_be(vec);
        self.index.write_bits_be(vec);
        self.id.write_bits_be(vec);
    }
}
