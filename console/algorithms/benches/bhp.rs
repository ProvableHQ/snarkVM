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

use snarkvm_console_algorithms::{BHP1024, BHP256, BHP512, BHP768};
use snarkvm_console_types::prelude::*;
use snarkvm_utilities::{TestRng, Uniform};

use criterion::Criterion;

fn bhp256(c: &mut Criterion) {
    let rng = &mut TestRng::default();
    let hash = BHP256::<Console>::setup("BHP256").unwrap();

    let input = (0..hash.window_size() as u64 * hash.num_windows() as u64).map(|_| bool::rand(rng)).collect::<Vec<_>>();
    c.bench_function(&format!("BHP256 Hash - input size {}", input.len()), |b| b.iter(|| hash.hash(&input)));

    let input = (0..256).map(|_| bool::rand(rng)).collect::<Vec<_>>();
    c.bench_function(&format!("BHP256 Hash - input size {}", input.len()), |b| b.iter(|| hash.hash(&input)));
}

fn bhp512(c: &mut Criterion) {
    let rng = &mut TestRng::default();
    let hash = BHP512::<Console>::setup("BHP512").unwrap();

    let input = (0..hash.window_size() as u64 * hash.num_windows() as u64).map(|_| bool::rand(rng)).collect::<Vec<_>>();
    c.bench_function(&format!("BHP512 Hash - input size {}", input.len()), |b| b.iter(|| hash.hash(&input)));

    let input = (0..512).map(|_| bool::rand(rng)).collect::<Vec<_>>();
    c.bench_function(&format!("BHP512 Hash - input size {}", input.len()), |b| b.iter(|| hash.hash(&input)));
}

fn bhp768(c: &mut Criterion) {
    let rng = &mut TestRng::default();
    let hash = BHP768::<Console>::setup("BHP768").unwrap();

    let input = (0..hash.window_size() as u64 * hash.num_windows() as u64).map(|_| bool::rand(rng)).collect::<Vec<_>>();
    c.bench_function(&format!("BHP768 Hash - input size {}", input.len()), |b| b.iter(|| hash.hash(&input)));

    let input = (0..768).map(|_| bool::rand(rng)).collect::<Vec<_>>();
    c.bench_function(&format!("BHP768 Hash - input size {}", input.len()), |b| b.iter(|| hash.hash(&input)));
}

fn bhp1024(c: &mut Criterion) {
    let rng = &mut TestRng::default();
    let hash = BHP1024::<Console>::setup("BHP1024").unwrap();

    let input = (0..hash.window_size() as u64 * hash.num_windows() as u64).map(|_| bool::rand(rng)).collect::<Vec<_>>();
    c.bench_function(&format!("BHP1024 Hash - input size {}", input.len()), |b| b.iter(|| hash.hash(&input)));

    let input = (0..1024).map(|_| bool::rand(rng)).collect::<Vec<_>>();
    c.bench_function(&format!("BHP1024 Hash - input size {}", input.len()), |b| b.iter(|| hash.hash(&input)));
}

criterion_group! {
    name = bhp;
    config = Criterion::default().sample_size(1000);
    targets = bhp256, bhp512, bhp768, bhp1024
}

criterion_main!(bhp);
