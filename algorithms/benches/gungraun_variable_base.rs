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

//! Callgrind measurements of the variable-base MSMs in `msm/variable_base.rs`.
//!
//! Each size runs once. Rayon is pinned to one worker so the instruction count
//! does not follow the pool size.

use snarkvm_algorithms::msm::*;
use snarkvm_curves::{
    AffineCurve,
    bls12_377::{Fr as BlsFr, G1Affine, G1Projective},
    edwards_bls12::{EdwardsAffine, EdwardsProjective, Fr as EdwardsFr},
};
use snarkvm_fields::PrimeField;
use snarkvm_utilities::TestRng;

use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

const SEED: u64 = 0x560B_A5E0;

fn create_scalar_bases<G: AffineCurve<ScalarField = F>, F: PrimeField>(size: usize) -> (Vec<G>, Vec<F::BigInteger>) {
    let mut rng = TestRng::from_seed(SEED);

    let bases = std::iter::repeat_n((0..(size / 1000)).map(|_| G::rand(&mut rng)).collect::<Vec<_>>(), 1000)
        .flatten()
        .collect::<Vec<_>>();
    let scalars = (0..size).map(|_| F::rand(&mut rng).to_bigint()).collect::<Vec<_>>();
    (bases, scalars)
}

#[library_benchmark]
#[bench::size_10000(create_scalar_bases::<G1Affine, BlsFr>(10_000))]
#[bench::size_100000(create_scalar_bases::<G1Affine, BlsFr>(100_000))]
#[bench::size_1000000(create_scalar_bases::<G1Affine, BlsFr>(1_000_000))]
fn variable_base_bls12_377(input: (Vec<G1Affine>, Vec<<BlsFr as PrimeField>::BigInteger>)) -> G1Projective {
    let (bases, scalars) = black_box(input);
    black_box(VariableBase::msm(&bases, &scalars))
}

#[library_benchmark]
#[bench::size_10000(create_scalar_bases::<EdwardsAffine, EdwardsFr>(10_000))]
#[bench::size_100000(create_scalar_bases::<EdwardsAffine, EdwardsFr>(100_000))]
#[bench::size_1000000(create_scalar_bases::<EdwardsAffine, EdwardsFr>(1_000_000))]
fn variable_base_edwards_bls12(
    input: (Vec<EdwardsAffine>, Vec<<EdwardsFr as PrimeField>::BigInteger>),
) -> EdwardsProjective {
    let (bases, scalars) = black_box(input);
    black_box(VariableBase::msm(&bases, &scalars))
}

library_benchmark_group!(name = variable_base, benchmarks = [variable_base_bls12_377, variable_base_edwards_bls12]);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = variable_base
);
