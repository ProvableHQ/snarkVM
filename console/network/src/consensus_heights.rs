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

use enum_iterator::{Sequence, last};

/// The different consensus versions.
/// If you need the version active for a specific height, see: `N::CONSENSUS_VERSION`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Sequence)]
pub enum ConsensusVersion {
    /// V1: The initial genesis consensus version.
    V1 = 1,
    /// V2: Update to the block reward and execution cost algorithms.
    V2 = 2,
    /// V3: Update to the number of validators and finalize scope RNG seed.
    V3 = 3,
    /// V4: Update to the Varuna version.
    V4 = 4,
    /// V5: Update to the number of validators and enable batch proposal spend limits.
    V5 = 5,
    /// V6: Update to the number of validators.
    V6 = 6,
    /// V7: Update to program rules.
    V7 = 7,
    /// V8: Update to inclusion version, record commitment version, and introduces sender ciphertexts.
    V8 = 8,
    /// V9: Support for program upgradability.
    V9 = 9,
    /// V10: Lower fees, appropriate record output type checking.
    V10 = 10,
    /// V11: Support for external structs.
    V11 = 11,
}

impl ConsensusVersion {
    pub fn latest() -> Self {
        last::<ConsensusVersion>().expect("At least one ConsensusVersion should be defined.")
    }
}

/// The number of consensus versions.
pub(crate) const NUM_CONSENSUS_VERSIONS: usize = 11;

/// The consensus version height for `CanaryV0`.
pub const CANARY_V0_CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); NUM_CONSENSUS_VERSIONS] = [
    (ConsensusVersion::V1, 0),
    (ConsensusVersion::V2, 2_900_000),
    (ConsensusVersion::V3, 4_560_000),
    (ConsensusVersion::V4, 5_730_000),
    (ConsensusVersion::V5, 5_780_000),
    (ConsensusVersion::V6, 6_240_000),
    (ConsensusVersion::V7, 6_880_000),
    (ConsensusVersion::V8, 7_565_000),
    (ConsensusVersion::V9, 8_028_000),
    (ConsensusVersion::V10, 8_600_000),
    (ConsensusVersion::V11, 10_235_000),
];

/// The consensus version height for `MainnetV0`.
pub const MAINNET_V0_CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); NUM_CONSENSUS_VERSIONS] = [
    (ConsensusVersion::V1, 0),
    (ConsensusVersion::V2, 2_800_000),
    (ConsensusVersion::V3, 4_900_000),
    (ConsensusVersion::V4, 6_135_000),
    (ConsensusVersion::V5, 7_060_000),
    (ConsensusVersion::V6, 7_560_000),
    (ConsensusVersion::V7, 7_570_000),
    (ConsensusVersion::V8, 9_430_000),
    (ConsensusVersion::V9, 10_272_000),
    (ConsensusVersion::V10, 11_115_000),
    (ConsensusVersion::V11, 13_575_000),
];

/// The consensus version heights for `TestnetV0`.
pub const TESTNET_V0_CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); NUM_CONSENSUS_VERSIONS] = [
    (ConsensusVersion::V1, 0),
    (ConsensusVersion::V2, 2_950_000),
    (ConsensusVersion::V3, 4_800_000),
    (ConsensusVersion::V4, 6_625_000),
    (ConsensusVersion::V5, 6_765_000),
    (ConsensusVersion::V6, 7_600_000),
    (ConsensusVersion::V7, 8_365_000),
    (ConsensusVersion::V8, 9_173_000),
    (ConsensusVersion::V9, 9_800_000),
    (ConsensusVersion::V10, 10_525_000),
    (ConsensusVersion::V11, 12_660_000),
];

/// The consensus version heights when the `test_consensus_heights` feature is enabled.
pub const TEST_CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); NUM_CONSENSUS_VERSIONS] = [
    (ConsensusVersion::V1, 0),
    (ConsensusVersion::V2, 10),
    (ConsensusVersion::V3, 11),
    (ConsensusVersion::V4, 12),
    (ConsensusVersion::V5, 13),
    (ConsensusVersion::V6, 14),
    (ConsensusVersion::V7, 15),
    (ConsensusVersion::V8, 16),
    (ConsensusVersion::V9, 17),
    (ConsensusVersion::V10, 18),
    (ConsensusVersion::V11, 19),
];

#[cfg(any(test, feature = "test", feature = "test_consensus_heights"))]
pub fn load_test_consensus_heights() -> [(ConsensusVersion, u32); NUM_CONSENSUS_VERSIONS] {
    // Define a closure to verify the consensus heights.
    let verify_consensus_heights = |heights: &[(ConsensusVersion, u32); NUM_CONSENSUS_VERSIONS]| {
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
    let mut test_consensus_heights = TEST_CONSENSUS_VERSION_HEIGHTS;

    // Check if we can read the heights from an environment variable.
    match std::env::var("CONSENSUS_VERSION_HEIGHTS") {
        Ok(height_string) => {
            let parsing_error = format!("Expected exactly {NUM_CONSENSUS_VERSIONS} ConsensusVersion heights.");
            // Parse the heights from the environment variable.
            let parsed_test_consensus_heights: [u32; NUM_CONSENSUS_VERSIONS] = height_string
                .replace(" ", "")
                .split(",")
                .map(|height| height.parse::<u32>().expect("Heights should be valid u32 values."))
                .collect::<Vec<u32>>()
                .try_into()
                .expect(&parsing_error);
            // Set the parsed heights in the test consensus heights.
            for (i, height) in parsed_test_consensus_heights.into_iter().enumerate() {
                test_consensus_heights[i] = (TEST_CONSENSUS_VERSION_HEIGHTS[i].0, height);
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

/// Returns the consensus configuration value for the specified height.
///
/// Arguments:
/// - `$network`: The network to use the constant of.
/// - `$constant`: The constant to search a value of.
/// - `$seek_height`: The block height to search the value for.
#[macro_export]
macro_rules! consensus_config_value {
    ($network:ident, $constant:ident, $seek_height:expr) => {
        // Search the consensus version enacted at the specified height.
        $network::CONSENSUS_VERSION($seek_height).map_or(None, |seek_version| {
            // Search the consensus value for the specified version.
            match $network::$constant.binary_search_by(|(version, _)| version.cmp(&seek_version)) {
                // If a value was found for this consensus version, return it.
                Ok(index) => Some($network::$constant[index].1),
                // If the specified version was not found exactly, determine whether to return an appropriate value anyway.
                Err(index) => {
                    // This constant is not yet in effect at this consensus version.
                    if index == 0 {
                        None
                    // Return the appropriate value belonging to the consensus version *lower* than the sought version.
                    } else {
                        Some($network::$constant[index - 1].1)
                    }
                }
            }
        })
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanaryV0, MainnetV0, Network, TestnetV0};

    /// Ensure that the consensus constants are defined and correct at genesis.
    /// It is possible this invariant no longer holds in the future, e.g. due to pruning or novel types of constants.
    fn consensus_constants_at_genesis<N: Network>() {
        let height = N::_CONSENSUS_VERSION_HEIGHTS.first().unwrap().1;
        assert_eq!(height, 0);
        let consensus_version = N::_CONSENSUS_VERSION_HEIGHTS.first().unwrap().0;
        assert_eq!(consensus_version, ConsensusVersion::V1);
        assert_eq!(consensus_version as usize, 1);
    }

    /// Ensure that the consensus *versions* are unique, incrementing and start with 1.
    fn consensus_versions<N: Network>() {
        let mut previous_version = N::_CONSENSUS_VERSION_HEIGHTS.first().unwrap().0;
        // Ensure that the consensus versions start with 1.
        assert_eq!(previous_version as usize, 1);
        // Ensure that the consensus versions are unique and incrementing by 1.
        for (version, _) in N::_CONSENSUS_VERSION_HEIGHTS.iter().skip(1) {
            assert_eq!(*version as usize, previous_version as usize + 1);
            previous_version = *version;
        }
        // Ensure that the consensus versions are unique and incrementing.
        let mut previous_version = N::MAX_CERTIFICATES.first().unwrap().0;
        for (version, _) in N::MAX_CERTIFICATES.iter().skip(1) {
            assert!(*version > previous_version);
            previous_version = *version;
        }
        let mut previous_version = N::TRANSACTION_SPEND_LIMIT.first().unwrap().0;
        for (version, _) in N::TRANSACTION_SPEND_LIMIT.iter().skip(1) {
            assert!(*version > previous_version);
            previous_version = *version;
        }
    }

    /// Ensure that consensus *heights* are unique and incrementing.
    fn consensus_constants_increasing_heights<N: Network>() {
        let mut previous_height = N::CONSENSUS_VERSION_HEIGHTS().first().unwrap().1;
        for (version, height) in N::CONSENSUS_VERSION_HEIGHTS().iter().skip(1) {
            assert!(*height > previous_height);
            previous_height = *height;
            // Ensure that N::CONSENSUS_VERSION returns the expected value.
            assert_eq!(N::CONSENSUS_VERSION(*height).unwrap(), *version);
            // Ensure that N::CONSENSUS_HEIGHT returns the expected value.
            assert_eq!(N::CONSENSUS_HEIGHT(*version).unwrap(), *height);
        }
    }

    /// Ensure that version of all consensus-relevant constants are present in the consensus version heights.
    fn consensus_constants_valid_heights<N: Network>() {
        for (version, value) in N::MAX_CERTIFICATES.iter() {
            // Ensure that the height at which an update occurs are present in CONSENSUS_VERSION_HEIGHTS.
            let height = N::CONSENSUS_VERSION_HEIGHTS().iter().find(|(c_version, _)| *c_version == *version).unwrap().1;
            // Double-check that consensus_config_value returns the correct value.
            assert_eq!(consensus_config_value!(N, MAX_CERTIFICATES, height).unwrap(), *value);
        }
        for (version, value) in N::TRANSACTION_SPEND_LIMIT.iter() {
            // Ensure that the height at which an update occurs are present in CONSENSUS_VERSION_HEIGHTS.
            let height = N::CONSENSUS_VERSION_HEIGHTS().iter().find(|(c_version, _)| *c_version == *version).unwrap().1;
            // Double-check that consensus_config_value returns the correct value.
            assert_eq!(consensus_config_value!(N, TRANSACTION_SPEND_LIMIT, height).unwrap(), *value);
        }
    }

    /// Ensure that consensus_config_value returns a valid value for all consensus versions.
    fn consensus_config_returns_some<N: Network>() {
        for (_, height) in N::CONSENSUS_VERSION_HEIGHTS().iter() {
            assert!(consensus_config_value!(N, MAX_CERTIFICATES, *height).is_some());
            assert!(consensus_config_value!(N, TRANSACTION_SPEND_LIMIT, *height).is_some());
        }
    }

    /// Ensure that `MAX_CERTIFICATES` increases and is correctly defined.
    /// See the constant declaration for an explanation why.
    fn max_certificates_increasing<N: Network>() {
        let mut previous_value = N::MAX_CERTIFICATES.first().unwrap().1;
        for (_, value) in N::MAX_CERTIFICATES.iter().skip(1) {
            assert!(*value >= previous_value);
            previous_value = *value;
        }
    }

    /// Ensure that the number of constant definitions is the same across networks.
    fn constants_equal_length<N1: Network, N2: Network, N3: Network>() {
        // If we can construct an array, that means the underlying types must be the same.
        let _ = [N1::CONSENSUS_VERSION_HEIGHTS, N2::CONSENSUS_VERSION_HEIGHTS, N3::CONSENSUS_VERSION_HEIGHTS];
        let _ = [N1::MAX_CERTIFICATES, N2::MAX_CERTIFICATES, N3::MAX_CERTIFICATES];
        let _ = [N1::TRANSACTION_SPEND_LIMIT, N2::TRANSACTION_SPEND_LIMIT, N3::TRANSACTION_SPEND_LIMIT];
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_consensus_constants() {
        consensus_constants_at_genesis::<MainnetV0>();
        consensus_constants_at_genesis::<TestnetV0>();
        consensus_constants_at_genesis::<CanaryV0>();

        consensus_versions::<MainnetV0>();
        consensus_versions::<TestnetV0>();
        consensus_versions::<CanaryV0>();

        consensus_constants_increasing_heights::<MainnetV0>();
        consensus_constants_increasing_heights::<TestnetV0>();
        consensus_constants_increasing_heights::<CanaryV0>();

        consensus_constants_valid_heights::<MainnetV0>();
        consensus_constants_valid_heights::<TestnetV0>();
        consensus_constants_valid_heights::<CanaryV0>();

        consensus_config_returns_some::<MainnetV0>();
        consensus_config_returns_some::<TestnetV0>();
        consensus_config_returns_some::<CanaryV0>();

        max_certificates_increasing::<MainnetV0>();
        max_certificates_increasing::<TestnetV0>();
        max_certificates_increasing::<CanaryV0>();

        constants_equal_length::<MainnetV0, TestnetV0, CanaryV0>();
    }
}
