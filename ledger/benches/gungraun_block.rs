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

//! Callgrind measurement of the block, nested-type, and signature serialization in `block.rs`.
//!
//! Genesis is built once, then every serialization from that bench runs in this process.
//! Rayon is pinned to one worker so the instruction count does not follow the pool size.

use snarkvm_console::{account::PrivateKey, network::MainnetV0, prelude::*};
use snarkvm_ledger::test_helpers::sample_genesis_block;
use snarkvm_ledger_block::Block;

use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

const SEED: u64 = 0xB10C_5EED;

fn measure<T>(object: &T) -> usize
where
    T: Serialize + DeserializeOwned + ToBytes + FromBytes + Clone,
{
    let bytes = object.to_bytes_le().unwrap();
    let encoded = bincode::serialize(object).unwrap();
    let from_bytes = T::from_bytes_le(&bytes).unwrap();
    let from_bytes_unchecked = T::from_bytes_le_unchecked(&bytes).unwrap();
    let decoded: T = bincode::deserialize(&encoded).unwrap();
    black_box(from_bytes);
    black_box(from_bytes_unchecked);
    black_box(decoded);
    bytes.len() + encoded.len()
}

fn setup_block() -> Block<MainnetV0> {
    sample_genesis_block(&mut TestRng::from_seed(SEED))
}

#[library_benchmark]
#[bench::serialization(setup_block())]
fn block_and_signature_serialization(block: Block<MainnetV0>) -> usize {
    let header = *block.header();
    let transactions = block.transactions().clone();
    let transaction = block.transactions().iter().next().unwrap().clone();
    let transition = transaction.transitions().next().unwrap().clone();

    let mut rng = TestRng::from_seed(SEED);
    let data = rng.random();
    let private_key = PrivateKey::<MainnetV0>::new(&mut rng).unwrap();
    let signature = private_key.sign(&[data], &mut rng).unwrap();

    measure(&block)
        + measure(&header)
        + measure(&transactions)
        + measure(&transaction)
        + measure(&transition)
        + measure(&signature)
}

library_benchmark_group!(name = block, benchmarks = [block_and_signature_serialization]);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = block
);
