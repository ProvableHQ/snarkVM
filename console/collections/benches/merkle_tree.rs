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

use snarkvm_console_network::{
    MainnetV0,
    Network,
    prelude::{TestRng, ToBits, Uniform},
};
use snarkvm_console_types::Field;

use criterion::{BatchSize, BenchmarkId, Criterion, SamplingMode};
use snarkvm_console_network::BHPMerkleTree;
use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

const DEPTH: u8 = 32;
const MAX_INSTANTIATED_DEPTH: u8 = 16;
const NUM_LEAVES: &[usize] = &[1, 10, 100, 1_000, 10_000, 100_000];
const APPEND_SIZES: &[usize] = &[1, 10, 100, 1_000, 10_000, 100_000];
const UPDATE_SIZES: &[usize] = &[1, 10, 100, 1_000, 10_000];

/// Generates the specified number of random Merkle tree leaves.
macro_rules! generate_leaves {
    ($num_leaves:expr, $rng:expr) => {{ (0..$num_leaves).map(|_| Field::<MainnetV0>::rand($rng).to_bits_le()).collect::<Vec<_>>() }};
}

// Lazy-initialized data for large benchmarks. Initialized only when the benchmark is actually run.
static LEAVES_65537: OnceLock<Vec<Vec<bool>>> = OnceLock::new();
static LEAVES_16777217: OnceLock<Vec<Vec<bool>>> = OnceLock::new();

struct AppendState {
    tree: BHPMerkleTree<MainnetV0, DEPTH>,
    new_leaf: Vec<bool>,
}

static APPEND_POW16_65534_STATE: OnceLock<AppendState> = OnceLock::new();
static APPEND_POW16_65535_STATE: OnceLock<AppendState> = OnceLock::new();
static APPEND_POW16_65536_STATE: OnceLock<AppendState> = OnceLock::new();
static APPEND_POW24_16777214_STATE: OnceLock<AppendState> = OnceLock::new();
static APPEND_POW24_16777215_STATE: OnceLock<AppendState> = OnceLock::new();
static APPEND_POW24_16777216_STATE: OnceLock<AppendState> = OnceLock::new();

fn new(c: &mut Criterion) {
    let mut rng = TestRng::default();
    // Accumulate leaves in a vector to avoid recomputing across iterations.
    let leaves = generate_leaves!(*NUM_LEAVES.last().unwrap(), &mut rng);
    for num_leaves in NUM_LEAVES {
        // Benchmark the creation of a Merkle tree with the specified number of leaves.
        c.bench_function(&format!("MerkleTree/new/{num_leaves}"), |b| {
            b.iter(|| {
                let _tree = MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[0..*num_leaves]).unwrap();
            })
        });
    }
}

fn append(c: &mut Criterion) {
    let mut rng = TestRng::default();
    // Accumulate all leaves in a vector to avoid recomputing across iterations.
    let leaves = generate_leaves!(*NUM_LEAVES.last().unwrap(), &mut rng);
    // Generate all of the leaves that will be appended to the tree.
    let new_leaves = generate_leaves!(*APPEND_SIZES.last().unwrap(), &mut rng);
    // For each number of leaves to append, benchmark the append operation.
    for num_leaves in NUM_LEAVES {
        for num_new_leaves in APPEND_SIZES {
            // Construct a Merkle tree with the specified number of leaves.
            let merkle_tree = MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[..*num_leaves]).unwrap();
            c.bench_function(&format!("MerkleTree/append/{num_leaves}/{num_new_leaves}"), |b| {
                b.iter_batched(
                    || merkle_tree.clone(),
                    |mut merkle_tree| {
                        merkle_tree.append(&new_leaves[..*num_new_leaves]).unwrap();
                    },
                    BatchSize::SmallInput,
                )
            });
        }
    }
}

fn update(c: &mut Criterion) {
    let mut rng = TestRng::default();
    // Accumulate leaves in a vector to avoid recomputing across iterations.
    let leaves = generate_leaves!(*NUM_LEAVES.last().unwrap(), &mut rng);
    // For each number of leaves to update, benchmark the update operation.
    for num_leaves in NUM_LEAVES {
        // Construct a vector of (index, new_leaf) pairs to update the tree with.
        // Note that we need to bound the number of updates since a large number of sequential singular updates to exceedingly long.
        let updates = generate_leaves!(std::cmp::min(*UPDATE_SIZES.last().unwrap(), 10_000), &mut rng)
            .into_iter()
            .map(|leaf| {
                let index: usize = Uniform::rand(&mut rng);
                (index % num_leaves, leaf)
            })
            .collect::<Vec<_>>();

        for num_new_leaves in UPDATE_SIZES {
            // Construct a Merkle tree with the specified number of leaves.
            let merkle_tree = MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[..*num_leaves]).unwrap();

            c.bench_function(&format!("MerkleTree/update/{num_leaves}/{num_new_leaves}"), |b| {
                b.iter_batched(
                    || merkle_tree.clone(),
                    |mut merkle_tree| {
                        for (index, new_leaf) in updates.iter().take(*num_new_leaves) {
                            merkle_tree.update(*index, new_leaf).unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                )
            });
        }
    }
}

fn update_many(c: &mut Criterion) {
    let mut rng = TestRng::default();
    // Accumulate leaves in a vector to avoid recomputing across iterations.
    let leaves = generate_leaves!(*NUM_LEAVES.last().unwrap(), &mut rng);
    // For each number of leaves to update, benchmark the update operation.
    for num_leaves in NUM_LEAVES {
        // Generate all of the updates that will be applied to the tree.
        // Note that we generate 2 * MAX_UPDATE_SIZE leaves to increase the number of unique of updates.
        let mut updates = generate_leaves!(2 * *UPDATE_SIZES.last().unwrap(), &mut rng)
            .into_iter()
            .map(|leaf| {
                let index: usize = Uniform::rand(&mut rng);
                (index % num_leaves, leaf)
            })
            .collect::<Vec<_>>();
        updates.sort_by_key(|(a, _)| *a);
        updates.reverse();
        updates.dedup_by_key(|(a, _)| *a);

        for num_new_leaves in UPDATE_SIZES {
            // Construct a Merkle tree with the specified number of leaves.
            let merkle_tree = MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[..*num_leaves]).unwrap();
            let num_new_leaves = std::cmp::min(*num_new_leaves, updates.len());
            let updates = BTreeMap::from_iter(updates[..num_new_leaves].iter().cloned());
            c.bench_function(&format!("MerkleTree/update_many/{num_leaves}/{num_new_leaves}",), |b| {
                b.iter_batched(
                    || merkle_tree.clone(),
                    |mut merkle_tree| {
                        merkle_tree.update_many(&updates).unwrap();
                    },
                    BatchSize::SmallInput,
                )
            });
        }
    }
}

/// Benchmarks Merkle tree creation with 2^16 vs 2^16+1 leaves at block-tree depth (32).
/// Run only this comparison: cargo bench -p snarkvm-console-collections --bench merkle_tree -- 65536_vs_65537
fn creation_65536_vs_65537_leaves(c: &mut Criterion) {
    const N: usize = 1 << 16; // 2^16
    let mut group = c.benchmark_group("MerkleTree/new/creation_65536_vs_65537");
    group.bench_function("65536_leaves_depth_32", |b| {
        let leaves = LEAVES_65537.get_or_init(|| {
            let mut rng = TestRng::default();
            generate_leaves!(N + 1, &mut rng)
        });
        b.iter(|| {
            let _tree = MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[..N]).unwrap();
        })
    });
    group.bench_function("65537_leaves_depth_32", |b| {
        let leaves = LEAVES_65537.get_or_init(|| {
            let mut rng = TestRng::default();
            generate_leaves!(N + 1, &mut rng)
        });
        b.iter(|| {
            let _tree = MainnetV0::merkle_tree_bhp::<DEPTH>(leaves).unwrap();
        })
    });
}

/// Benchmarks Merkle tree creation with 2^24 vs 2^24+1 leaves at block-tree depth (32).
/// Uses Criterion's Flat sampling mode and minimal sample count for long-running iterations.
/// Run only this comparison: cargo bench -p snarkvm-console-collections --bench merkle_tree -- 16777216_vs_16777217
fn creation_16777216_vs_16777217_leaves(c: &mut Criterion) {
    const N: usize = 1 << 24; // 2^24
    let mut group = c.benchmark_group("MerkleTree/new/creation_16777216_vs_16777217");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
    group.bench_function("16777216_leaves_depth_32", |b| {
        let leaves = LEAVES_16777217.get_or_init(|| {
            let mut rng = TestRng::default();
            generate_leaves!(N + 1, &mut rng)
        });
        b.iter(|| {
            let _tree = MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[..N]).unwrap();
        })
    });
    group.bench_function("16777217_leaves_depth_32", |b| {
        let leaves = LEAVES_16777217.get_or_init(|| {
            let mut rng = TestRng::default();
            generate_leaves!(N + 1, &mut rng)
        });
        b.iter(|| {
            let _tree = MainnetV0::merkle_tree_bhp::<DEPTH>(leaves).unwrap();
        })
    });
}

/// Benchmarks Merkle tree append of 1 leaf at 2^16-1 vs 2^16 leaves (latter crosses power-of-two boundary).
/// Run only this comparison: cargo bench -p snarkvm-console-collections --bench merkle_tree -- append_65535_vs_65536
fn append_pow16_leaves(c: &mut Criterion) {
    const N: usize = 1 << 16; // 2^16
    let mut group = c.benchmark_group("MerkleTree/append/append_pow16_leaves");
    group.bench_function("append_1_to_65534_leaves", |b| {
        let state = APPEND_POW16_65534_STATE.get_or_init(|| {
            let mut rng = TestRng::default();
            let leaves = generate_leaves!(N + 1, &mut rng);
            AppendState {
                tree: MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[..N - 1]).unwrap(),
                new_leaf: leaves[N].clone(),
            }
        });
        b.iter_batched(
            || state.tree.clone(),
            |mut tree| tree.append(std::slice::from_ref(&state.new_leaf)).unwrap(),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("append_1_to_65535_leaves", |b| {
        let state = APPEND_POW16_65535_STATE.get_or_init(|| {
            let mut rng = TestRng::default();
            let leaves = generate_leaves!(N + 2, &mut rng);
            AppendState {
                tree: MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[..N]).unwrap(),
                new_leaf: leaves[N + 1].clone(),
            }
        });
        b.iter_batched(
            || state.tree.clone(),
            |mut tree| tree.append(std::slice::from_ref(&state.new_leaf)).unwrap(),
            BatchSize::SmallInput,
        )
    });
    group.bench_function("append_1_to_65536_leaves", |b| {
        let state = APPEND_POW16_65536_STATE.get_or_init(|| {
            let mut rng = TestRng::default();
            let leaves = generate_leaves!(N + 3, &mut rng);
            AppendState {
                tree: MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[..N + 1]).unwrap(),
                new_leaf: leaves[N + 2].clone(),
            }
        });
        b.iter_batched(
            || state.tree.clone(),
            |mut tree| tree.append(std::slice::from_ref(&state.new_leaf)).unwrap(),
            BatchSize::SmallInput,
        )
    });
}

/// Benchmarks Merkle tree append of 1 leaf at 2^24-2, 2^24-1, and 2^24 leaves (last crosses power-of-two boundary).
/// Uses Criterion's Flat sampling mode and minimal sample count for long-running iterations.
/// Run only this comparison: cargo bench -p snarkvm-console-collections --bench merkle_tree -- append_pow24_leaves
fn append_pow24_leaves(c: &mut Criterion) {
    const N: usize = 1 << 24; // 2^24
    let mut group = c.benchmark_group("MerkleTree/append/append_pow24_leaves");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
    group.bench_function("append_1_to_16777214_leaves", |b| {
        let state = APPEND_POW24_16777214_STATE.get_or_init(|| {
            let mut rng = TestRng::default();
            let leaves = generate_leaves!(N, &mut rng);
            AppendState {
                tree: MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[..N - 1]).unwrap(),
                new_leaf: leaves[N - 1].clone(),
            }
        });
        b.iter_batched(
            || state.tree.clone(),
            |mut tree| tree.append(std::slice::from_ref(&state.new_leaf)).unwrap(),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("append_1_to_16777215_leaves", |b| {
        let state = APPEND_POW24_16777215_STATE.get_or_init(|| {
            let mut rng = TestRng::default();
            let leaves = generate_leaves!(N + 2, &mut rng);
            AppendState {
                tree: MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[..N]).unwrap(),
                new_leaf: leaves[N + 1].clone(),
            }
        });
        b.iter_batched(
            || state.tree.clone(),
            |mut tree| tree.append(std::slice::from_ref(&state.new_leaf)).unwrap(),
            BatchSize::SmallInput,
        )
    });
    group.bench_function("append_1_to_16777216_leaves", |b| {
        let state = APPEND_POW24_16777216_STATE.get_or_init(|| {
            let mut rng = TestRng::default();
            let leaves = generate_leaves!(N + 3, &mut rng);
            AppendState {
                tree: MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[..N + 1]).unwrap(),
                new_leaf: leaves[N + 2].clone(),
            }
        });
        b.iter_batched(
            || state.tree.clone(),
            |mut tree| tree.append(std::slice::from_ref(&state.new_leaf)).unwrap(),
            BatchSize::SmallInput,
        )
    });
}

/// Benchmarks prepare_append with vs without preserved_tree_allocation (same result, allocation reuse).
/// Run only: cargo bench -p snarkvm-console-collections --bench merkle_tree -- prepare_append_preserved
fn prepare_append_with_vs_without_preserved_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("MerkleTree/prepare_append/prepare_append_preserved");
    let mut rng = TestRng::default();
    const NUM_LEAVES: usize = 2 ^ 24;
    const NUM_NEW_LEAVES: usize = 1;
    let leaves = generate_leaves!(NUM_LEAVES, &mut rng);
    let new_leaves = generate_leaves!(NUM_NEW_LEAVES, &mut rng);
    let base_tree = MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves).unwrap();

    group.bench_function("without_preserved_allocation", |b| {
        b.iter_batched(
            || base_tree.clone(),
            |tree| {
                tree.prepare_append(&new_leaves).unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("with_preserved_allocation", |b| {
        b.iter_batched(
            || {
                let mut t = base_tree.clone();
                let with_preserved = t.prepare_append(&[]).unwrap();
                with_preserved.preserve_tree_allocation(&mut t);
                with_preserved
            },
            |tree| {
                tree.prepare_append(&new_leaves).unwrap();
            },
            BatchSize::SmallInput,
        )
    });
}

fn update_vs_update_many(c: &mut Criterion) {
    let mut group = c.benchmark_group("UpdateVSUpdateMany");
    let mut rng = TestRng::default();
    // Accumulate leaves in a vector to avoid recomputing across iterations.
    let max_leaves = 2usize.saturating_pow(MAX_INSTANTIATED_DEPTH as u32);
    let leaves = generate_leaves!(max_leaves, &mut rng);
    for depth in 1..=MAX_INSTANTIATED_DEPTH {
        // Compute the number of leaves at this depth.
        let num_leaves = 2usize.saturating_pow(depth as u32);
        // Construct a Merkle tree with the specified number of leaves.
        let tree = MainnetV0::merkle_tree_bhp::<DEPTH>(&leaves[..num_leaves]).unwrap();
        // Generate a new leaf and select a random index to update.
        let index: usize = Uniform::rand(&mut rng);
        let index = index % num_leaves;
        let new_leaf = generate_leaves!(1, &mut rng).pop().unwrap();
        // Benchmark the standard update operation.
        group.bench_with_input(BenchmarkId::new("Single", format!("{depth}")), &new_leaf, |b, new_leaf| {
            b.iter_batched(|| tree.clone(), |mut tree| tree.update(index, new_leaf), BatchSize::SmallInput)
        });
        // Benchmark the `update_many` operation.
        group.bench_with_input(
            BenchmarkId::new("Batch", format!("{depth}")),
            &BTreeMap::from([(index, new_leaf)]),
            |b, updates| b.iter_batched(|| tree.clone(), |mut tree| tree.update_many(updates), BatchSize::SmallInput),
        );
    }
}

criterion_group! {
    name = merkle_tree;
    config = Criterion::default().sample_size(10);
    targets = new, append, update, update_many, creation_65536_vs_65537_leaves, creation_16777216_vs_16777217_leaves, append_pow16_leaves, append_pow24_leaves, prepare_append_with_vs_without_preserved_allocation, update_vs_update_many
}
criterion_main!(merkle_tree);
