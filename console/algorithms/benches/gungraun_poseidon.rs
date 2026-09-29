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

//! Callgrind measurements of the Poseidon hashes in `poseidon.rs`.
//!
//! Each case runs once. Rayon is pinned to one worker so the instruction count does not follow the pool size.

use snarkvm_console_algorithms::{Poseidon2, Poseidon4, Poseidon8};
use snarkvm_console_types::prelude::*;
use snarkvm_utilities::{TestRng, Uniform};

use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

type F = Field<Console>;

const SEED: u64 = 0x905E_1D00;

fn field_array() -> [F; 4] {
    let mut rng = TestRng::from_seed(SEED);
    [F::rand(&mut rng), F::rand(&mut rng), F::rand(&mut rng), F::rand(&mut rng)]
}

fn field_vec() -> Vec<F> {
    let mut rng = TestRng::from_seed(SEED);
    (0..10).map(|_| F::rand(&mut rng)).collect()
}

fn setup_poseidon2_4() -> (Poseidon2<Console>, [F; 4]) {
    (Poseidon2::setup("Poseidon2").unwrap(), field_array())
}

fn setup_poseidon2_10() -> (Poseidon2<Console>, Vec<F>) {
    (Poseidon2::setup("Poseidon2").unwrap(), field_vec())
}

fn setup_poseidon4_4() -> (Poseidon4<Console>, [F; 4]) {
    (Poseidon4::setup("Poseidon4").unwrap(), field_array())
}

fn setup_poseidon4_10() -> (Poseidon4<Console>, Vec<F>) {
    (Poseidon4::setup("Poseidon4").unwrap(), field_vec())
}

fn setup_poseidon8_4() -> (Poseidon8<Console>, [F; 4]) {
    (Poseidon8::setup("Poseidon8").unwrap(), field_array())
}

fn setup_poseidon8_10() -> (Poseidon8<Console>, Vec<F>) {
    (Poseidon8::setup("Poseidon8").unwrap(), field_vec())
}

#[library_benchmark]
#[bench::hash(setup_poseidon2_4())]
fn poseidon2_hash_4_to_1(input: (Poseidon2<Console>, [F; 4])) -> F {
    let (hash, input) = black_box(input);
    black_box(hash.hash(&input).unwrap())
}

#[library_benchmark]
#[bench::hash(setup_poseidon2_4())]
fn poseidon2_hash_4_to_2(input: (Poseidon2<Console>, [F; 4])) -> Vec<F> {
    let (hash, input) = black_box(input);
    black_box(hash.hash_many(&input, 2))
}

#[library_benchmark]
#[bench::hash(setup_poseidon2_10())]
fn poseidon2_hash_10_to_1(input: (Poseidon2<Console>, Vec<F>)) -> F {
    let (hash, input) = black_box(input);
    black_box(hash.hash(&input).unwrap())
}

#[library_benchmark]
#[bench::hash(setup_poseidon2_10())]
fn poseidon2_hash_10_to_4(input: (Poseidon2<Console>, Vec<F>)) -> Vec<F> {
    let (hash, input) = black_box(input);
    black_box(hash.hash_many(&input, 4))
}

#[library_benchmark]
#[bench::hash(setup_poseidon2_10())]
fn poseidon2_hash_10_to_8(input: (Poseidon2<Console>, Vec<F>)) -> Vec<F> {
    let (hash, input) = black_box(input);
    black_box(hash.hash_many(&input, 8))
}

#[library_benchmark]
#[bench::hash(setup_poseidon4_4())]
fn poseidon4_hash_4_to_1(input: (Poseidon4<Console>, [F; 4])) -> F {
    let (hash, input) = black_box(input);
    black_box(hash.hash(&input).unwrap())
}

#[library_benchmark]
#[bench::hash(setup_poseidon4_4())]
fn poseidon4_hash_4_to_2(input: (Poseidon4<Console>, [F; 4])) -> Vec<F> {
    let (hash, input) = black_box(input);
    black_box(hash.hash_many(&input, 2))
}

#[library_benchmark]
#[bench::hash(setup_poseidon4_10())]
fn poseidon4_hash_10_to_1(input: (Poseidon4<Console>, Vec<F>)) -> F {
    let (hash, input) = black_box(input);
    black_box(hash.hash(&input).unwrap())
}

#[library_benchmark]
#[bench::hash(setup_poseidon4_10())]
fn poseidon4_hash_10_to_4(input: (Poseidon4<Console>, Vec<F>)) -> Vec<F> {
    let (hash, input) = black_box(input);
    black_box(hash.hash_many(&input, 4))
}

#[library_benchmark]
#[bench::hash(setup_poseidon4_10())]
fn poseidon4_hash_10_to_8(input: (Poseidon4<Console>, Vec<F>)) -> Vec<F> {
    let (hash, input) = black_box(input);
    black_box(hash.hash_many(&input, 8))
}

#[library_benchmark]
#[bench::hash(setup_poseidon8_4())]
fn poseidon8_hash_4_to_1(input: (Poseidon8<Console>, [F; 4])) -> F {
    let (hash, input) = black_box(input);
    black_box(hash.hash(&input).unwrap())
}

#[library_benchmark]
#[bench::hash(setup_poseidon8_4())]
fn poseidon8_hash_4_to_2(input: (Poseidon8<Console>, [F; 4])) -> Vec<F> {
    let (hash, input) = black_box(input);
    black_box(hash.hash_many(&input, 2))
}

#[library_benchmark]
#[bench::hash(setup_poseidon8_10())]
fn poseidon8_hash_10_to_1(input: (Poseidon8<Console>, Vec<F>)) -> F {
    let (hash, input) = black_box(input);
    black_box(hash.hash(&input).unwrap())
}

#[library_benchmark]
#[bench::hash(setup_poseidon8_10())]
fn poseidon8_hash_10_to_4(input: (Poseidon8<Console>, Vec<F>)) -> Vec<F> {
    let (hash, input) = black_box(input);
    black_box(hash.hash_many(&input, 4))
}

#[library_benchmark]
#[bench::hash(setup_poseidon8_10())]
fn poseidon8_hash_10_to_8(input: (Poseidon8<Console>, Vec<F>)) -> Vec<F> {
    let (hash, input) = black_box(input);
    black_box(hash.hash_many(&input, 8))
}

library_benchmark_group!(
    name = poseidon,
    benchmarks = [
        poseidon2_hash_4_to_1,
        poseidon2_hash_4_to_2,
        poseidon2_hash_10_to_1,
        poseidon2_hash_10_to_4,
        poseidon2_hash_10_to_8,
        poseidon4_hash_4_to_1,
        poseidon4_hash_4_to_2,
        poseidon4_hash_10_to_1,
        poseidon4_hash_10_to_4,
        poseidon4_hash_10_to_8,
        poseidon8_hash_4_to_1,
        poseidon8_hash_4_to_2,
        poseidon8_hash_10_to_1,
        poseidon8_hash_10_to_4,
        poseidon8_hash_10_to_8,
    ]
);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = poseidon
);
