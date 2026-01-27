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

use crate::{
    fft::domain::{FFTPrecomputation, IFFTPrecomputation},
    polycommit::sonic_pc::CommitterKey,
};
use snarkvm_curves::PairingEngine;

/// `UniversalProver` is used to compute evaluation proofs for a given
/// commitment.
#[derive(Debug)]
pub struct UniversalProver<E: PairingEngine> {
    /// The committer key for the underlying KZG10 scheme.
    pub committer_key: CommitterKey<E>,
    /// The precomputed roots for calculating FFTs.
    pub fft_precomputation: FFTPrecomputation<E::Fr>,
    /// The precomputed inverse roots for calculating inverse FFTs.
    pub ifft_precomputation: IFFTPrecomputation<E::Fr>,
    /// The maximum degree supported by the universal SRS.
    pub max_degree: usize,
}
