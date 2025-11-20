// Copyright (c) 2019-2025 Provable Inc.
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

use criterion::{Criterion, criterion_group, criterion_main};

mod utils;
use utils::{
    bench::{build_vm, setup_transfer_public_state},
    fees::{PRIORITY_FEE, PUBLIC_BASE_FEE},
};

fn bench_transfer_public(c: &mut Criterion) {
    let vm = build_vm();

    c.bench_function("vm.execute_authorization (transfer_public)", |b| {
        b.iter_batched(
            // Setup (not timed).
            || setup_transfer_public_state(&vm, PUBLIC_BASE_FEE, PRIORITY_FEE),
            // Measurement (timed).
            |mut inputs| {
                // Black box to stop the compiler from optimising the call.
                std::hint::black_box(
                    vm.execute_authorization(
                        inputs.authorization,
                        Some(inputs.fee_authorization),
                        Some(&inputs.query),
                        &mut inputs.rng,
                    )
                    .unwrap(),
                )
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(100);
    targets = bench_transfer_public
}
criterion_main!(benches);
