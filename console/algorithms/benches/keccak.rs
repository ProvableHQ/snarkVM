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

#[macro_use]
extern crate criterion;

use snarkvm_console_algorithms::{Keccak256, Keccak384, Keccak512};
use snarkvm_console_types::prelude::*;
use snarkvm_utilities::{TestRng, Uniform};

use criterion::Criterion;

fn keccak256(c: &mut Criterion) {
    let rng = &mut TestRng::default();
    let hash = Keccak256::default();

    let input = (0..256).map(|_| bool::rand(rng)).collect::<Vec<_>>();
    c.bench_function(&format!("Keccak256 Hash - input size {}", input.len()), |b| b.iter(|| hash.hash(&input)));
}

fn keccak384(c: &mut Criterion) {
    let rng = &mut TestRng::default();
    let hash = Keccak384::default();

    let input = (0..256).map(|_| bool::rand(rng)).collect::<Vec<_>>();
    c.bench_function(&format!("Keccak384 Hash - input size {}", input.len()), |b| b.iter(|| hash.hash(&input)));
}

fn keccak512(c: &mut Criterion) {
    let rng = &mut TestRng::default();
    let hash = Keccak512::default();

    let input = (0..256).map(|_| bool::rand(rng)).collect::<Vec<_>>();
    c.bench_function(&format!("Keccak512 Hash - input size {}", input.len()), |b| b.iter(|| hash.hash(&input)));
}

criterion_group! {
    name = keccak;
    config = Criterion::default().sample_size(1000);
    targets = keccak256, keccak384, keccak512
}

criterion_main!(keccak);
