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

//! Canonical little-endian byte bridges for BLS12-377 `Fr`.

use ark_bls12_377::Fr;
use ark_ff::PrimeField;

/// Serializes a BLS12-377 scalar to its canonical 32-byte little-endian
/// representation.
pub(super) fn field_to_bytes_le(fe: Fr) -> [u8; 32] {
    let limbs = fe.into_bigint().0;
    let mut out = [0u8; 32];
    for (i, &limb) in limbs.iter().enumerate() {
        out[i * 8..(i + 1) * 8].copy_from_slice(&limb.to_le_bytes());
    }
    out
}
