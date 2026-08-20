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

//! Public-input instance hashing over BLS12-377 `Fr`.

use super::bytes::field_to_bytes_le;
use crate::snark::provekit::common::HashConfig;
use ark_bls12_377::Fr;
use ark_ff::PrimeField;

/// Domain-separation tag for public-input instance binding.
const PUBLIC_INPUTS_DST: &[u8] = b"PROVEKIT_PUBLIC_INPUTS_V1";

/// Hashes `elements` into a single field element under `config`.
pub(super) fn hash_field_elements(config: HashConfig, elements: &[Fr]) -> Fr {
    match config {
        HashConfig::Sha256 => hash_sha256(PUBLIC_INPUTS_DST, elements),
        HashConfig::Keccak => hash_keccak(PUBLIC_INPUTS_DST, elements),
        HashConfig::Blake3 => hash_blake3(PUBLIC_INPUTS_DST, elements),
        HashConfig::Skyscraper | HashConfig::Poseidon2 => {
            panic!("HashConfig::{config:?} is not supported over BLS12-377")
        }
    }
}

fn hash_sha256(dst: &[u8], elements: &[Fr]) -> Fr {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(dst);
    for fe in elements {
        hasher.update(field_to_bytes_le(*fe));
    }
    Fr::from_le_bytes_mod_order(&hasher.finalize())
}

fn hash_keccak(dst: &[u8], elements: &[Fr]) -> Fr {
    use sha3::Digest;
    let mut hasher = sha3::Keccak256::new();
    hasher.update(dst);
    for fe in elements {
        hasher.update(field_to_bytes_le(*fe));
    }
    Fr::from_le_bytes_mod_order(&hasher.finalize())
}

fn hash_blake3(dst: &[u8], elements: &[Fr]) -> Fr {
    let mut hasher = blake3::Hasher::new();
    hasher.update(dst);
    for fe in elements {
        hasher.update(&field_to_bytes_le(*fe));
    }
    Fr::from_le_bytes_mod_order(hasher.finalize().as_bytes())
}
