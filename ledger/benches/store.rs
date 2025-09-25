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
    store::{
        BlockStorage,
        BlockStore,
        helpers::{memory::BlockMemory, rocksdb::BlockDB},
    },
    test_helpers::TestChainBuilder,
};
use snarkvm_utilities::PrettyUnwrap;

use aleo_std_storage::StorageMode;

use criterion::{BenchmarkGroup, Criterion, criterion_group, criterion_main, measurement::WallTime};
use rand::seq::SliceRandom;
use std::time::Instant;

type Network = snarkvm_console::network::MainnetV0;

/// The number of blocks in the store for get/search operations.
/// Also, the number of pre-generated blocks.
const NUM_BLOCKS: usize = 1000;

// Helper method to benchmark serialization.
fn bench_block_store<S: BlockStorage<Network>>(
    name: &str,
    group: &mut BenchmarkGroup<WallTime>,
    num_validators: usize,
) {
    let rng = &mut TestRng::default();

    // Pre-generate enough blocks for all benchmarks.
    println!("Generating test chain of {NUM_BLOCKS} blocks with {num_validators} validators");
    let mut builder = TestChainBuilder::new_with_quorum_size(num_validators, rng).pretty_unwrap();
    let blocks = builder.generate_blocks(NUM_BLOCKS, rng).unwrap();

    println!("Done generating blocks. Starting benchmark.");

    // TODO(kaimast): Figure out a way to pre-generate a large number of blocks or reduce the number of iterations.
    /*   group.bench_function(format!("{name}::insert/{num_validators}validators"), |b| {
        b.iter_custom(|num_inserts| {
            let num_inserts = num_inserts as usize;
            let store = BlockStore::<Network, S>::open(StorageMode::new_test(None)).unwrap();
            store.insert(builder.genesis_block()).unwrap();

            assert!(num_inserts < NUM_BLOCKS);

            let start = Instant::now();
            for block in &blocks[..num_inserts] {
                if let Err(err) = store.insert(&block) {
                    panic!("Failed to insert block at height {}: {err}", block.height());
                }
            }

            start.elapsed()
        })
    });*/

    group.bench_function(format!("{name}::get_block/{num_validators}validators"), |b| {
        let hashes: Vec<_> = blocks.iter().map(|b| b.hash()).collect();

        b.iter_custom(|num_gets| {
            let store = BlockStore::<Network, S>::open(StorageMode::new_test(None)).unwrap();
            store.insert(builder.genesis_block()).unwrap();

            for block in &blocks {
                if let Err(err) = store.insert(block) {
                    panic!("Failed to insert block at height {}: {err}", block.height());
                }
            }

            let start = Instant::now();
            for _ in 0..num_gets {
                let hash = hashes.choose(rng).unwrap();
                let _ = store.get_block(hash).unwrap();
            }

            start.elapsed()
        })
    });

    group.bench_function(format!("{name}::get_block_height/{num_validators}validators"), |b| {
        let hashes: Vec<_> = blocks.iter().map(|b| b.hash()).collect();

        b.iter_custom(|num_gets| {
            let store = BlockStore::<Network, S>::open(StorageMode::new_test(None)).unwrap();
            store.insert(builder.genesis_block()).unwrap();

            for block in &blocks {
                if let Err(err) = store.insert(block) {
                    panic!("Failed to insert block at height {}: {err}", block.height());
                }
            }

            let start = Instant::now();
            for _ in 0..num_gets {
                let hash = hashes.choose(rng).unwrap();
                let _ = store.get_block_height(hash).unwrap();
            }

            start.elapsed()
        })
    });
}

fn block_store(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_store");
    group.sample_size(10);

    //TODO(kaimast) find a way to speed this up
    //for f in 1..=4 {
    for f in 1..=1 {
        let num_validators = 3 * f + 1;

        bench_block_store::<BlockMemory<Network>>("BlockMemory", &mut group, num_validators);
        bench_block_store::<BlockDB<Network>>("BlockDB", &mut group, num_validators);
    }

    group.finish();
}

criterion_group!(benches, block_store);
criterion_main!(benches);
