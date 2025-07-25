// Copyright (c) 2019-2025 Provable Inc.
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

use crate::ConsensusVersion;

/// The consensus version height for `CanaryV0`.
pub const CANARY_V0_CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); 9] = [
    (ConsensusVersion::V1, 0),
    (ConsensusVersion::V2, 2_900_000),
    (ConsensusVersion::V3, 4_560_000),
    (ConsensusVersion::V4, 5_730_000),
    (ConsensusVersion::V5, 5_780_000),
    (ConsensusVersion::V6, 6_240_000),
    (ConsensusVersion::V7, 6_880_000),
    (ConsensusVersion::V8, 7_565_000),
    (ConsensusVersion::V9, 8_028_000),
];

/// The consensus version height for `MainnetV0`.
pub const MAINNET_V0_CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); 9] = [
    (ConsensusVersion::V1, 0),
    (ConsensusVersion::V2, 2_800_000),
    (ConsensusVersion::V3, 4_900_000),
    (ConsensusVersion::V4, 6_135_000),
    (ConsensusVersion::V5, 7_060_000),
    (ConsensusVersion::V6, 7_560_000),
    (ConsensusVersion::V7, 7_570_000),
    (ConsensusVersion::V8, 9_430_000),
    (ConsensusVersion::V9, 10_272_000),
];

/// The consensus version heights for `TestnetV0`.
pub const TESTNET_V0_CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); 9] = [
    (ConsensusVersion::V1, 0),
    (ConsensusVersion::V2, 2_950_000),
    (ConsensusVersion::V3, 4_800_000),
    (ConsensusVersion::V4, 6_625_000),
    (ConsensusVersion::V5, 6_765_000),
    (ConsensusVersion::V6, 7_600_000),
    (ConsensusVersion::V7, 8_365_000),
    (ConsensusVersion::V8, 9_173_000),
    (ConsensusVersion::V9, 9_800_000),
];

/// The consensus version heights when the `test_consensus_heights` feature is enabled.
pub const TEST_CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); 9] = [
    (ConsensusVersion::V1, 0),
    (ConsensusVersion::V2, 10),
    (ConsensusVersion::V3, 11),
    (ConsensusVersion::V4, 12),
    (ConsensusVersion::V5, 13),
    (ConsensusVersion::V6, 14),
    (ConsensusVersion::V7, 15),
    (ConsensusVersion::V8, 16),
    (ConsensusVersion::V9, 17),
];

#[cfg(any(test, feature = "test", feature = "test_consensus_heights"))]
pub(crate) fn load_test_consensus_heights<N: crate::Network>()
-> [(ConsensusVersion, u32); crate::NUM_CONSENSUS_VERSIONS] {
    // Define a closure to verify the consensus heights.
    let verify_consensus_heights = |heights: &[(ConsensusVersion, u32); crate::NUM_CONSENSUS_VERSIONS]| {
        // Assert that the genesis height is 0.
        assert_eq!(heights[0].1, 0, "Genesis height must be 0.");
        // Assert that the consensus heights are strictly increasing.
        for window in heights.windows(2) {
            if window[0] >= window[1] {
                panic!("Heights must be strictly increasing, but found: {window:?}");
            }
        }
    };

    // Define consensus version heights container used for testing.
    let mut test_consensus_heights = N::TEST_CONSENSUS_VERSION_HEIGHTS;

    // Check if we can read the heights from an environment variable.
    match std::env::var("CONSENSUS_VERSION_HEIGHTS") {
        Ok(height_string) => {
            // Parse the heights from the environment variable.
            let parsed_test_consensus_heights: [u32; crate::NUM_CONSENSUS_VERSIONS] = height_string
                .replace(" ", "")
                .split(",")
                .map(|height| height.parse::<u32>().unwrap())
                .collect::<Vec<u32>>()
                .try_into()
                .unwrap();
            // Set the parsed heights in the test consensus heights.
            for (i, height) in parsed_test_consensus_heights.into_iter().enumerate() {
                test_consensus_heights[i] = (N::TEST_CONSENSUS_VERSION_HEIGHTS[i].0, height);
            }
            // Verify and return the parsed test consensus heights.
            verify_consensus_heights(&test_consensus_heights);
            test_consensus_heights
        }
        Err(_) => {
            // Verify and return the default test consensus heights.
            verify_consensus_heights(&test_consensus_heights);
            test_consensus_heights
        }
    }
}
