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

//! BLS12-377 instantiation of the ProveKit spine (`Identity<Fr>`, base == ext).

mod bytes;
mod field;
mod field_hash;
mod transcript_sponge;

pub use field::Bls12_377Field;
pub use transcript_sponge::TranscriptSponge;

/// Register the BLS12-377 engines in WHIR's global registries.
///
/// Must be called once before any prove/verify operation. Idempotent.
pub fn register() {
    use std::sync::{Arc, Once};

    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let ntt: Arc<dyn whir::algebra::ntt::ReedSolomon<ark_bls12_377::Fr>> =
            Arc::new(whir::algebra::ntt::NttEngine::<ark_bls12_377::Fr>::new_from_fftfield());
        whir::algebra::ntt::NTT.insert(ntt);
    });
}
