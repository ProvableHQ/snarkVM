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

//! Callgrind measurement of the subdag serialization in `dag.rs`.
//!
//! The batch header, certificate, and subdag serializations run in one process.
//! Rayon is pinned to one worker so the instruction count does not follow the pool size.

use snarkvm_console::prelude::*;
use snarkvm_ledger::narwhal::{Subdag, subdag::test_helpers::sample_subdag};
use snarkvm_utilities::bytes::unchecked_deserialize;

use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

/// Fixed RNG seed so benchmark inputs are reproducible across CI runs.
const SEED: u64 = 0x00BA_6DAB_5EED_00D1;

fn measure<T>(object: &T) -> usize
where
    T: Serialize + DeserializeOwned + ToBytes + FromBytes + Clone,
{
    let bytes = object.to_bytes_le().unwrap();
    let encoded = bincode::serialize(object).unwrap();
    let json = serde_json::to_string(object).unwrap();
    let from_bytes = T::from_bytes_le(&bytes).unwrap();
    let from_bytes_unchecked = T::from_bytes_le_unchecked(&bytes).unwrap();
    let decoded: T = bincode::deserialize(&encoded).unwrap();
    let decoded_unchecked: T = unchecked_deserialize(&encoded).unwrap();
    let from_json: T = serde_json::from_str(&json).unwrap();
    black_box(from_bytes);
    black_box(from_bytes_unchecked);
    black_box(decoded);
    black_box(decoded_unchecked);
    black_box(from_json);
    bytes.len() + encoded.len() + json.len()
}

fn setup_subdag() -> Subdag<snarkvm_console::network::MainnetV0> {
    sample_subdag(&mut TestRng::from_seed(SEED))
}

#[library_benchmark]
#[bench::serialization(setup_subdag())]
fn subdag_serialization(subdag: Subdag<snarkvm_console::network::MainnetV0>) -> usize {
    let batch = subdag.iter().next().unwrap().1.iter().next().unwrap().clone();
    let batch_header = batch.batch_header().clone();
    measure(&batch_header) + measure(&batch) + measure(&subdag)
}

library_benchmark_group!(name = subdag, benchmarks = [subdag_serialization]);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = subdag
);
