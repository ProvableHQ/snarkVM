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

// Originally derived from ProveKit, Copyright 2026 World Foundation (MIT).

//! Runtime hash configuration selection for ProveKit.
//!
//! Runtime selection of hash algorithms used for:
//! - Merkle tree commitments (via WHIR's `EngineId`)
//! - the Fiat-Shamir transcript sponge (each backend's `TranscriptSponge`,
//!   built via [`crate::snark::provekit::common::FieldHash::transcript_sponge`])
//! - public-input instance binding (the field-element hashing lives in each
//!   backend's [`crate::snark::provekit::common::FieldHash::hash_public_inputs`])

use crate::snark::provekit::whir::engines::EngineId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Raw 32-byte engine ID for the Skyscraper hash engine.
///
/// Derived as `SHA3-256("crate::snark::provekit::whir::hash" || "skyscraper")`.
/// Protocol-visible metadata: the engine *implementation* lives in each
/// backend, but the ID is a field-agnostic tag so [`HashConfig::engine_id`] can
/// route to it.
pub const SKYSCRAPER_ENGINE_ID: [u8; 32] = [
    0xa5, 0x0d, 0x5e, 0xe2, 0xa3, 0xfc, 0x52, 0xe9, 0x6f, 0x11, 0x10, 0x3c, 0xbb, 0x8a, 0x65, 0xa3, 0x77, 0xb5, 0x82,
    0xb0, 0xb2, 0xdd, 0x42, 0x1c, 0x66, 0x19, 0x13, 0xe6, 0xa5, 0x63, 0xf8, 0xa1,
];

/// Pre-computed [`EngineId`] for the Skyscraper hash engine.
pub const SKYSCRAPER: EngineId = EngineId::new(SKYSCRAPER_ENGINE_ID);

/// Pre-computed [`EngineId`] for the Poseidon2 hash engine.
///
/// Derived as `SHA3-256("crate::snark::provekit::whir::hash" || "poseidon2")`.
pub const POSEIDON2: EngineId = EngineId::new([
    0xab, 0x4c, 0x81, 0x0f, 0x79, 0xdd, 0xf2, 0xa5, 0x4e, 0x81, 0x73, 0x1f, 0x97, 0x08, 0x23, 0x6a, 0xcf, 0x5c, 0xc2,
    0xcc, 0x35, 0xe9, 0xc4, 0x57, 0xa4, 0x35, 0x37, 0xdc, 0x01, 0x23, 0x6b, 0xb7,
]);

/// Hash algorithm configuration that can be selected at runtime.
///
/// Each variant selects the same algorithm for Merkle commitments,
/// Fiat-Shamir sponge, and public-input binding. [`Self::Skyscraper`] is the
/// default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HashConfig {
    #[default]
    #[serde(alias = "sky")]
    Skyscraper,

    #[serde(alias = "sha", alias = "sha-256")]
    Sha256,

    #[serde(alias = "keccak-256", alias = "shake")]
    Keccak,

    #[serde(alias = "blake-3", alias = "b3")]
    Blake3,

    #[serde(alias = "pos2", alias = "p2")]
    Poseidon2,
}

impl HashConfig {
    /// Returns the canonical name of this hash configuration.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Skyscraper => "skyscraper",
            Self::Sha256 => "sha256",
            Self::Keccak => "keccak",
            Self::Blake3 => "blake3",
            Self::Poseidon2 => "poseidon2",
        }
    }

    /// Returns the WHIR 2.0 engine ID for this hash configuration.
    #[must_use]
    pub fn engine_id(&self) -> crate::snark::provekit::whir::engines::EngineId {
        match self {
            Self::Skyscraper => SKYSCRAPER,
            Self::Sha256 => crate::snark::provekit::whir::hash::SHA2,
            Self::Keccak => crate::snark::provekit::whir::hash::KECCAK,
            Self::Blake3 => crate::snark::provekit::whir::hash::BLAKE3,
            Self::Poseidon2 => POSEIDON2,
        }
    }

    /// Converts hash configuration to a single byte for binary file headers.
    #[must_use]
    pub fn to_byte(&self) -> u8 {
        match self {
            Self::Skyscraper => 0,
            Self::Sha256 => 1,
            Self::Keccak => 2,
            Self::Blake3 => 3,
            Self::Poseidon2 => 4,
        }
    }

    /// Converts a byte from binary file header to hash configuration.
    #[must_use]
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Skyscraper),
            1 => Some(Self::Sha256),
            2 => Some(Self::Keccak),
            3 => Some(Self::Blake3),
            4 => Some(Self::Poseidon2),
            _ => None,
        }
    }

    /// Parses a hash configuration from a string.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let lower = s.to_lowercase();
        match lower.as_str() {
            "skyscraper" | "sky" => Some(Self::Skyscraper),
            "sha256" | "sha" | "sha-256" => Some(Self::Sha256),
            "keccak" | "keccak-256" | "shake" => Some(Self::Keccak),
            "blake3" | "blake-3" | "b3" => Some(Self::Blake3),
            "poseidon2" | "pos2" | "p2" => Some(Self::Poseidon2),
            _ => None,
        }
    }
}

impl fmt::Display for HashConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl std::str::FromStr for HashConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| {
            format!(
                "Invalid hash configuration: '{s}'. Valid options: skyscraper, sha256, keccak, \
                 blake3, poseidon2"
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All known variants. If a new variant is added to `HashConfig`, this
    /// list must be updated — causing the exhaustiveness tests below to fail
    /// until `from_byte` / `to_byte` are also updated.
    const ALL_VARIANTS: &[HashConfig] =
        &[HashConfig::Skyscraper, HashConfig::Sha256, HashConfig::Keccak, HashConfig::Blake3, HashConfig::Poseidon2];

    #[test]
    fn from_byte_roundtrips_with_to_byte() {
        for &variant in ALL_VARIANTS {
            let byte = variant.to_byte();
            let recovered = HashConfig::from_byte(byte)
                .unwrap_or_else(|| panic!("from_byte({byte}) returned None for {variant:?}"));
            assert_eq!(variant, recovered, "roundtrip failed for {variant:?}");
        }
    }

    #[test]
    fn from_byte_returns_none_for_invalid() {
        let first_invalid = ALL_VARIANTS.len() as u8;
        assert!(HashConfig::from_byte(first_invalid).is_none(), "from_byte({first_invalid}) should be None");
        assert!(HashConfig::from_byte(u8::MAX).is_none(), "from_byte(255) should be None");
    }

    #[test]
    fn to_byte_values_are_contiguous_from_zero() {
        let mut bytes: Vec<u8> = ALL_VARIANTS.iter().map(|v| v.to_byte()).collect();
        bytes.sort();
        let expected: Vec<u8> = (0..ALL_VARIANTS.len() as u8).collect();
        assert_eq!(bytes, expected, "to_byte values should be 0..N contiguous");
    }

    #[test]
    fn from_byte_covers_all_variants() {
        let recovered: Vec<HashConfig> = (0..=u8::MAX).filter_map(HashConfig::from_byte).collect();
        for &variant in ALL_VARIANTS {
            assert!(recovered.contains(&variant), "{variant:?} is not reachable via from_byte");
        }
        assert_eq!(recovered.len(), ALL_VARIANTS.len(), "from_byte maps to more variants than ALL_VARIANTS lists");
    }
}
