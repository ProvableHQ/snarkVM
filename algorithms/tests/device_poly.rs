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

//! Round four's device copies across whole proofs. The only test in its
//! process, so the process-wide counters move only with it.

#![cfg(all(feature = "cuda", feature = "test", target_arch = "x86_64"))]

use snarkvm_algorithms::{
    crypto_hash::PoseidonSponge,
    snark::varuna::{AHPForR1CS, TestCircuit, VarunaHidingMode, VarunaSNARK, VarunaVersion},
    traits::{AlgebraicSponge, SNARK},
};
use snarkvm_algorithms_cuda::device_poly::stats;
use snarkvm_curves::bls12_377::{Bls12_377, Fq, Fr};
use snarkvm_utilities::rand::TestRng;

type FS = PoseidonSponge<Fq, 2, 1>;
type Inst = VarunaSNARK<Bls12_377, FS, VarunaHidingMode>;

const N: usize = 2000;

/// Each proof keeps `a`, `b` and `g` for its three matrices, reads every copy,
/// and frees them all before it returns.
#[test]
fn a_proof_reads_and_frees_its_device_copies() {
    if snarkvm_algorithms_cuda::device_count() != 1 {
        return;
    }
    let rng = &mut TestRng::fixed(1);
    let (circuit, inputs) = TestCircuit::gen_rand(1, N, N, rng);
    let srs = Inst::universal_setup(AHPForR1CS::<Fr, VarunaHidingMode>::max_degree(N, N, N).unwrap()).unwrap();
    let (prover, verifier) = (srs.to_universal_prover().unwrap(), srs.to_universal_verifier().unwrap());
    let fs = FS::sample_parameters();
    let (pk, vk) = Inst::circuit_setup(&srs, &circuit).unwrap();
    for version in [VarunaVersion::V2, VarunaVersion::V3] {
        let before = stats();
        let proof = Inst::prove(&prover, &fs, &pk, version, &circuit, rng).unwrap();
        let after = stats();
        assert!(Inst::verify(&verifier, &fs, &vk, version, inputs.as_slice(), &proof).unwrap(), "{version:?}");
        assert_eq!(after.retained - before.retained, 3, "{version:?}");
        assert_eq!(after.device_reads - before.device_reads, 12, "{version:?}");
        assert_eq!((after.retain_failures, after.host_fallbacks), (0, 0), "{version:?}");
        assert_eq!(after.held_bytes, before.held_bytes, "{version:?}: the proof left device copies behind");
    }
}
