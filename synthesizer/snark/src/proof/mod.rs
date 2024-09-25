// Copyright 2024 Aleo Network Foundation
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

use super::*;

mod bytes;
mod parse;
mod serialize;

#[derive(Clone, PartialEq, Eq)]
pub struct Proof<N: Network> {
    /// The proof.
    proof: varuna::Proof<N::PairingCurve>,
}

impl<N: Network> Proof<N> {
    /// Initializes a new proof.
    pub const fn new(proof: varuna::Proof<N::PairingCurve>) -> Self {
        Self { proof }
    }
}

impl<N: Network> Deref for Proof<N> {
    type Target = varuna::Proof<N::PairingCurve>;

    fn deref(&self) -> &Self::Target {
        &self.proof
    }
}
