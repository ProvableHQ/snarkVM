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

use super::*;
use snarkvm_console_algorithms::{BHP512, BHP1024, Poseidon};
use snarkvm_console_types::prelude::Console;

type CurrentEnvironment = Console;

const ITERATIONS: u128 = 10;

/// Runs the following test:
/// 1. Construct the Sparse K-ary Merkle tree for the entries.
/// 2. Check that the Merkle proof for every entry is valid.
/// 3. Update entries in the Sparse K-ary Merkle tree using update_many.
/// 4. Check that the Merkle proof for every updated entry is valid.
fn check_sparse_kary_merkle_tree_update_many<
    E: Environment,
    KH: KeyHash<Hash = PH::Hash>,
    LH: LeafHash<Hash = PH::Hash>,
    PH: PathHash,
    const DEPTH: u8,
    const ARITY: u8,
>(
    key_hasher: &KH,
    leaf_hasher: &LH,
    path_hasher: &PH,
    entries: &[(KH::Key, LH::Leaf)],
    updates: &BTreeMap<KH::Key, LH::Leaf>,
    rng: &mut TestRng,
) -> Result<()> {
    // Construct the Sparse K-ary Merkle tree for the given entries.
    let mut sparse_kary_merkle_tree = SparseKaryMerkleTree::<E, PH, KH, LH, DEPTH, ARITY>::new_with_entries(
        path_hasher,
        key_hasher,
        leaf_hasher,
        entries,
        false,
    )?;
    assert_eq!(entries.len(), sparse_kary_merkle_tree.len());

    // Check each entry in the Sparse K-ary Merkle tree.
    for (key, value) in entries {
        // Compute a Merkle proof for the key.
        let proof = sparse_kary_merkle_tree.prove(key)?;

        // Verify the Merkle proof succeeds.
        assert!(sparse_kary_merkle_tree.verify(&proof, sparse_kary_merkle_tree.root(), key, value));
        assert!(proof.verify(key_hasher, leaf_hasher, path_hasher, sparse_kary_merkle_tree.root(), key, value));

        // Verify the Merkle proof **fails** on an invalid root.
        assert!(!proof.verify(key_hasher, leaf_hasher, path_hasher, &PH::Hash::default(), key, value));
        assert!(!proof.verify(key_hasher, leaf_hasher, path_hasher, &{ let invalid_children = vec![PH::Hash::default(); 2]; path_hasher.hash_children(&invalid_children).unwrap() }, key, value));
    }

    // If additional entries are provided, check that the Sparse K-ary Merkle tree is consistent with them.
    if !updates.is_empty() {
        // Update the entries of the Sparse K-ary Merkle tree.
        sparse_kary_merkle_tree.update_many(updates)?;

        // Check each updated entry in the Sparse K-ary Merkle tree.
        for (key, value) in updates {
            // Compute a Merkle proof for the key.
            let proof = sparse_kary_merkle_tree.prove(key)?;

            // Verify the Merkle proof succeeds.
            assert!(sparse_kary_merkle_tree.verify(&proof, sparse_kary_merkle_tree.root(), key, value));
            assert!(proof.verify(key_hasher, leaf_hasher, path_hasher, sparse_kary_merkle_tree.root(), key, value));

            // Verify the Merkle proof **fails** on an invalid root.
            assert!(!proof.verify(key_hasher, leaf_hasher, path_hasher, &PH::Hash::default(), key, value));
            assert!(!proof.verify(key_hasher, leaf_hasher, path_hasher, &{ let invalid_children = vec![PH::Hash::default(); 2]; path_hasher.hash_children(&invalid_children).unwrap() }, key, value));
        }
    }
    Ok(())
}

/// Runs the following test:
/// 1. Construct a Sparse K-ary Merkle tree of a given depth with a given number of entries.
/// 2. Apply the updates to the Sparse K-ary Merkle tree.
/// 3. Construct a new Sparse K-ary Merkle tree with the updated entries.
/// 4. Check that the Merkle root of the new Sparse K-ary Merkle tree is the same as the Merkle root of the original Sparse K-ary Merkle tree.
fn check_updated_sparse_kary_merkle_tree_is_consistent<
    E: Environment,
    KH: KeyHash<Hash = PH::Hash>,
    LH: LeafHash<Hash = PH::Hash>,
    PH: PathHash,
    const DEPTH: u8,
    const ARITY: u8,
>(
    key_hasher: &KH,
    leaf_hasher: &LH,
    path_hasher: &PH,
    entries: Vec<(KH::Key, LH::Leaf)>,
    updates: BTreeMap<KH::Key, LH::Leaf>,
) -> Result<()> {
    // Construct the Sparse K-ary Merkle tree for the given entries.
    let mut sparse_kary_merkle_tree = SparseKaryMerkleTree::<E, PH, KH, LH, DEPTH, ARITY>::new_with_entries(
        path_hasher,
        key_hasher,
        leaf_hasher,
        &entries,
        false,
    )?;
    assert_eq!(entries.len(), sparse_kary_merkle_tree.len());

    // Apply the updates to the Sparse K-ary Merkle tree.
    sparse_kary_merkle_tree.update_many(&updates)?;

    // Construct a map with updated entries.
    let mut updated_entries_map: BTreeMap<_, _> = entries.into_iter().collect();
    for (key, value) in updates {
        updated_entries_map.insert(key, value);
    }

    // Get the updated entries.
    let updated_entries: Vec<_> = updated_entries_map.into_iter().collect();

    // Construct a new Sparse K-ary Merkle tree with the updated entries.
    let updated_sparse_kary_merkle_tree = SparseKaryMerkleTree::<E, PH, KH, LH, DEPTH, ARITY>::new_with_entries(
        path_hasher,
        key_hasher,
        leaf_hasher,
        &updated_entries,
        false,
    )?;

    // Check that the Merkle root of the new Sparse K-ary Merkle tree is the same as the Merkle root of the original Sparse K-ary Merkle tree.
    assert_eq!(sparse_kary_merkle_tree.root(), updated_sparse_kary_merkle_tree.root());
    Ok(())
}

#[test]
fn test_sparse_kary_merkle_tree_update_many_bhp() -> Result<()> {
    fn run_test<const DEPTH: u8>(rng: &mut TestRng) -> Result<()> {
        type KH = BHP1024<CurrentEnvironment>;
        type LH = BHP1024<CurrentEnvironment>;
        type PH = BHP512<CurrentEnvironment>;

        let key_hasher = KH::setup("SparseKaryKeyHash0")?;
        let leaf_hasher = LH::setup("SparseKaryLeafHash0")?;
        let path_hasher = PH::setup("SparseKaryPathHash0")?;

        for i in 0..ITERATIONS {
            for j in 0..ITERATIONS {
                // Determine the entries and updates.
                let num_entries = core::cmp::min(2u128.pow(DEPTH as u32), i);
                let num_updates = core::cmp::min(num_entries, core::cmp::min(2u128.pow(DEPTH as u32) - num_entries, j));

                // Create entries.
                let entries: Vec<_> = (0..num_entries)
                    .map(|_| {
                        let key = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                        let value = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                        (key, value)
                    })
                    .collect();

                // Create updates (reuse keys from entries).
                let updates: BTreeMap<_, _> = (0..num_updates)
                    .rev()
                    .map(|i| {
                        let idx = (i % entries.len() as u128) as usize;
                        let key = entries[idx].0.clone();
                        let value = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                        (key, value)
                    })
                    .collect();

                // Check the Sparse K-ary Merkle tree.
                check_sparse_kary_merkle_tree_update_many::<CurrentEnvironment, KH, LH, PH, DEPTH, 2>(
                    &key_hasher,
                    &leaf_hasher,
                    &path_hasher,
                    &entries,
                    &updates,
                    rng,
                )?;
            }
        }
        Ok(())
    }

    let mut rng = TestRng::default();

    // Ensure DEPTH = 0 fails.
    assert!(run_test::<0>(&mut rng).is_err());
    // Spot check important depths.
    run_tests!(&mut rng, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 15, 16, 17, 31, 32, 64]);
    Ok(())
}

#[test]
fn test_sparse_kary_merkle_tree_update_many_poseidon() -> Result<()> {
    fn run_test<const DEPTH: u8>(rng: &mut TestRng) -> Result<()> {
        type KH = Poseidon<CurrentEnvironment, 4>;
        type LH = Poseidon<CurrentEnvironment, 4>;
        type PH = Poseidon<CurrentEnvironment, 2>;

        let key_hasher = KH::setup("SparseKaryKeyHash0")?;
        let leaf_hasher = LH::setup("SparseKaryLeafHash0")?;
        let path_hasher = PH::setup("SparseKaryPathHash0")?;

        for i in 0..ITERATIONS {
            for j in 0..ITERATIONS {
                // Determine the entries and updates.
                let num_entries = core::cmp::min(2u128.pow(DEPTH as u32), i);
                let num_updates = core::cmp::min(num_entries, core::cmp::min(2u128.pow(DEPTH as u32) - num_entries, j));

                // Create entries.
                let entries: Vec<_> = (0..num_entries)
                    .map(|_| {
                        let key = Field::<CurrentEnvironment>::rand(rng);
                        let value = vec![Field::<CurrentEnvironment>::rand(rng)];
                        (key, value)
                    })
                    .collect();

                // Create updates (reuse keys from entries).
                let updates: BTreeMap<_, _> = (0..num_updates)
                    .rev()
                    .map(|i| {
                        let idx = (i % entries.len() as u128) as usize;
                        let key = entries[idx].0;
                        let value = vec![Field::<CurrentEnvironment>::rand(rng)];
                        (key, value)
                    })
                    .collect();

                // Check the Sparse K-ary Merkle tree.
                check_sparse_kary_merkle_tree_update_many::<CurrentEnvironment, KH, LH, PH, DEPTH, 2>(
                    &key_hasher,
                    &leaf_hasher,
                    &path_hasher,
                    &entries,
                    &updates,
                    rng,
                )?;
            }
        }
        Ok(())
    }

    let mut rng = TestRng::default();

    // Ensure DEPTH = 0 fails.
    assert!(run_test::<0>(&mut rng).is_err());
    // Spot check important depths.
    run_tests!(&mut rng, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 15, 16, 17, 31, 32, 64]);
    Ok(())
}

#[test]
fn test_sparse_kary_merkle_tree_update_many_is_consistent_bhp() -> Result<()> {
    fn run_test<const DEPTH: u8>(rng: &mut TestRng) -> Result<()> {
        type KH = BHP1024<CurrentEnvironment>;
        type LH = BHP1024<CurrentEnvironment>;
        type PH = BHP512<CurrentEnvironment>;

        let key_hasher = KH::setup("SparseKaryKeyHash0")?;
        let leaf_hasher = LH::setup("SparseKaryLeafHash0")?;
        let path_hasher = PH::setup("SparseKaryPathHash0")?;

        for _ in 0..ITERATIONS {
            // Determine the number of entries.
            let num_entries = 2u128.pow(DEPTH as u32);

            // Create entries.
            let entries: Vec<_> = (0..num_entries)
                .map(|_| {
                    let key = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                    let value = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                    (key, value)
                })
                .collect();

            // Create updates.
            let updates: BTreeMap<_, _> = (0..num_entries)
                .map(|_| {
                    let index: u128 = Uniform::rand(rng);
                    let key = entries[(index % num_entries) as usize].0.clone();
                    let value = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                    (key, value)
                })
                .collect();

            // Check the Sparse K-ary Merkle tree.
            check_updated_sparse_kary_merkle_tree_is_consistent::<CurrentEnvironment, KH, LH, PH, DEPTH, 2>(
                &key_hasher,
                &leaf_hasher,
                &path_hasher,
                entries,
                updates,
            )?;
        }
        Ok(())
    }

    let mut rng = TestRng::default();

    // Ensure DEPTH = 0 fails.
    assert!(run_test::<0>(&mut rng).is_err());
    // Spot check important depths.
    run_tests!(&mut rng, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
    Ok(())
}

#[test]
fn test_sparse_kary_merkle_tree_update_and_update_many_match() -> Result<()> {
    fn run_test<const DEPTH: u8>(rng: &mut TestRng) -> Result<()> {
        type KH = BHP1024<CurrentEnvironment>;
        type LH = BHP1024<CurrentEnvironment>;
        type PH = BHP512<CurrentEnvironment>;

        let key_hasher = KH::setup("SparseKaryKeyHash0")?;
        let leaf_hasher = LH::setup("SparseKaryLeafHash0")?;
        let path_hasher = PH::setup("SparseKaryPathHash0")?;

        for _ in 0..ITERATIONS {
            // Determine the number of entries.
            let num_entries = core::cmp::min(2u128.pow(DEPTH as u32), 256);

            // Create entries.
            let entries: Vec<_> = (0..num_entries)
                .map(|_| {
                    let key = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                    let value = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                    (key, value)
                })
                .collect();

            // Create updates.
            let single_updates: Vec<_> = (0..num_entries)
                .map(|_| {
                    let index: u128 = Uniform::rand(rng);
                    let key = entries[(index % num_entries) as usize].0.clone();
                    let value = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                    (key, value)
                })
                .collect();

            // Construct the batch updates from the single updates.
            let batch_updates: BTreeMap<_, _> = single_updates.iter().cloned().collect();

            // Initialize a Sparse K-ary Merkle tree for single updates.
            let mut sparse_kary_merkle_tree_1 = SparseKaryMerkleTree::<CurrentEnvironment, PH, KH, LH, DEPTH, 2>::new_with_entries(
                &path_hasher,
                &key_hasher,
                &leaf_hasher,
                &entries,
                false,
            )?;
            // Update the Sparse K-ary Merkle tree with single updates.
            for (key, value) in &single_updates {
                sparse_kary_merkle_tree_1.update(key, value.clone())?;
            }

            // Initialize a Sparse K-ary Merkle tree for batch updates.
            let mut sparse_kary_merkle_tree_2 = SparseKaryMerkleTree::<CurrentEnvironment, PH, KH, LH, DEPTH, 2>::new_with_entries(
                &path_hasher,
                &key_hasher,
                &leaf_hasher,
                &entries,
                false,
            )?;
            // Update the Sparse K-ary Merkle tree with batch updates.
            sparse_kary_merkle_tree_2.update_many(&batch_updates)?;

            // Check that the roots of the two Sparse K-ary Merkle trees match.
            assert_eq!(sparse_kary_merkle_tree_1.root(), sparse_kary_merkle_tree_2.root());
        }
        Ok(())
    }

    let mut rng = TestRng::default();

    // Ensure DEPTH = 0 fails.
    assert!(run_test::<0>(&mut rng).is_err());
    // Spot check important depths.
    run_tests!(&mut rng, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 15, 16, 17, 31, 32, 64]);
    Ok(())
}

