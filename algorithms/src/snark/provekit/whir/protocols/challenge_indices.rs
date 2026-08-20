// Copyright (c) 2019-2026 Provable Inc.
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

// Originally derived from WHIR (https://github.com/WizardOfMenlo/whir),
// licensed under Apache-2.0 OR MIT.

//! Produce challenge indices from a transcript.

use crate::snark::provekit::whir::transcript::{Decoding, VerifierMessage};

/// Generate a set of indices for challenges.
pub fn challenge_indices<T>(transcript: &mut T, num_leaves: usize, count: usize, deduplicate: bool) -> Vec<usize>
where
    T: VerifierMessage,
    u8: Decoding<[T::U]>,
{
    if count == 0 {
        return Vec::new();
    }
    // TODO: This is blocking non-power-of-two support.
    assert!(num_leaves.is_power_of_two(), "Number of leaves must be a power of two for unbiased results.");
    if num_leaves == 1 {
        // `size_bytes` would be zero, making `chunks_exact` panic.
        return if deduplicate { vec![0] } else { vec![0; count] };
    }

    // Calculate the required bytes of entropy
    // TODO: Round total to bytes, instead of per index.
    let size_bytes = (num_leaves.ilog2() as usize).div_ceil(8);

    // Get required entropy bits.
    let entropy: Vec<u8> = (0..count * size_bytes).map(|_| transcript.verifier_message()).collect();

    // Convert bytes into indices
    let mut indices = entropy
        .chunks_exact(size_bytes)
        .map(|chunk| chunk.iter().fold(0usize, |acc, &b| (acc << 8) | b as usize) % num_leaves)
        .collect::<Vec<usize>>();

    // Sort and deduplicate indices if requested
    if deduplicate {
        indices.sort_unstable();
        indices.dedup();
    }
    indices
}

#[cfg(any())]
mod tests {
    use super::*;
    use crate::snark::provekit::whir::transcript::{DomainSeparator, MockSponge, ProverState, codecs::Empty};

    #[test]
    fn test_challenge_stir_queries_single_byte_indices() {
        let num_leaves = 1 << 7;
        let num_queries = 5;

        let ds = DomainSeparator::protocol(&module_path!())
            .session(&format!("Test at {}:{}", file!(), line!()))
            .instance(&Empty);
        // Mock transcript with fixed bytes (ensuring reproducibility)
        let sponge = MockSponge {
            absorb: None, // Anything is fine
            squeeze: &[
                0x01, 0x23, 0x45, 0x67, 0x89, // Query 1
                0xAB, 0xCD, 0xEF, 0x12, 0x34, // Query 2
                0x56, 0x78, 0x9A, 0xBC, 0xDE, // Query 3
                0xF0, 0x11, 0x22, 0x33, 0x44, // Query 4
                0x55, 0x66, 0x77, 0x88, 0x99, // Query 5
            ],
        };
        let mut prover_state = ProverState::new(&ds, sponge);

        let result = challenge_indices(&mut prover_state, num_leaves, num_queries, true);

        // Manually computed expected indices
        let index_0 = 0x01 % num_leaves;
        let index_1 = 0x23 % num_leaves;
        let index_2 = 0x45 % num_leaves;
        let index_3 = 0x67 % num_leaves;
        let index_4 = 0x89 % num_leaves;

        let mut expected_indices = vec![index_0, index_1, index_2, index_3, index_4];
        expected_indices.sort_unstable();
        expected_indices.dedup();

        assert_eq!(result, expected_indices, "Mismatch in computed indices for domain_size_bytes = 1");
    }

    #[test]
    fn test_challenge_stir_queries_two_byte_indices() {
        let num_leaves = 1 << 13;
        let num_queries = 5;

        let ds = DomainSeparator::protocol(&module_path!())
            .session(&format!("Test at {}:{}", file!(), line!()))
            .instance(&Empty);
        // Expected `folded_domain_size = 65536 / 8 = 8192`
        let sponge = MockSponge {
            absorb: None,
            squeeze: &[
                0x01, 0x23, 0x45, 0x67, 0x89, // Query 1
                0xAB, 0xCD, 0xEF, 0x12, 0x34, // Query 2
                0x56, 0x78, 0x9A, 0xBC, 0xDE, // Query 3
                0xF0, 0x11, 0x22, 0x33, 0x44, // Query 4
                0x55, 0x66, 0x77, 0x88, 0x99, // Query 5
            ],
        };
        let mut prover_state = ProverState::new(&ds, sponge);

        let result = challenge_indices(&mut prover_state, num_leaves, num_queries, true);

        // Manually computed expected indices using two bytes per index
        let index_0 = ((0x01 << 8) | 0x23) % num_leaves;
        let index_1 = ((0x45 << 8) | 0x67) % num_leaves;
        let index_2 = ((0x89 << 8) | 0xAB) % num_leaves;
        let index_3 = ((0xCD << 8) | 0xEF) % num_leaves;
        let index_4 = ((0x12 << 8) | 0x34) % num_leaves;

        let mut expected_indices = vec![index_0, index_1, index_2, index_3, index_4];
        expected_indices.sort_unstable();

        assert_eq!(result, expected_indices, "Mismatch in computed indices for domain_size_bytes = 2");
    }

    #[test]
    fn test_challenge_stir_queries_three_byte_indices() {
        let num_leaves = 1 << 20;
        let num_queries = 4;

        // Expected `folded_domain_size = 2^24 / 16 = 2^20 = 1,048,576`
        let ds = DomainSeparator::protocol(&module_path!())
            .session(&format!("Test at {}:{}", file!(), line!()))
            .instance(&Empty);
        let sponge = MockSponge {
            absorb: None,
            squeeze: &[
                0x12, 0x34, 0x56, // Query 1
                0x78, 0x9A, 0xBC, // Query 2
                0xDE, 0xF0, 0x11, // Query 3
                0x22, 0x33, 0x44, // Query 4
            ],
        };
        let mut prover_state = ProverState::new(&ds, sponge);

        let result = challenge_indices(&mut prover_state, num_leaves, num_queries, true);

        // Manually computed expected indices using three bytes per index
        let index_0 = ((0x12 << 16) | (0x34 << 8) | 0x56) % num_leaves;
        let index_1 = ((0x78 << 16) | (0x9A << 8) | 0xBC) % num_leaves;
        let index_2 = ((0xDE << 16) | (0xF0 << 8) | 0x11) % num_leaves;
        let index_3 = ((0x22 << 16) | (0x33 << 8) | 0x44) % num_leaves;

        let mut expected_indices = vec![index_0, index_1, index_2, index_3];
        expected_indices.sort_unstable();

        assert_eq!(result, expected_indices, "Mismatch in computed indices for domain_size_bytes = 3");
    }

    #[test]
    fn test_challenge_stir_queries_duplicate_indices() {
        // Case where the function should deduplicate indices
        let num_leaves = 128;
        let num_queries = 5;

        // Mock narg_string where some indices will collide
        let ds = DomainSeparator::protocol(&module_path!())
            .session(&format!("Test at {}:{}", file!(), line!()))
            .instance(&Empty);
        let mut prover_state = ProverState::new(&ds, MockSponge {
            absorb: None,
            squeeze: &[
                0x20, 0x40, 0x20, 0x60, 0x40, // Duplicate indices 0x20 and 0x40
            ],
        });

        let result = challenge_indices(&mut prover_state, num_leaves, num_queries, true);

        // Manually computed expected indices, ensuring duplicates are removed
        let mut expected_indices = vec![0x20 % num_leaves, 0x40 % num_leaves, 0x60 % num_leaves];
        expected_indices.sort_unstable();

        assert_eq!(result, expected_indices, "Mismatch in computed indices for deduplication test");
    }
}
