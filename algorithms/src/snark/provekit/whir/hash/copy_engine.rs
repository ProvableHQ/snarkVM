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

use std::borrow::Cow;

use const_oid::ObjectIdentifier;
use hex_literal::hex;

use super::{Hash, HashEngine};
use crate::snark::provekit::whir::engines::EngineId;

pub const COPY: EngineId = EngineId::new(hex!("09459020f451874a1b399819d079632cc0f9263b1486c423173c6e15d8e2d61d"));

/// No-op hash engine that copies the input data without hashing it.
///
/// Requires the input data to be at most 32 bytes long.
#[derive(Clone, Copy, Debug, Default)]
pub struct Copy;

impl Copy {
    pub const fn new() -> Self {
        Self
    }
}

impl HashEngine for Copy {
    fn name(&self) -> Cow<'_, str> {
        "copy".into()
    }

    fn oid(&self) -> Option<ObjectIdentifier> {
        None
    }

    fn supports_size(&self, size: usize) -> bool {
        size <= 32
    }

    fn preferred_batch_size(&self) -> usize {
        1
    }

    fn hash_many(&self, size: usize, input: &[u8], output: &mut [Hash]) {
        assert!(size <= 32, "Copy engine only supports sizes up to 32 bytes");
        assert_eq!(
            input.len(),
            size * output.len(),
            "Input length should be size * output.len() = {size} * {}",
            output.len()
        );
        if size == 0 {
            output.fill(Hash([0; 32]));
            return;
        }
        for (input, out) in input.chunks_exact(size).zip(output.iter_mut()) {
            let mut bytes = [0; 32];
            bytes[..size].copy_from_slice(input);
            *out = Hash(bytes);
        }
    }
}

#[cfg(any())]
mod tests {

    use super::*;
    use crate::snark::provekit::whir::engines::Engine;

    #[test]
    fn test_protocol_ids() {
        assert_eq!(Copy::new().engine_id(), COPY);
    }
}
