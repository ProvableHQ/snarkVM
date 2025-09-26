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

use snarkvm_console::prelude::*;
use snarkvm_ledger::{
    Block,
    Ledger,
    LedgerOptions,
    store::{
        ConsensusStorage,
        helpers::{memory::ConsensusMemory, rocksdb::ConsensusDB},
    },
    test_helpers::TestChainBuilder,
};
use snarkvm_utilities::PrettyUnwrap;

use aleo_std_storage::StorageMode;

use criterion::{BenchmarkGroup, Criterion, criterion_group, criterion_main, measurement::WallTime};
use std::time::Instant;

type Network = snarkvm_console::network::MainnetV0;

/// Helper to initialize the `Ledger`.
fn create_ledger<S: ConsensusStorage<Network>>(
    genesis_block: Block<Network>,
    enable_cache: bool,
) -> Ledger<Network, S> {
    let options = if enable_cache { LedgerOptions::default().enable_block_cache() } else { LedgerOptions::default() };

    Ledger::load_with_opts(genesis_block, StorageMode::new_test(None), options).unwrap()
}

/// Measures block advancement.
fn bench_ledger_advancement<S: ConsensusStorage<Network>>(
    name: &str,
    group: &mut BenchmarkGroup<WallTime>,
    genesis_block: &Block<Network>,
    blocks: &[Block<Network>],
    enable_cache: bool,
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
            let ledger = create_ledger::<S>(genesis_block.clone(), enable_cache);
            let mut blocks_iter = blocks.iter();

            let start = Instant::now();
            for _ in 0..num_ops {
                let block = blocks_iter.next().expect("Not enough blocks");
                if check_next_block {
                    ledger.check_next_block(block, rng).unwrap();
                }
                ledger.advance_to_next_block(block).unwrap();
            }

            start.elapsed()
        })
    });
}

/// Measures block checks.
fn bench_ledger_checks<S: ConsensusStorage<Network>>(
    name: &str,
    group: &mut BenchmarkGroup<WallTime>,
    genesis_block: &Block<Network>,
    blocks: &[Block<Network>],
    enable_cache: bool,
    rng: &mut TestRng,
) {
    group.bench_function(format!("Ledger<{name}>::check_next_block"), |b| {
        b.iter_custom(|num_ops| {
            let ledger = create_ledger::<S>(genesis_block.clone(), enable_cache);
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
    // The number of pre generated blocks.
    const NUM_BLOCKS: usize = 1000;

    let mut group = c.benchmark_group("ledger_advance");
    group.sample_size(10);

    // Pre-generate enough blocks for all benchmarks.
    println!("Generating test chain of {NUM_BLOCKS} blocks");
    let rng = &mut TestRng::default();
    let mut builder = TestChainBuilder::new(rng).pretty_unwrap();
    let blocks = builder.generate_blocks(NUM_BLOCKS, rng).unwrap();

    println!("Done generating blocks. Starting benchmark.");

    for check_next_block in [false, true] {
        /* memory-backed is too fast for the small number of blocks
        bench_ledger_advancement::<ConsensusMemory<Network>>(
            "BlockMemory",
            &mut group,
            builder.genesis_block(),
            &blocks,
            false, // disable cache
            check_next_block,
            rng,
        );*/
        bench_ledger_advancement::<ConsensusDB<Network>>(
            "BlockDB",
            &mut group,
            builder.genesis_block(),
            &blocks,
            false, // disable cache
            check_next_block,
            rng,
        );
        bench_ledger_advancement::<ConsensusDB<Network>>(
            "CachedBlockDB",
            &mut group,
            builder.genesis_block(),
            &blocks,
            true, // enable cache
            check_next_block,
            rng,
        );
    }

    bench_ledger_checks::<ConsensusMemory<Network>>(
        "BlockMemory",
        &mut group,
        builder.genesis_block(),
        &blocks,
        false, // disable cache
        rng,
    );
    bench_ledger_checks::<ConsensusDB<Network>>(
        "BlockDB",
        &mut group,
        builder.genesis_block(),
        &blocks,
        false, // disable cache
        rng,
    );
    bench_ledger_checks::<ConsensusDB<Network>>(
        "CachedBlockDB",
        &mut group,
        builder.genesis_block(),
        &blocks,
        true, // enable cache
        rng,
    );

    group.finish();
}

criterion_group!(benches, ledger_advance);
criterion_main!(benches);
