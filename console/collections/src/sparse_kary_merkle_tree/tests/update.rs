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

const ITERATIONS: u128 = 3;

/// Runs the following test:
/// 1. Construct the Sparse K-ary Merkle tree for the entries.
/// 2. Check that the Merkle proof for every entry is valid.
/// 3. Update entries in the Sparse K-ary Merkle tree.
/// 4. Check that the Merkle proof for every updated entry is valid.
fn check_sparse_kary_merkle_tree_update<
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
    updates: &[(KH::Key, LH::Leaf)],
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
        // Create an invalid root by hashing ARITY default values
        let invalid_children = vec![PH::Hash::default(); ARITY as usize];
        let invalid_root = path_hasher.hash_children(&invalid_children)?;
        assert!(!proof.verify(key_hasher, leaf_hasher, path_hasher, &invalid_root, key, value));
    }

    // Update the entries of the Sparse K-ary Merkle tree.
    for (key, value) in updates {
        sparse_kary_merkle_tree.update(key, value.clone())?;
    }

    // Check each updated entry in the Sparse K-ary Merkle tree.
    for (key, value) in updates {
        // Compute a Merkle proof for the key.
        let proof = sparse_kary_merkle_tree.prove(key)?;

        // Verify the Merkle proof succeeds.
        assert!(sparse_kary_merkle_tree.verify(&proof, sparse_kary_merkle_tree.root(), key, value));
        assert!(proof.verify(key_hasher, leaf_hasher, path_hasher, sparse_kary_merkle_tree.root(), key, value));

        // Verify the Merkle proof **fails** on an invalid root.
        assert!(!proof.verify(key_hasher, leaf_hasher, path_hasher, &PH::Hash::default(), key, value));
        // Create an invalid root by hashing ARITY default values
        let invalid_children = vec![PH::Hash::default(); ARITY as usize];
        let invalid_root = path_hasher.hash_children(&invalid_children)?;
        assert!(!proof.verify(key_hasher, leaf_hasher, path_hasher, &invalid_root, key, value));
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
    updates: Vec<(KH::Key, LH::Leaf)>,
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
    for (key, value) in &updates {
        sparse_kary_merkle_tree.update(key, value.clone())?;
    }

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
fn test_sparse_kary_merkle_tree_update_bhp() -> Result<()> {
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
                let updates: Vec<_> = (0..num_updates)
                    .map(|i| {
                        let idx = (i % entries.len() as u128) as usize;
                        let key = entries[idx].0.clone();
                        let value = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                        (key, value)
                    })
                    .collect();

                // Check the Sparse K-ary Merkle tree.
                check_sparse_kary_merkle_tree_update::<CurrentEnvironment, KH, LH, PH, DEPTH, 2>(
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
    run_tests!(&mut rng, [1, 2, 3, 4, 5, 7, 8, 10]);
    Ok(())
}

#[test]
fn test_sparse_kary_merkle_tree_update_poseidon() -> Result<()> {
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
                let updates: Vec<_> = (0..num_updates)
                    .map(|i| {
                        let idx = (i % entries.len() as u128) as usize;
                        let key = entries[idx].0;
                        let value = vec![Field::<CurrentEnvironment>::rand(rng)];
                        (key, value)
                    })
                    .collect();

                // Check the Sparse K-ary Merkle tree.
                check_sparse_kary_merkle_tree_update::<CurrentEnvironment, KH, LH, PH, DEPTH, 2>(
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
    run_tests!(&mut rng, [1, 2, 3, 4, 5, 7, 8, 10]);
    Ok(())
}

#[test]
fn test_sparse_kary_merkle_tree_update_is_consistent_bhp() -> Result<()> {
    fn run_test<const DEPTH: u8>(rng: &mut TestRng) -> Result<()> {
        type KH = BHP1024<CurrentEnvironment>;
        type LH = BHP1024<CurrentEnvironment>;
        type PH = BHP512<CurrentEnvironment>;

        let key_hasher = KH::setup("SparseKaryKeyHash0")?;
        let leaf_hasher = LH::setup("SparseKaryLeafHash0")?;
        let path_hasher = PH::setup("SparseKaryPathHash0")?;

        for _ in 0..ITERATIONS {
            // Determine the number of entries.
            let num_entries = std::cmp::min(2u128.pow(DEPTH as u32), 1000);

            // Create entries.
            let entries: Vec<_> = (0..num_entries)
                .map(|_| {
                    let key = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                    let value = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                    (key, value)
                })
                .collect();

            // Create updates.
            let updates: Vec<_> = (0..num_entries)
                .map(|i| {
                    let idx = (i % entries.len() as u128) as usize;
                    let key = entries[idx].0.clone();
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
    run_tests!(&mut rng, [1, 2, 3, 4, 5, 7, 8]);
    Ok(())
}

#[test]
fn test_sparse_kary_merkle_tree_update_nonexistent_key() -> Result<()> {
    type KH = BHP1024<CurrentEnvironment>;
    type LH = BHP1024<CurrentEnvironment>;
    type PH = BHP512<CurrentEnvironment>;

    let key_hasher = KH::setup("SparseKaryKeyHash0")?;
    let leaf_hasher = LH::setup("SparseKaryLeafHash0")?;
    let path_hasher = PH::setup("SparseKaryPathHash0")?;

    let mut rng = TestRng::default();

    // Create a Sparse K-ary Merkle tree with one entry.
    let key = Field::<CurrentEnvironment>::rand(&mut rng).to_bits_le();
    let value = Field::<CurrentEnvironment>::rand(&mut rng).to_bits_le();
    let mut sparse_kary_merkle_tree = SparseKaryMerkleTree::<CurrentEnvironment, PH, KH, LH, 32, 2>::new_with_entries(
        &path_hasher,
        &key_hasher,
        &leaf_hasher,
        &[(key.clone(), value.clone())],
        false,
    )?;

    // Try to update a non-existent key - should fail.
    let nonexistent_key = Field::<CurrentEnvironment>::rand(&mut rng).to_bits_le();
    let new_value = Field::<CurrentEnvironment>::rand(&mut rng).to_bits_le();
    assert!(sparse_kary_merkle_tree.update(&nonexistent_key, new_value).is_err());

    Ok(())
}

