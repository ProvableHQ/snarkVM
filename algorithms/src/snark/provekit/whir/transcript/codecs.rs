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

use static_assertions::assert_impl_all;
use zerocopy::{FromBytes, IntoBytes, KnownLayout};

use super::{Codec, Decoding, Encoding, NargDeserialize, VerificationResult};

/// An empty object. Like `()` with a `Codec`.
pub struct Empty;

impl<T> Encoding<[T]> for Empty {
    fn encode(&self) -> impl AsRef<[T]> {
        []
    }
}

impl<T> Decoding<[T]> for Empty {
    type Repr = [T; 0];

    fn decode(_buf: Self::Repr) -> Self {
        Self
    }
}

impl NargDeserialize for Empty {
    fn deserialize_from_narg(_buf: &mut &[u8]) -> VerificationResult<Self> {
        Ok(Self)
    }
}

assert_impl_all!(Empty: Codec);

/// Wrapper because spongefish is missing NargDeserialize for `u64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, FromBytes, KnownLayout, IntoBytes)]
#[repr(transparent)]
pub struct U64(pub u64);

impl Encoding<[u8]> for U64 {
    fn encode(&self) -> impl AsRef<[u8]> {
        self.0.to_le_bytes()
    }
}

impl Decoding<[u8]> for U64 {
    type Repr = [u8; 8];

    fn decode(buf: Self::Repr) -> Self {
        Self(u64::from_le_bytes(buf))
    }
}

impl NargDeserialize for U64 {
    fn deserialize_from_narg(buf: &mut &[u8]) -> VerificationResult<Self> {
        NargDeserialize::deserialize_from_narg(buf).map(u64::from_le_bytes).map(Self)
    }
}

assert_impl_all!(U64: Codec);
