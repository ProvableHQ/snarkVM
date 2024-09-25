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

impl<N: Network> ToBits for Signature<N> {
    /// Returns the little-endian bits of the signature.
    fn write_bits_le(&self, vec: &mut Vec<bool>) {
        // Write the challenge bits.
        self.challenge.write_bits_le(vec);
        // Write the response bits.
        self.response.write_bits_le(vec);
        // Write the compute key bits.
        self.compute_key.write_bits_le(vec);
    }

    /// Returns the big-endian bits of the signature.
    fn write_bits_be(&self, vec: &mut Vec<bool>) {
        // Write the challenge bits.
        self.challenge.write_bits_be(vec);
        // Write the response bits.
        self.response.write_bits_be(vec);
        // Write the compute key bits.
        self.compute_key.write_bits_be(vec);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 1_000;

    #[test]
    fn test_to_bits_le() {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random signature.
            let signature = test_helpers::sample_signature(i, rng);

            let candidate = signature.to_bits_le();
            assert_eq!(Signature::<CurrentNetwork>::size_in_bits(), candidate.len());

            // Construct the expected bits.
            let mut expected = Vec::new();
            expected.extend(signature.challenge.to_bits_le());
            expected.extend(signature.response.to_bits_le());
            expected.extend(signature.compute_key.to_bits_le());

            for (expected, candidate) in expected.iter().zip_eq(&candidate) {
                assert_eq!(expected, candidate);
            }
        }
    }

    #[test]
    fn test_to_bits_be() {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random signature.
            let signature = test_helpers::sample_signature(i, rng);

            let candidate = signature.to_bits_be();
            assert_eq!(Signature::<CurrentNetwork>::size_in_bits(), candidate.len());

            // Construct the expected bits.
            let mut expected = Vec::new();
            expected.extend(signature.challenge.to_bits_be());
            expected.extend(signature.response.to_bits_be());
            expected.extend(signature.compute_key.to_bits_be());

            for (expected, candidate) in expected.iter().zip_eq(&candidate) {
                assert_eq!(expected, candidate);
            }
        }
    }
}
