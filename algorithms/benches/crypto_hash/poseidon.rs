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

#[macro_use]
extern crate criterion;

use snarkvm_algorithms::{crypto_hash::PoseidonSponge, AlgebraicSponge};
use snarkvm_curves::bls12_377::{Fq, FqParameters};
use snarkvm_fields::Fp384;
use snarkvm_utilities::{TestRng, Uniform};

use criterion::Criterion;
use std::time::Duration;

fn sponge_2_1_absorb_100_native(c: &mut Criterion) {
    let rng = &mut TestRng::default();
    let mut sponge = PoseidonSponge::<Fq, 2, 1>::new();

    let mut input = Vec::with_capacity(100);
    for _ in 0..100 {
        input.push(Fq::rand(rng));
    }
    c.bench_function("PoseidonSponge<2, 1> Absorb 100 native", move |b| {
        b.iter(|| sponge.absorb_native_field_elements(&input))
    });
}

fn sponge_2_1_absorb_100_nonnative(c: &mut Criterion) {
    let rng = &mut TestRng::default();
    let mut sponge = PoseidonSponge::<Fq, 2, 1>::new();

    let mut input = Vec::with_capacity(100);
    for _ in 0..100 {
        input.push(Fp384::<FqParameters>::rand(rng));
    }
    c.bench_function("PoseidonSponge<2, 1> Absorb 100 nonnative", move |b| {
        b.iter(|| sponge.absorb_nonnative_field_elements(input.clone()))
    });
}

criterion_group! {
    name = sponge;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets = sponge_2_1_absorb_100_native, sponge_2_1_absorb_100_nonnative
}

criterion_main!(sponge);
