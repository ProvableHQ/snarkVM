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

#![cfg(test)]

use super::DuplexSpongeInterface;

#[derive(Clone, Debug)]
pub struct MockSponge<'a> {
    pub absorb: Option<&'a [u8]>,
    pub squeeze: &'a [u8],
}

impl DuplexSpongeInterface for MockSponge<'_> {
    type U = u8;

    fn absorb(&mut self, input: &[Self::U]) -> &mut Self {
        if let Some(absorb) = self.absorb.as_mut() {
            assert!(&absorb[..input.len()] == input);
            *absorb = &absorb[input.len()..];
        }
        self
    }

    fn squeeze(&mut self, output: &mut [Self::U]) -> &mut Self {
        output.copy_from_slice(&self.squeeze[..output.len()]);
        self.squeeze = &self.squeeze[output.len()..];
        self
    }

    fn ratchet(&mut self) -> &mut Self {
        if let Some(absorb) = self.absorb.as_mut() {
            assert!(&absorb[..7] == b"RATCHET");
            *absorb = &absorb[7..];
        }
        assert!(&self.squeeze[..7] == b"RATCHET");
        self.squeeze = &self.squeeze[7..];
        self
    }
}
