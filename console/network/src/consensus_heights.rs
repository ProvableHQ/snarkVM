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
pub const CANARY_V0_CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); 8] = [
    (ConsensusVersion::V1, 0),
    (ConsensusVersion::V2, 2_900_000),
    (ConsensusVersion::V3, 4_560_000),
    (ConsensusVersion::V4, 5_730_000),
    (ConsensusVersion::V5, 5_780_000),
    (ConsensusVersion::V6, 6_240_000),
    (ConsensusVersion::V7, 6_880_000),
    (ConsensusVersion::V8, 7_565_000),
];

/// The consensus version height for `MainnetV0`.
pub const MAINNET_V0_CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); 8] = [
    (ConsensusVersion::V1, 0),
    (ConsensusVersion::V2, 2_800_000),
    (ConsensusVersion::V3, 4_900_000),
    (ConsensusVersion::V4, 6_135_000),
    (ConsensusVersion::V5, 7_060_000),
    (ConsensusVersion::V6, 7_560_000),
    (ConsensusVersion::V7, 7_570_000),
    (ConsensusVersion::V8, 9_417_000),
];

/// The consensus version heights for `TestnetV0`.
pub const TESTNET_V0_CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); 8] = [
    (ConsensusVersion::V1, 0),
    (ConsensusVersion::V2, 2_950_000),
    (ConsensusVersion::V3, 4_800_000),
    (ConsensusVersion::V4, 6_625_000),
    (ConsensusVersion::V5, 6_765_000),
    (ConsensusVersion::V6, 7_600_000),
    (ConsensusVersion::V7, 8_365_000),
    (ConsensusVersion::V8, 9_155_000),
];

/// The consensus version heights for `TestConsensusV0`.
pub const TEST_CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); 8] = [
    (ConsensusVersion::V1, 0),
    (ConsensusVersion::V2, 10),
    (ConsensusVersion::V3, 11),
    (ConsensusVersion::V4, 12),
    (ConsensusVersion::V5, 13),
    (ConsensusVersion::V6, 14),
    (ConsensusVersion::V7, 15),
    (ConsensusVersion::V8, 16),
];
