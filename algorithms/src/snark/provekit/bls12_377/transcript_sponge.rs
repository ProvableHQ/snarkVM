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

//! Runtime-selectable Fiat-Shamir transcript sponge for BLS12-377.

use crate::snark::provekit::common::HashConfig;
use spongefish::{DuplexSpongeInterface, instantiations};
use std::fmt;

/// Fiat-Shamir transcript sponge, selected at runtime by [`HashConfig`].
#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum TranscriptSponge {
    Sha256(instantiations::SHA256),
    Blake3(instantiations::Blake3),
    Keccak(instantiations::Keccak),
}

impl fmt::Debug for TranscriptSponge {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Sha256(_) => f.debug_tuple("Sha256").finish(),
            Self::Blake3(_) => f.debug_tuple("Blake3").finish(),
            Self::Keccak(_) => f.debug_tuple("Keccak").finish(),
        }
    }
}

impl TranscriptSponge {
    /// Create a sponge matching the given hash configuration.
    ///
    /// # Panics
    ///
    /// Panics for [`HashConfig::Skyscraper`] and [`HashConfig::Poseidon2`],
    /// which are not defined over BLS12-377.
    pub fn from_config(config: HashConfig) -> Self {
        match config {
            HashConfig::Sha256 => Self::Sha256(Default::default()),
            HashConfig::Blake3 => Self::Blake3(Default::default()),
            HashConfig::Keccak => Self::Keccak(Default::default()),
            HashConfig::Skyscraper | HashConfig::Poseidon2 => {
                panic!("HashConfig::{config:?} is not supported over BLS12-377")
            }
        }
    }
}

impl DuplexSpongeInterface for TranscriptSponge {
    type U = u8;

    fn absorb(&mut self, input: &[u8]) -> &mut Self {
        match self {
            Self::Sha256(s) => {
                s.absorb(input);
            }
            Self::Blake3(s) => {
                s.absorb(input);
            }
            Self::Keccak(s) => {
                s.absorb(input);
            }
        }
        self
    }

    fn squeeze(&mut self, output: &mut [u8]) -> &mut Self {
        match self {
            Self::Sha256(s) => {
                s.squeeze(output);
            }
            Self::Blake3(s) => {
                s.squeeze(output);
            }
            Self::Keccak(s) => {
                s.squeeze(output);
            }
        }
        self
    }

    fn ratchet(&mut self) -> &mut Self {
        match self {
            Self::Sha256(s) => {
                s.ratchet();
            }
            Self::Blake3(s) => {
                s.ratchet();
            }
            Self::Keccak(s) => {
                s.ratchet();
            }
        }
        self
    }
}
