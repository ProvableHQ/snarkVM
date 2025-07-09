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

use aleo_std_storage::StorageMode;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

type Network = snarkvm_console::network::MainnetV0;

// Helper method to benchmark serialization.
fn bench_block_store<S: BlockStorage<Network>>(name: &str, c: &mut Criterion) {
    let rng = &mut TestRng::default();

    let (private_keys, genesis) = TestChainBuilder::initialize_components(rng).unwrap();

    c.bench_function(&format!("{name}::insert"), |b| {
        b.iter_batched(
            || {
                const NUM_WRITES: usize = 10;

                let store = BlockStore::<Network, S>::open(StorageMode::new_test(None)).unwrap();
                let mut builder = TestChainBuilder::from_components(private_keys.clone(), genesis.clone()).unwrap();
                store.insert(builder.genesis_block()).unwrap();
                let blocks = builder.generate_blocks(NUM_WRITES, rng);

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

    c.bench_function(&format!("{name}::get_block"), |b| {
        b.iter_batched(
            || {
                const NUM_READS: usize = 10;

                let store = BlockStore::<Network, S>::open(StorageMode::new_test(None)).unwrap();
                let mut builder = TestChainBuilder::from_components(private_keys.clone(), genesis.clone()).unwrap();
                store.insert(builder.genesis_block()).unwrap();
                let blocks = builder.generate_blocks(NUM_READS, rng);

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

    c.bench_function(&format!("{name}::get_block_height"), |b| {
        b.iter_batched(
            || {
                const NUM_READS: usize = 10;

                let store = BlockStore::<Network, S>::open(StorageMode::new_test(None)).unwrap();
                let mut builder = TestChainBuilder::from_components(private_keys.clone(), genesis.clone()).unwrap();
                store.insert(builder.genesis_block()).unwrap();
                let blocks = builder.generate_blocks(NUM_READS, rng);

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

fn memory_store(c: &mut Criterion) {
    bench_block_store::<BlockMemory<Network>>("BlockMemory", c);
}

fn rocksdb_store(c: &mut Criterion) {
    bench_block_store::<BlockDB<Network>>("BlockDB", c);
}

criterion_group! {
    name = block_store;
    config = Criterion::default().sample_size(10);
    targets = memory_store,rocksdb_store
}

criterion_main!(block_store);
