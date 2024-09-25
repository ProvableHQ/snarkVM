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

use snarkvm_circuit_environment::*;
use snarkvm_circuit_types_boolean::Boolean;

use criterion::Criterion;

fn bench_and(c: &mut Criterion) {
    c.bench_function("Boolean::and", move |b| {
        let first = Boolean::<Circuit>::new(Mode::Public, true);
        let second = Boolean::new(Mode::Private, true);

        b.iter(|| {
            let _third = &first & &second;
        })
    });

    c.bench_function("Boolean::and_assign", move |b| {
        let mut first = Boolean::<Circuit>::new(Mode::Public, true);
        let second = Boolean::new(Mode::Private, true);

        b.iter(|| {
            first &= &second;
        })
    });
}

criterion_group! {
    name = and;
    config = Criterion::default().sample_size(10);
    targets = bench_and
}

criterion_main!(and);
