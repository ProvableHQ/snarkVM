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

use crate::polycommit::sonic_pc;
use snarkvm_curves::PairingEngine;
use snarkvm_utilities::{
    error,
    io::{self, Read, Write},
    serialize::*,
    FromBytes,
    ToBytes,
};

/// A certificate for the verifying key.
#[derive(Clone, Debug, PartialEq, Eq, CanonicalSerialize, CanonicalDeserialize)]
pub struct Certificate<E: PairingEngine> {
    /// An evaluation proof from the polynomial commitment.
    pub pc_proof: sonic_pc::BatchLCProof<E>,
}

impl<E: PairingEngine> Certificate<E> {
    /// Construct a new certificate.
    pub fn new(pc_proof: sonic_pc::BatchLCProof<E>) -> Self {
        Self { pc_proof }
    }
}

impl<E: PairingEngine> ToBytes for Certificate<E> {
    fn write_le<W: Write>(&self, mut w: W) -> io::Result<()> {
        Self::serialize_compressed(self, &mut w).map_err(|_| error("Failed to serialize certificate"))
    }
}

impl<E: PairingEngine> FromBytes for Certificate<E> {
    fn read_le<R: Read>(mut r: R) -> io::Result<Self> {
        Self::deserialize_compressed(&mut r).map_err(|_| error("Failed to deserialize certificate"))
    }
}
