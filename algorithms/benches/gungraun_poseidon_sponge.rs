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

//! Callgrind measurements of the Poseidon sponge absorbs in
//! `crypto_hash/poseidon.rs`.
//!
//! Each case runs once. Rayon is pinned to one worker so the instruction count
//! does not follow the pool size.

use snarkvm_algorithms::{AlgebraicSponge, crypto_hash::PoseidonSponge};
use snarkvm_curves::bls12_377::{Fq, FqParameters};
use snarkvm_fields::Fp384;
use snarkvm_utilities::{TestRng, Uniform};

use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

const SEED: u64 = 0x5906_3E00;

fn setup_native() -> (PoseidonSponge<Fq, 2, 1>, Vec<Fq>) {
    let mut rng = TestRng::from_seed(SEED);
    let input = (0..100).map(|_| Fq::rand(&mut rng)).collect();
    (PoseidonSponge::new(), input)
}

fn setup_nonnative() -> (PoseidonSponge<Fq, 2, 1>, Vec<Fp384<FqParameters>>) {
    let mut rng = TestRng::from_seed(SEED);
    let input = (0..100).map(|_| Fp384::<FqParameters>::rand(&mut rng)).collect();
    (PoseidonSponge::new(), input)
}

#[library_benchmark]
#[bench::absorb_100(setup_native())]
fn sponge_2_1_absorb_100_native(input: (PoseidonSponge<Fq, 2, 1>, Vec<Fq>)) -> PoseidonSponge<Fq, 2, 1> {
    let (mut sponge, input) = black_box(input);
    sponge.absorb_native_field_elements(&input);
    black_box(sponge)
}

#[library_benchmark]
#[bench::absorb_100(setup_nonnative())]
fn sponge_2_1_absorb_100_nonnative(
    input: (PoseidonSponge<Fq, 2, 1>, Vec<Fp384<FqParameters>>),
) -> PoseidonSponge<Fq, 2, 1> {
    let (mut sponge, input) = black_box(input);
    sponge.absorb_nonnative_field_elements(input);
    black_box(sponge)
}

library_benchmark_group!(name = sponge, benchmarks = [sponge_2_1_absorb_100_native, sponge_2_1_absorb_100_nonnative]);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = sponge
);
