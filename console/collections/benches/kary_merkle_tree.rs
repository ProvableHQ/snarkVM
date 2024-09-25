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

use snarkvm_console_algorithms::Sha3_256;
use snarkvm_console_collections::kary_merkle_tree::KaryMerkleTree;
use snarkvm_console_network::{
    prelude::{TestRng, ToBits, Uniform},
    MainnetV0,
};
use snarkvm_console_types::Field;

use criterion::Criterion;

const DEPTH: u8 = 8;
const ARITY: u8 = 8;

/// Generates the specified number of random Merkle tree leaves.
macro_rules! generate_leaves {
    ($num_leaves:expr, $rng:expr) => {{ (0..$num_leaves).map(|_| Field::<MainnetV0>::rand($rng).to_bits_le()).collect::<Vec<_>>() }};
}

fn new(c: &mut Criterion) {
    let mut rng = TestRng::default();

    // Initialize the hashers.
    let path_hasher = Sha3_256::default();
    let leaf_hasher = Sha3_256::default();

    // Determine the number of leaves to benchmark.
    let max_num_leaves = (ARITY as u32).pow(DEPTH as u32);
    const RATIO: u32 = 4;

    // Accumulate leaves in a vector to avoid recomputing across iterations.
    let leaves = generate_leaves!(max_num_leaves, &mut rng);
    for ratio in 0..=RATIO {
        let num_leaves = max_num_leaves * ratio / 4;
        // Benchmark the creation of a KaryMerkle tree with the specified number of leaves.
        c.bench_function(&format!("KaryMerkleTree/new/{num_leaves} ({}% full)", ratio as f64 * 100f64 / 4f64), |b| {
            b.iter(|| {
                let _tree = KaryMerkleTree::<_, _, DEPTH, ARITY>::new(
                    &leaf_hasher,
                    &path_hasher,
                    &leaves[0..num_leaves as usize],
                )
                .unwrap();
            })
        });
    }
}

fn verify(c: &mut Criterion) {
    let mut rng = TestRng::default();

    // Initialize the hashers.
    let path_hasher = Sha3_256::default();
    let leaf_hasher = Sha3_256::default();

    // Determine the maximum number of leaves.
    let max_num_leaves = (ARITY as u32).pow(DEPTH as u32);
    // Initialize the leaves.
    let leaves = generate_leaves!(max_num_leaves, &mut rng);

    // Initialize the tree.
    let tree = KaryMerkleTree::<_, _, DEPTH, ARITY>::new(&leaf_hasher, &path_hasher, &leaves).unwrap();
    let proof = tree.prove(0, &leaves[0]).unwrap();

    // Benchmark the verification a KaryMerkle tree proof.
    c.bench_function("KaryMerkleTree/verify", |b| {
        b.iter(|| {
            assert!(proof.verify(&leaf_hasher, &path_hasher, tree.root(), &leaves[0]));
        })
    });
}

criterion_group! {
    name = kary_merkle_tree;
    config = Criterion::default().sample_size(10);
    targets = new, verify
}
criterion_main!(kary_merkle_tree);
