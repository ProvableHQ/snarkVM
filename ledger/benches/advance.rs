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

use snarkvm_console::prelude::*;
use snarkvm_ledger::{
    Block,
    store::{
        ConsensusStorage,
        helpers::{memory::ConsensusMemory, rocksdb::ConsensusDB},
    },
};
use snarkvm_utilities::PrettyUnwrap;

use criterion::{BenchmarkGroup, Criterion, criterion_group, criterion_main, measurement::WallTime};
use std::time::Instant;

mod common;
use common::{CurrentNetwork, create_ledger, initialize_logging, load_blocks};

/// Measures block advancement.
fn bench_ledger_advancement<S: ConsensusStorage<CurrentNetwork>>(
    name: &str,
    group: &mut BenchmarkGroup<WallTime>,
    genesis_block: &Block<CurrentNetwork>,
    blocks: &[Block<CurrentNetwork>],
    check_next_block: bool,
    rng: &mut TestRng,
) {
    let name = if check_next_block {
        format!("Ledger<{name}>::check_and_advance")
    } else {
        format!("Ledger<{name}>::advance_without_checks")
    };

    group.bench_function(name, |b| {
        b.iter_custom(|num_ops| {
            let ledger = create_ledger::<S>(genesis_block.clone());
            let mut blocks_iter = blocks.iter();

            let start = Instant::now();
            for _ in 0..num_ops {
                let block = blocks_iter.next().expect("Not enough blocks");
                if check_next_block {
                    ledger.check_next_block(block, rng).pretty_expect("Check for next block failed");
                }
                ledger.advance_to_next_block(block).pretty_expect("Advancement to next block failed");
            }

            start.elapsed()
        })
    });
}

/// Measures block checks.
fn bench_ledger_checks<S: ConsensusStorage<CurrentNetwork>>(
    name: &str,
    group: &mut BenchmarkGroup<WallTime>,
    genesis_block: &Block<CurrentNetwork>,
    blocks: &[Block<CurrentNetwork>],
    rng: &mut TestRng,
) {
    group.bench_function(format!("Ledger<{name}>::check_next_block"), |b| {
        b.iter_custom(|num_ops| {
            let ledger = create_ledger::<S>(genesis_block.clone());
            let mut blocks_iter = blocks.iter();

            // Pre-load the ledger with blocks.
            let num_preloaded_blocks = blocks.len() - 1;
            while (ledger.latest_height() as usize) < num_preloaded_blocks {
                ledger.advance_to_next_block(blocks_iter.next().unwrap()).unwrap();
            }

            let last_block = blocks_iter.next().unwrap();

            let start = Instant::now();
            for _ in 0..num_ops {
                ledger.check_next_block(last_block, rng).unwrap();
            }
            start.elapsed()
        })
    });
}

fn ledger_advance(c: &mut Criterion) {
    initialize_logging();

    let (genesis_block, blocks) = load_blocks("test-ledger").pretty_expect("Failed to load blocks from disk");

    let mut rng = TestRng::default();

    let mut group = c.benchmark_group("ledger_advance");
    group.sample_size(10);

    for check_next_block in [false, true] {
        bench_ledger_advancement::<ConsensusMemory<CurrentNetwork>>(
            "BlockMemory",
            &mut group,
            &genesis_block,
            &blocks,
            check_next_block,
            &mut rng,
        );
        bench_ledger_advancement::<ConsensusDB<CurrentNetwork>>(
            "BlockDB",
            &mut group,
            &genesis_block,
            &blocks,
            check_next_block,
            &mut rng,
        );
    }

    bench_ledger_checks::<ConsensusMemory<CurrentNetwork>>(
        "BlockMemory",
        &mut group,
        &genesis_block,
        &blocks,
        &mut rng,
    );
    bench_ledger_checks::<ConsensusDB<CurrentNetwork>>("BlockDB", &mut group, &genesis_block, &blocks, &mut rng);
    group.finish();
}

criterion_group!(benches, ledger_advance);
criterion_main!(benches);
