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

impl<N: Network> ToBits for Metadata<N> {
    /// Returns the little-endian bits of the metadata.
    #[rustfmt::skip]
    fn write_bits_le(&self, vec: &mut Vec<bool>) {
        0u8.write_bits_le(vec);                               // 1 byte
        self.network.write_bits_le(vec);                      // 2 bytes
        self.round.write_bits_le(vec);                        // 8 bytes
        self.height.write_bits_le(vec);                       // 4 bytes
        self.cumulative_weight.write_bits_le(vec);            // 16 bytes
        self.cumulative_proof_target.write_bits_le(vec);      // 16 bytes
        self.coinbase_target.write_bits_le(vec);              // 8 bytes
        self.proof_target.write_bits_le(vec);                 // 8 bytes
        self.last_coinbase_target.write_bits_le(vec);         // 8 bytes
        self.last_coinbase_timestamp.write_bits_le(vec);      // 8 bytes
        self.timestamp.write_bits_le(vec);                    // 8 bytes
    }

    /// Returns the big-endian bits of the metadata.
    #[rustfmt::skip]
    fn write_bits_be(&self, vec: &mut Vec<bool>) {
        0u8.write_bits_be(vec);                               // 1 byte
        self.network.write_bits_be(vec);                      // 2 bytes
        self.round.write_bits_be(vec);                        // 8 bytes
        self.height.write_bits_be(vec);                       // 4 bytes
        self.cumulative_weight.write_bits_be(vec);            // 16 bytes
        self.cumulative_proof_target.write_bits_be(vec);      // 16 bytes
        self.coinbase_target.write_bits_be(vec);              // 8 bytes
        self.proof_target.write_bits_be(vec);                 // 8 bytes
        self.last_coinbase_target.write_bits_be(vec);         // 8 bytes
        self.last_coinbase_timestamp.write_bits_be(vec);      // 8 bytes
        self.timestamp.write_bits_be(vec);                    // 8 bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bits_le() {
        let rng = &mut TestRng::default();

        for expected in [crate::header::metadata::test_helpers::sample_block_metadata(rng)].into_iter() {
            // Check the length matches.
            let expected_bytes = expected.to_bytes_le().unwrap();
            let expected_bits = expected.to_bits_le();
            assert_eq!(expected_bytes.to_bits_le().len(), expected_bits.len());
        }
    }

    #[test]
    fn test_bits_be() {
        let rng = &mut TestRng::default();

        for expected in [crate::header::metadata::test_helpers::sample_block_metadata(rng)].into_iter() {
            // Check the length matches.
            let expected_bytes = expected.to_bytes_le().unwrap(); // There is no 'to_bytes_be' function.
            let expected_bits = expected.to_bits_be();
            assert_eq!(expected_bytes.to_bits_be().len(), expected_bits.len());
        }
    }
}
