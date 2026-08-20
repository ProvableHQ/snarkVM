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

mod blake3_engine;
mod copy_engine;
mod digest_engine;
mod hash_counter;

use core::fmt;
use std::{
    borrow::Cow,
    sync::{Arc, LazyLock},
};

use const_oid::ObjectIdentifier;
use serde::{Deserialize, Serialize};
use static_assertions::{assert_impl_all, assert_obj_safe};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};

pub use self::{
    blake3_engine::{BLAKE3, Blake3},
    copy_engine::{COPY, Copy},
    digest_engine::{DigestEngine, KECCAK, Keccak, SHA2, SHA3, Sha2, Sha3},
    hash_counter::HASH_COUNTER,
};
use crate::snark::provekit::whir::{
    engines::{self, EngineId, Engines},
    transcript::{Encoding, NargDeserialize, ProverMessage, VerificationError, VerificationResult},
};

pub static ENGINES: LazyLock<Engines<dyn HashEngine>> = LazyLock::new(|| {
    let engines = Engines::<dyn HashEngine>::new();
    engines.register(Arc::new(Copy::new()));
    engines.register(Arc::new(Sha2::new()));
    engines.register(Arc::new(Keccak::new()));
    engines.register(Arc::new(Sha3::new()));
    engines.register(Arc::new(Blake3::detect()));
    engines
});

#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Serialize,
    Deserialize,
    KnownLayout,
    Immutable,
    Unaligned,
    FromBytes,
    IntoBytes,
)]
#[repr(transparent)]
pub struct Hash(pub [u8; 32]);

pub trait HashEngine: Send + Sync {
    fn name(&self) -> Cow<'_, str>;

    fn oid(&self) -> Option<ObjectIdentifier> {
        None
    }

    /// Check if the engine supports hashing messages of the given size.
    fn supports_size(&self, size: usize) -> bool;

    /// The number of messages that should be hashed together to achieve
    /// maximally utilize single thread parallelism.
    ///
    /// The caller should attermpt to call `hash_many` with multiples of this
    /// size for optimal performance, e.g. when dividing work over threads.
    ///
    /// Regardless, all batch sizes must be supported.
    fn preferred_batch_size(&self) -> usize {
        1
    }

    /// Hash many messages of size `size`.
    ///
    /// Input contains `output.len()` messages concatenated together.
    ///
    /// Note: Implementation should be single-threaded. Parallelization should
    /// be taken care of by the caller.
    fn hash_many(&self, size: usize, input: &[u8], output: &mut [Hash]);
}

impl<E: HashEngine + ?Sized> engines::Engine for E {
    fn engine_id(&self) -> EngineId {
        use digest::Digest;
        use sha3::Sha3_256;

        let mut hasher = Sha3_256::new();
        hasher.update(b"whir::hash");
        if let Some(oid) = self.oid() {
            hasher.update(oid.as_bytes());
        } else {
            hasher.update(self.name().as_bytes());
        }
        let hash: [u8; 32] = hasher.finalize().into();
        hash.into()
    }
}

assert_obj_safe!(HashEngine);

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl Encoding<[u8]> for Hash {
    fn encode(&self) -> impl AsRef<[u8]> {
        self.as_bytes()
    }
}

impl NargDeserialize for Hash {
    fn deserialize_from_narg(buf: &mut &[u8]) -> VerificationResult<Self> {
        let (hash, tail) = Self::read_from_prefix(buf).map_err(|_| VerificationError)?;
        *buf = tail;
        Ok(hash)
    }
}

assert_impl_all!(Hash: ProverMessage);

#[cfg(any())]
pub(crate) mod tests {
    use proptest::{sample::select, strategy::Strategy};

    use super::*;
    use crate::snark::provekit::whir::{
        engines::EngineId,
        hash::{BLAKE3, KECCAK},
    };

    const HASHES: [EngineId; 5] = [COPY, SHA2, SHA3, KECCAK, BLAKE3];

    pub fn hash_for_size(size: usize) -> impl Strategy<Value = EngineId> {
        let suitable = HASHES
            .iter()
            .copied()
            .filter(|h| ENGINES.retrieve(*h).is_some_and(|h| h.supports_size(size)))
            .collect::<Vec<_>>();
        select(suitable)
    }
}
