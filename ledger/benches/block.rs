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

#[macro_use]
extern crate criterion;

use snarkvm_console::{account::PrivateKey, network::MainnetV0, prelude::*};
use snarkvm_ledger::test_helpers::{TestChainBuilder, sample_genesis_block};

use criterion::Criterion;

type CurrentNetwork = MainnetV0;

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
    c.bench_function(&format!("{name}::to_bytes_le"), |b| b.iter(|| object.to_bytes_le().unwrap()));

    // bincode::serialize
    c.bench_function(&format!("{name}::serialize (bincode)"), |b| b.iter(|| bincode::serialize(&object).unwrap()));

    // serde_json::to_string
    c.bench_function(&format!("{name}::to_string (serde_json)"), |b| {
        b.iter(|| serde_json::to_string(&object).unwrap())
    });

    /////////////////
    // Deserialize //
    /////////////////

    // snarkvm_utilities::FromBytes
    {
        let buffer = object.to_bytes_le().unwrap();
        c.bench_function(&format!("{name}::from_bytes_le"), |b| b.iter(|| T::from_bytes_le(&buffer).unwrap()));

        let buffer = object.to_bytes_le().unwrap();
        c.bench_function(&format!("{name}::from_bytes_le_unchecked"), |b| {
            b.iter(|| T::from_bytes_le_unchecked(&buffer).unwrap())
        });
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
    let mut rng = TestRng::default();

    let mut builder = TestChainBuilder::new(&mut rng).unwrap();
    let block = builder.generate_block(&mut rng).unwrap();

    bench_serialization(c, "Block", block);
}

fn block_header_serialization(c: &mut Criterion) {
    let mut rng = TestRng::default();

    let mut builder = TestChainBuilder::new(&mut rng).unwrap();
    let block = builder.generate_block(&mut rng).unwrap();

    bench_serialization(c, "Header", *block.header());
}

fn block_transactions_serialization(c: &mut Criterion) {
    let mut rng = TestRng::default();
    let block = sample_genesis_block(&mut rng);
    bench_serialization(c, "Transactions", block.transactions().clone());
}

fn transaction_serialization(c: &mut Criterion) {
    let mut rng = TestRng::default();
    let block = sample_genesis_block(&mut rng);
    let transaction = block.transactions().iter().next().unwrap().clone();
    bench_serialization(c, "Transaction", transaction);
}

fn transition_serialization(c: &mut Criterion) {
    let mut rng = TestRng::default();
    let block = sample_genesis_block(&mut rng);
    let transaction = block.transactions().iter().next().unwrap().clone();
    let transition = transaction.transitions().next().unwrap().clone();
    bench_serialization(c, "Transition", transition);
}

fn signature_serialization(c: &mut Criterion) {
    let mut rng = TestRng::default();
    let data = rng.r#gen();

    let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng).unwrap();
    let signature = private_key.sign(&[data], &mut rng).unwrap();

    bench_serialization(c, "Signature", signature);
}

criterion_group! {
    name = block;
    config = Criterion::default().sample_size(10);
    targets = block_serialization,block_header_serialization, block_transactions_serialization,
    transaction_serialization, transition_serialization, signature_serialization,
}

criterion_main!(block);
