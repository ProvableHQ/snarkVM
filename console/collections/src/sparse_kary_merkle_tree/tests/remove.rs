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
/// 2. Remove entries from the Sparse K-ary Merkle tree.
/// 3. Check that removed entries cannot be proven.
fn check_sparse_kary_merkle_tree_remove<
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
    removals: &[KH::Key],
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

    // Remove entries from the Sparse K-ary Merkle tree.
    for key in removals {
        sparse_kary_merkle_tree.remove(key)?;
    }

    assert_eq!(entries.len() - removals.len(), sparse_kary_merkle_tree.len());

    // Check that removed entries cannot be proven.
    for key in removals {
        assert!(sparse_kary_merkle_tree.prove(key).is_err());
    }

    // Check that remaining entries can still be proven.
    let removals_set: std::collections::BTreeSet<_> = removals.iter().cloned().collect();
    let remaining_keys: Vec<_> = entries
        .iter()
        .map(|(k, _)| k)
        .filter(|k| !removals_set.contains(*k))
        .collect();

    for key in remaining_keys {
        let value = entries.iter().find(|(k, _)| k == key).unwrap().1.clone();
        let proof = sparse_kary_merkle_tree.prove(key)?;
        assert!(sparse_kary_merkle_tree.verify(&proof, sparse_kary_merkle_tree.root(), key, &value));
    }

    Ok(())
}

#[test]
fn test_sparse_kary_merkle_tree_remove_bhp() -> Result<()> {
    fn run_test<const DEPTH: u8>(rng: &mut TestRng) -> Result<()> {
        type KH = BHP1024<CurrentEnvironment>;
        type LH = BHP1024<CurrentEnvironment>;
        type PH = BHP512<CurrentEnvironment>;

        let key_hasher = KH::setup("SparseKaryKeyHash0")?;
        let leaf_hasher = LH::setup("SparseKaryLeafHash0")?;
        let path_hasher = PH::setup("SparseKaryPathHash0")?;

        for i in 0..ITERATIONS {
            // Determine the number of entries.
            let num_entries = core::cmp::min(2u128.pow(DEPTH as u32), i);
            if num_entries == 0 {
                continue;
            }

            // Create entries.
            let entries: Vec<_> = (0..num_entries)
                .map(|_| {
                    let key = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                    let value = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                    (key, value)
                })
                .collect();

            // Create removals (remove some entries).
            let num_removals = core::cmp::min(num_entries / 2, 10) as usize;
            let removals: Vec<_> = entries.iter().take(num_removals).map(|(k, _)| k.clone()).collect();

            // Check the Sparse K-ary Merkle tree.
            check_sparse_kary_merkle_tree_remove::<CurrentEnvironment, KH, LH, PH, DEPTH, 2>(
                &key_hasher,
                &leaf_hasher,
                &path_hasher,
                &entries,
                &removals,
            )?;
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
fn test_sparse_kary_merkle_tree_remove_poseidon() -> Result<()> {
    fn run_test<const DEPTH: u8>(rng: &mut TestRng) -> Result<()> {
        type KH = Poseidon<CurrentEnvironment, 4>;
        type LH = Poseidon<CurrentEnvironment, 4>;
        type PH = Poseidon<CurrentEnvironment, 2>;

        let key_hasher = KH::setup("SparseKaryKeyHash0")?;
        let leaf_hasher = LH::setup("SparseKaryLeafHash0")?;
        let path_hasher = PH::setup("SparseKaryPathHash0")?;

        for i in 0..ITERATIONS {
            // Determine the number of entries.
            let num_entries = core::cmp::min(2u128.pow(DEPTH as u32), i);
            if num_entries == 0 {
                continue;
            }

            // Create entries.
            let entries: Vec<_> = (0..num_entries)
                .map(|_| {
                    let key = Field::<CurrentEnvironment>::rand(rng);
                    let value = vec![Field::<CurrentEnvironment>::rand(rng)];
                    (key, value)
                })
                .collect();

            // Create removals (remove some entries).
            let num_removals = core::cmp::min(num_entries / 2, 10) as usize;
            let removals: Vec<_> = entries.iter().take(num_removals).map(|(k, _)| k.clone()).collect();

            // Check the Sparse K-ary Merkle tree.
            check_sparse_kary_merkle_tree_remove::<CurrentEnvironment, KH, LH, PH, DEPTH, 2>(
                &key_hasher,
                &leaf_hasher,
                &path_hasher,
                &entries,
                &removals,
            )?;
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
fn test_sparse_kary_merkle_tree_remove_nonexistent_key() -> Result<()> {
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

    // Try to remove a non-existent key - should fail.
    let nonexistent_key = Field::<CurrentEnvironment>::rand(&mut rng).to_bits_le();
    assert!(sparse_kary_merkle_tree.remove(&nonexistent_key).is_err());

    Ok(())
}

