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

use console::{network::MainnetV0, prelude::*};
use ledger_block::Block;

use criterion::Criterion;

type CurrentNetwork = MainnetV0;

/// Loads the genesis block.
fn load_genesis_block() -> Block<CurrentNetwork> {
    Block::<CurrentNetwork>::from_bytes_le(CurrentNetwork::genesis_bytes()).unwrap()
}

/// Helper method to benchmark serialization.
fn bench_serialization<T: Serialize + DeserializeOwned + ToBytes + FromBytes + Clone>(
    c: &mut Criterion,
    name: &str,
    object: T,
) {
    ///////////////
    // Serialize //
    ///////////////

    // snarkvm_utilities::ToBytes
    {
        let object = object.clone();
        c.bench_function(&format!("{name}::to_bytes_le"), move |b| b.iter(|| object.to_bytes_le().unwrap()));
    }
    // bincode::serialize
    {
        let object = object.clone();
        c.bench_function(&format!("{name}::serialize (bincode)"), move |b| {
            b.iter(|| bincode::serialize(&object).unwrap())
        });
    }
    // serde_json::to_string
    {
        let object = object.clone();
        c.bench_function(&format!("{name}::to_string (serde_json)"), move |b| {
            b.iter(|| serde_json::to_string(&object).unwrap())
        });
    }

    /////////////////
    // Deserialize //
    /////////////////

    // snarkvm_utilities::FromBytes
    {
        let buffer = object.to_bytes_le().unwrap();
        c.bench_function(&format!("{name}::from_bytes_le"), move |b| b.iter(|| T::from_bytes_le(&buffer).unwrap()));
    }
    // bincode::deserialize
    {
        let buffer = bincode::serialize(&object).unwrap();
        c.bench_function(&format!("{name}::deserialize (bincode)"), move |b| {
            b.iter(|| bincode::deserialize::<T>(&buffer).unwrap())
        });
    }
    // serde_json::from_str
    {
        let object = serde_json::to_string(&object).unwrap();
        c.bench_function(&format!("{name}::from_str (serde_json)"), move |b| {
            b.iter(|| serde_json::from_str::<T>(&object).unwrap())
        });
    }
}

fn block_serialization(c: &mut Criterion) {
    let block = load_genesis_block();
    bench_serialization(c, "Block", block);
}

fn block_header_serialization(c: &mut Criterion) {
    let header = *load_genesis_block().header();
    bench_serialization(c, "Header", header);
}

fn block_transactions_serialization(c: &mut Criterion) {
    let transactions = load_genesis_block().transactions().clone();
    bench_serialization(c, "Transactions", transactions);
}

fn transaction_serialization(c: &mut Criterion) {
    let transaction = load_genesis_block().transactions().iter().next().unwrap().clone();
    bench_serialization(c, "Transaction", transaction);
}

fn transition_serialization(c: &mut Criterion) {
    let transaction = load_genesis_block().transactions().iter().next().unwrap().clone();
    let transition = transaction.transitions().next().unwrap().clone();
    bench_serialization(c, "Transition", transition);
}

criterion_group! {
    name = block;
    config = Criterion::default().sample_size(10);
    targets = block_serialization, block_header_serialization, block_transactions_serialization, transaction_serialization, transition_serialization
}

criterion_main!(block);
