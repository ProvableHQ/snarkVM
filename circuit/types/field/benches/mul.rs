// Copyright (C) 2019-2023 Aleo Systems Inc.
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

use snarkvm_circuit_environment::*;
use snarkvm_circuit_types_field::Field;

use criterion::Criterion;

fn bench_mul(c: &mut Criterion) {
    c.bench_function("Field::mul_assign", move |b| {
        let one = Field::<Circuit>::one();
        let two = one.clone() + &one;
        let mut base = two.clone();

        b.iter(|| {
            base *= &two;
        })
    });
}

criterion_group! {
    name = mul;
    config = Criterion::default();
    targets = bench_mul
}

criterion_main!(mul);
