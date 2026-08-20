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

use snarkvm_curves::bls12_377::Fr;
use snarkvm_utilities::ToBytes;

/// Serializes a BLS12-377 scalar to its canonical 32-byte little-endian
/// representation.
pub(super) fn field_to_bytes_le(fe: Fr) -> [u8; 32] {
    let mut out = [0u8; 32];
    fe.write_le(&mut out[..]).expect("BLS12-377 Fr is 32 little-endian bytes");
    out
}
