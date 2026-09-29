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

//! Callgrind measurements of the linear-combination operations in `linear_combination.rs`.
//!
//! Each case runs once. Rayon is pinned to one worker so the instruction count does not follow the pool size.

use snarkvm_circuit::{
    environment::{Circuit, Eject, Environment, Inject, LinearCombination, Mode, One, prelude::num_traits::One as _},
    types::Field,
};

use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

const ADD_ITERATIONS: u64 = 1000;
const VALUE_TERMS: usize = 500_000;

fn setup_large_field() -> Field<Circuit> {
    let one = snarkvm_console_types::Field::<<Circuit as Environment>::Network>::one();
    let two = one + one;

    let mut candidate = Field::<Circuit>::one();
    (0..VALUE_TERMS).for_each(|_| {
        candidate += Field::new(Mode::Constant, two);
        candidate += Field::new(Mode::Public, two);
        candidate += Field::new(Mode::Private, two);
    });
    candidate
}

#[library_benchmark]
fn linear_combination_add() {
    let one = <Circuit as Environment>::BaseField::one();
    let two = one + one;
    let mut candidate = <Circuit as Environment>::one();
    for _ in 0..ADD_ITERATIONS {
        candidate = &candidate + &LinearCombination::from(Circuit::new_variable(Mode::Public, two));
    }
    black_box(candidate);
}

#[library_benchmark]
fn linear_combination_add_assign() {
    let one = <Circuit as Environment>::BaseField::one();
    let two = one + one;
    let mut candidate = <Circuit as Environment>::one();
    for _ in 0..ADD_ITERATIONS {
        candidate += LinearCombination::from(Circuit::new_variable(Mode::Public, two));
    }
    black_box(candidate);
}

#[library_benchmark]
#[bench::eject(setup_large_field())]
fn to_value(candidate: Field<Circuit>) {
    black_box(black_box(candidate).eject_value());
}

#[library_benchmark]
#[bench::format(setup_large_field())]
fn debug(candidate: Field<Circuit>) {
    black_box(format!("{:?}", LinearCombination::from(&black_box(candidate))));
}

library_benchmark_group!(
    name = linear_combination,
    benchmarks = [linear_combination_add, linear_combination_add_assign, to_value, debug]
);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = linear_combination
);
