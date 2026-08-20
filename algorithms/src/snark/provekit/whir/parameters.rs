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

use std::fmt::{Debug, Display};

use serde::{Deserialize, Serialize};

use crate::snark::provekit::whir::engines::EngineId;

/// Configuration parameters for WHIR proofs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolParameters {
    /// Whether to require unique decoding.
    pub unique_decoding: bool,
    /// The logarithmic inverse rate for sampling.
    pub starting_log_inv_rate: usize,
    /// Folding factor for the initial round.
    pub initial_folding_factor: usize,
    /// Folding factor for rounds after the initial round.
    pub folding_factor: usize,
    /// The security level in bits.
    pub security_level: usize,
    /// The maximum number of bits required for proof-of-work (PoW).
    pub pow_bits: usize,
    /// Number of vectors committed in the batch.
    pub batch_size: usize,
    /// Hash function identifier.
    pub hash_id: EngineId,
}

impl Display for ProtocolParameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Targeting {}-bits of security with {}-bits of PoW using {} decoding",
            self.security_level,
            self.pow_bits,
            if self.unique_decoding { "unique" } else { "list" }
        )?;
        writeln!(
            f,
            "Starting rate: 2^-{}, initial_folding_factor: {}, folding_factor: {}",
            self.starting_log_inv_rate, self.initial_folding_factor, self.folding_factor,
        )
    }
}
