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

mod bitwise;
mod bytes;
mod from_bits;
mod serialize;
mod size_in_bits;
mod size_in_bytes;
mod to_address;
mod to_bits;
mod to_fields;
mod try_from;

#[cfg(feature = "private_key")]
use crate::PrivateKey;

use snarkvm_console_network::prelude::*;
use snarkvm_console_types::{Address, Boolean, Field, Group, Scalar};

static _COMPUTE_KEY_PREFIX: [u8; 10] = [109, 249, 98, 224, 36, 15, 213, 187, 79, 190]; // AComputeKey1

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ComputeKey<N: Network> {
    /// The signature public key `pk_sig` := G^sk_sig.
    pk_sig: Group<N>,
    /// The signature public randomizer `pr_sig` := G^r_sig.
    pr_sig: Group<N>,
    /// The PRF secret key `sk_prf` := HashToScalar(pk_sig || pr_sig).
    sk_prf: Scalar<N>,
}

impl<N: Network> ComputeKey<N> {
    /// Returns the signature public key.
    pub const fn pk_sig(&self) -> Group<N> {
        self.pk_sig
    }

    /// Returns the signature public randomizer.
    pub const fn pr_sig(&self) -> Group<N> {
        self.pr_sig
    }

    /// Returns a reference to the PRF secret key.
    pub const fn sk_prf(&self) -> Scalar<N> {
        self.sk_prf
    }
}
