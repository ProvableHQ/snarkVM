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

//! Callgrind measurements of the Merkle tree operations selected by `criterion_main` in `merkle_tree.rs`.
//!
//! Each case runs once. Rayon is pinned to one worker so the instruction count does not follow the pool size.

use snarkvm_console_network::{
    BHPMerkleTree,
    MainnetV0,
    Network,
    prelude::{Rng, TestRng, ToBits, Uniform},
};
use snarkvm_console_types::Field;

use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::{collections::BTreeMap, hint::black_box};

const DEPTH: u8 = 32;
const MAX_INSTANTIATED_DEPTH: u8 = 8;
const SEED: u64 = 0x3E2C_1E00;

type Tree = BHPMerkleTree<MainnetV0, DEPTH>;
type Leaf = Vec<bool>;

fn leaves(count: usize) -> Vec<Leaf> {
    let mut rng = TestRng::from_seed(SEED);
    (0..count).map(|_| Field::<MainnetV0>::rand(&mut rng).to_bits_le()).collect()
}

fn tree(count: usize) -> Tree {
    MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves(count)[..count]).unwrap()
}

#[library_benchmark]
#[bench::leaves_1(leaves(1))]
#[bench::leaves_100(leaves(100))]
fn merkle_new(leaves: Vec<Leaf>) -> Tree {
    black_box(MainnetV0::merkle_tree_bhp::<DEPTH>(&black_box(leaves)).unwrap())
}

#[library_benchmark]
#[bench::leaves_1_append_1((tree(1), leaves(1)))]
#[bench::leaves_1_append_100((tree(1), leaves(100)))]
#[bench::leaves_100_append_1((tree(100), leaves(1)))]
#[bench::leaves_100_append_100((tree(100), leaves(100)))]
fn merkle_append(input: (Tree, Vec<Leaf>)) -> Tree {
    let (mut merkle_tree, new_leaves) = black_box(input);
    merkle_tree.append(&new_leaves).unwrap();
    black_box(merkle_tree)
}

fn updates(num_leaves: usize, count: usize) -> Vec<(usize, Leaf)> {
    let mut rng = TestRng::from_seed(SEED);
    leaves(count)
        .into_iter()
        .map(|leaf| {
            let index = rng.random_range(0..num_leaves);
            (index, leaf)
        })
        .collect()
}

#[library_benchmark]
#[bench::leaves_1_update_1((tree(1), updates(1, 1)))]
#[bench::leaves_1_update_100((tree(1), updates(1, 100)))]
#[bench::leaves_100_update_1((tree(100), updates(100, 1)))]
#[bench::leaves_100_update_100((tree(100), updates(100, 100)))]
fn merkle_update(input: (Tree, Vec<(usize, Leaf)>)) -> Tree {
    let (mut merkle_tree, updates) = black_box(input);
    for (index, new_leaf) in &updates {
        merkle_tree.update(*index, new_leaf).unwrap();
    }
    black_box(merkle_tree)
}

fn update_map(num_leaves: usize, count: usize) -> BTreeMap<usize, Leaf> {
    let mut updates = updates(num_leaves, 2 * count);
    updates.sort_by_key(|(index, _)| *index);
    updates.reverse();
    updates.dedup_by_key(|(index, _)| *index);
    let count = std::cmp::min(count, updates.len());
    BTreeMap::from_iter(updates[..count].iter().cloned())
}

#[library_benchmark]
#[bench::leaves_1_update_1((tree(1), update_map(1, 1)))]
#[bench::leaves_1_update_100((tree(1), update_map(1, 100)))]
#[bench::leaves_100_update_1((tree(100), update_map(100, 1)))]
#[bench::leaves_100_update_100((tree(100), update_map(100, 100)))]
fn merkle_update_many(input: (Tree, BTreeMap<usize, Leaf>)) -> Tree {
    let (mut merkle_tree, updates) = black_box(input);
    merkle_tree.update_many(&updates).unwrap();
    black_box(merkle_tree)
}

fn single_update(depth: u8) -> (Tree, usize, Leaf) {
    let mut rng = TestRng::from_seed(SEED.wrapping_add(depth as u64));
    let num_leaves = 2usize.saturating_pow(depth as u32);
    let merkle_tree = MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves(num_leaves)[..num_leaves]).unwrap();
    let index = rng.random_range(0..num_leaves);
    let new_leaf = leaves(1).pop().unwrap();
    (merkle_tree, index, new_leaf)
}

#[library_benchmark]
#[bench::depth_1(single_update(1))]
#[bench::depth_2(single_update(2))]
#[bench::depth_3(single_update(3))]
#[bench::depth_4(single_update(4))]
#[bench::depth_5(single_update(5))]
#[bench::depth_6(single_update(6))]
#[bench::depth_7(single_update(7))]
#[bench::depth_8(single_update(MAX_INSTANTIATED_DEPTH))]
fn update_single(input: (Tree, usize, Leaf)) -> Tree {
    let (mut merkle_tree, index, new_leaf) = black_box(input);
    merkle_tree.update(index, &new_leaf).unwrap();
    black_box(merkle_tree)
}

#[library_benchmark]
#[bench::depth_1(single_update(1))]
#[bench::depth_2(single_update(2))]
#[bench::depth_3(single_update(3))]
#[bench::depth_4(single_update(4))]
#[bench::depth_5(single_update(5))]
#[bench::depth_6(single_update(6))]
#[bench::depth_7(single_update(7))]
#[bench::depth_8(single_update(MAX_INSTANTIATED_DEPTH))]
fn update_batch(input: (Tree, usize, Leaf)) -> Tree {
    let (mut merkle_tree, index, new_leaf) = black_box(input);
    let updates = BTreeMap::from([(index, new_leaf)]);
    merkle_tree.update_many(&updates).unwrap();
    black_box(merkle_tree)
}

library_benchmark_group!(
    name = merkle_tree,
    benchmarks = [merkle_new, merkle_append, merkle_update, merkle_update_many, update_single, update_batch]
);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = merkle_tree
);
