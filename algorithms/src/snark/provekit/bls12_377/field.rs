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

//! BLS12-377 proof field: `Identity<Fr>` (base == ext).

use super::{TranscriptSponge, bytes::field_to_bytes_le};
use crate::snark::provekit::common::{Base, Ext, FieldHash, HashConfig, ProofField};
use ark_bls12_377::Fr;
use whir::algebra::embedding::Identity;

/// BLS12-377 proof field: the `Identity<Fr>` embedding (base == ext).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bls12_377Field;

impl ProofField for Bls12_377Field {
    type Embedding = Identity<Fr>;

    /// Distinct from ProveKit's BN254 (`0`) and Goldilocks (`1`, `2`) tags.
    const FIELD_ID: u8 = 3;
}

impl FieldHash for Bls12_377Field {
    type Sponge = TranscriptSponge;

    fn hash_public_inputs(config: HashConfig, inputs: &[Base<Self>]) -> Ext<Self> {
        super::field_hash::hash_field_elements(config, inputs)
    }

    fn ext_to_bytes_le(x: &Ext<Self>) -> Vec<u8> {
        field_to_bytes_le(*x).to_vec()
    }

    fn transcript_sponge(config: HashConfig) -> Self::Sponge {
        TranscriptSponge::from_config(config)
    }
}
