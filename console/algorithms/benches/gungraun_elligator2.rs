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

//! Callgrind measurement of the Elligator2 encode in `elligator2.rs`.
//!
//! The case runs once. Rayon is pinned to one worker so the instruction count does not follow the pool size.

use snarkvm_console_algorithms::Elligator2;
use snarkvm_console_types::prelude::*;
use snarkvm_utilities::{TestRng, Uniform};

use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

const SEED: u64 = 0xE111_6A70;

fn setup_field() -> Field<Console> {
    Field::rand(&mut TestRng::from_seed(SEED))
}

#[library_benchmark]
#[bench::encode(setup_field())]
fn elligator2_encode(input: Field<Console>) -> (Group<Console>, bool) {
    black_box(Elligator2::<Console>::encode(&black_box(input)).unwrap())
}

library_benchmark_group!(name = elligator2, benchmarks = [elligator2_encode]);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = elligator2
);
