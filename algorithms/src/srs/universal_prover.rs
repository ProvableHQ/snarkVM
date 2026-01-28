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
    fft::{
        EvaluationDomain,
        domain::{FFTPrecomputation, IFFTPrecomputation},
    },
    polycommit::{kzg10::DegreeInfo, sonic_pc::CommitterKey},
    snark::varuna::UniversalSRS,
};
use snarkvm_curves::PairingEngine;

use anyhow::{Result, anyhow};

/// `UniversalProver` is used to compute evaluation proofs for a given
/// commitment.
#[derive(Debug)]
pub struct UniversalProver<E: PairingEngine> {
    /// The committer key for the underlying KZG10 scheme.
    pub committer_key: CommitterKey<E>,
    /// The precomputed roots for calculating FFTs.
    pub fft_precomputation: Option<FFTPrecomputation<E::Fr>>,
    /// The precomputed inverse roots for calculating inverse FFTs.
    pub ifft_precomputation: Option<IFFTPrecomputation<E::Fr>>,
    /// The maximum degree supported by the universal SRS.
    pub max_degree: usize,
}

impl<E: PairingEngine> Default for UniversalProver<E> {
    fn default() -> Self {
        let committer_key = CommitterKey {
            powers_of_beta_g: Vec::new(),
            lagrange_bases_at_beta_g: None,
            powers_of_beta_times_gamma_g: Vec::new(),
            shifted_powers_of_beta_g: None,
            shifted_powers_of_beta_times_gamma_g: None,
            enforced_degree_bounds: None,
        };
        Self { committer_key, fft_precomputation: None, ifft_precomputation: None, max_degree: 0 }
    }
}

impl<E: PairingEngine> UniversalProver<E> {
    /// Update the UniversalProver:
    /// - set a new max_degree
    /// - grow or shrink the committer_key
    /// - grow or shrink the FFT and iFFT precomputation
    ///
    /// Note that this implementation is not atomic. If specialize fails halfway
    /// through, we might have an inconsistent UniversalProver.
    pub fn update(&mut self, srs: &UniversalSRS<E>, degree_info: DegreeInfo) -> Result<()> {
        // Set the new max_degree
        self.max_degree = degree_info.max_degree;
        // Update the committer key.
        self.committer_key.update(srs, &degree_info)?;
        // Specialize the FFT and iFFT precomputation.
        self.update_fft_precomputation(degree_info.max_fft_size)?;
        Ok(())
    }

    /// Returns the FFT and iFFT precomputation for the largest supported
    /// domain.
    fn update_fft_precomputation(&mut self, max_domain_size: usize) -> Result<()> {
        let domain = EvaluationDomain::new(max_domain_size).ok_or(anyhow!("Invalid domain size"))?;
        self.fft_precomputation = Some(domain.precompute_fft());
        self.ifft_precomputation = Some(self.fft_precomputation.as_ref().unwrap().to_ifft_precomputation());
        Ok(())
    }
}
