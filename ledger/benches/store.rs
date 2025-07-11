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

use criterion::{
    BatchSize,
    BenchmarkGroup,
    Criterion,
    Throughput,
    criterion_group,
    criterion_main,
    measurement::Measurement,
};

type Network = snarkvm_console::network::MainnetV0;

// Helper method to benchmark serialization.
fn bench_block_store<S: BlockStorage<Network>, M: Measurement>(
    name: &str,
    group: &mut BenchmarkGroup<M>,
    num_validators: usize,
    num_ops: usize,
) {
    let rng = &mut TestRng::default();

    let (private_keys, genesis) = TestChainBuilder::initialize_components(num_validators, rng).pretty_unwrap();

    group.bench_function(format!("{name}::insert/{num_validators}validators"), |b| {
        b.iter_batched(
            || {
                let store = BlockStore::<Network, S>::open(StorageMode::new_test(None)).unwrap();
                let mut builder = TestChainBuilder::from_components(private_keys.clone(), genesis.clone()).unwrap();
                store.insert(builder.genesis_block()).unwrap();
                let blocks = builder.generate_blocks(num_ops, rng).unwrap();

                (store, blocks)
            },
            |(store, blocks)| {
                for block in blocks {
                    if let Err(err) = store.insert(&block) {
                        panic!("Failed to insert block at height {}: {err}", block.height());
                    }
                }
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function(format!("{name}::get_block/{num_validators}validators"), |b| {
        b.iter_batched(
            || {
                let store = BlockStore::<Network, S>::open(StorageMode::new_test(None)).unwrap();
                let mut builder = TestChainBuilder::from_components(private_keys.clone(), genesis.clone()).unwrap();
                store.insert(builder.genesis_block()).unwrap();
                let blocks = builder.generate_blocks(num_ops, rng).unwrap();

                let hashes: Vec<_> = blocks.iter().map(|b| b.hash()).collect();

                for block in blocks {
                    if let Err(err) = store.insert(&block) {
                        panic!("Failed to insert block at height {}: {err}", block.height());
                    }
                }

                (store, hashes)
            },
            |(store, hashes)| {
                for hash in hashes {
                    let _ = store.get_block(&hash).unwrap();
                }
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function(format!("{name}::get_block_height/{num_validators}validators"), |b| {
        b.iter_batched(
            || {
                let store = BlockStore::<Network, S>::open(StorageMode::new_test(None)).unwrap();
                let mut builder = TestChainBuilder::from_components(private_keys.clone(), genesis.clone()).unwrap();
                store.insert(builder.genesis_block()).unwrap();
                let blocks = builder.generate_blocks(num_validators, rng).unwrap();

                let hashes: Vec<_> = blocks.iter().map(|b| b.hash()).collect();

                for block in blocks {
                    if let Err(err) = store.insert(&block) {
                        panic!("Failed to insert block at height {}: {err}", block.height());
                    }
                }

                (store, hashes)
            },
            |(store, hashes)| {
                for hash in hashes {
                    let _ = store.get_block_height(&hash).unwrap();
                }
            },
            BatchSize::SmallInput,
        )
    });
}

fn block_store(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_store");

    for f in [1, 2, 3, 4] {
        let num_validators = 3 * f + 1;
        let num_ops = 10;

        group.throughput(Throughput::Elements(num_ops as u64));
        bench_block_store::<BlockMemory<Network>, _>("BlockMemory", &mut group, num_validators, num_ops);
        bench_block_store::<BlockDB<Network>, _>("BlockDB", &mut group, num_validators, num_ops);
    }

    group.finish();
}

criterion_group!(benches, block_store);
criterion_main!(benches);
