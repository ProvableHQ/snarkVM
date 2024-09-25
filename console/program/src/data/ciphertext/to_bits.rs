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

impl<N: Network> ToBits for Ciphertext<N> {
    /// Returns this ciphertext as a list of **little-endian** bits.
    fn write_bits_le(&self, vec: &mut Vec<bool>) {
        let initial_len = vec.len();
        self.0.write_bits_le(vec);
        assert_eq!(self.0.len() * Field::<N>::size_in_bits(), vec.len() - initial_len);
    }

    /// Returns this ciphertext as a list of **big-endian** bits.
    fn write_bits_be(&self, vec: &mut Vec<bool>) {
        let initial_len = vec.len();
        self.0.write_bits_be(vec);
        assert_eq!(self.0.len() * Field::<N>::size_in_bits(), vec.len() - initial_len);
    }
}
