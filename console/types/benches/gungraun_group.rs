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

//! Callgrind measurements of the group conversions in `group.rs`.
//!
//! Each case converts 1000 fields, matching the criterion bench body. Rayon is pinned to one worker.

use snarkvm_console_network::{MainnetV0, environment::prelude::*};
use snarkvm_console_types::{Field, Group};

use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

type CurrentNetwork = MainnetV0;

const SEED: u64 = 0x6900_9000;
const SAMPLES: usize = 1000;

fn setup_fields() -> Vec<Field<CurrentNetwork>> {
    let mut rng = TestRng::from_seed(SEED);
    (0..SAMPLES).map(|_| rng.random()).collect()
}

fn setup_on_curve() -> Vec<Field<CurrentNetwork>> {
    let mut rng = TestRng::from_seed(SEED);
    type Projective = <CurrentNetwork as Environment>::Projective;
    (0..SAMPLES).map(|_| rng.random::<Projective>().to_affine().to_x_coordinate()).map(Field::new).collect()
}

fn setup_off_curve() -> Vec<Field<CurrentNetwork>> {
    let mut rng = TestRng::from_seed(SEED);
    type Affine = <CurrentNetwork as Environment>::Affine;
    std::iter::repeat(())
        .map(|_| rng.random::<Field<CurrentNetwork>>())
        .filter(|&field| Affine::from_x_coordinate(*field, true).is_none())
        .take(SAMPLES)
        .collect()
}

fn convert_all(fields: Vec<Field<CurrentNetwork>>) {
    for field in &black_box(fields) {
        let _ = black_box(Group::from_field(field));
    }
}

#[library_benchmark]
#[bench::thousand(setup_fields())]
fn group_from_field(fields: Vec<Field<CurrentNetwork>>) {
    convert_all(fields);
}

#[library_benchmark]
#[bench::thousand(setup_on_curve())]
fn group_from_field_on_curve(fields: Vec<Field<CurrentNetwork>>) {
    convert_all(fields);
}

#[library_benchmark]
#[bench::thousand(setup_off_curve())]
fn group_from_field_off_curve(fields: Vec<Field<CurrentNetwork>>) {
    convert_all(fields);
}

library_benchmark_group!(
    name = group,
    benchmarks = [group_from_field, group_from_field_on_curve, group_from_field_off_curve]
);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = group
);
