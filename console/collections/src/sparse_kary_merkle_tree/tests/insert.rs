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
use std::collections::BTreeMap;

type CurrentEnvironment = Console;

const ITERATIONS: u128 = 3;

/// Runs the following test:
/// 1. Construct an empty Sparse K-ary Merkle tree.
/// 2. Insert key-value pairs.
/// 3. Check that the Merkle proof for every key is valid.
fn check_sparse_kary_merkle_tree_insert<
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
    rng: &mut TestRng,
) -> Result<()> {
    // Construct an empty Sparse K-ary Merkle tree.
    let mut sparse_kary_merkle_tree = SparseKaryMerkleTree::<E, PH, KH, LH, DEPTH, ARITY>::new(path_hasher, key_hasher, leaf_hasher, false)?;
    assert!(sparse_kary_merkle_tree.is_empty());
    assert_eq!(0, sparse_kary_merkle_tree.len());

    // Insert each entry.
    for (key, value) in entries {
        sparse_kary_merkle_tree.insert(key.clone(), value.clone())?;
    }

    assert_eq!(entries.len(), sparse_kary_merkle_tree.len());
    if !entries.is_empty() {
        assert!(!sparse_kary_merkle_tree.is_empty());
    }

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

    Ok(())
}

#[test]
fn test_sparse_kary_merkle_tree_insert_bhp() -> Result<()> {
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

            // Create entries with unique keys to avoid collisions.
            let entries: Vec<_> = (0..num_entries)
                .map(|i| {
                    let key = generate_unique_bool_key::<DEPTH, 2>(i as u64);
                    let value = Field::<CurrentEnvironment>::rand(rng).to_bits_le();
                    (key, value)
                })
                .collect();

            // Check the Sparse K-ary Merkle tree.
            check_sparse_kary_merkle_tree_insert::<CurrentEnvironment, KH, LH, PH, DEPTH, 2>(
                &key_hasher,
                &leaf_hasher,
                &path_hasher,
                &entries,
                rng,
            )?;
        }
        Ok(())
    }

    let mut rng = TestRng::default();

    // Ensure DEPTH = 0 fails.
    assert!(run_test::<0>(&mut rng).is_err());
    // Test critical depths for sparse k-ary trees.
    run_tests!(&mut rng, [1, 2, 3, 4, 5, 7, 8, 10]);
    Ok(())
}

#[test]
fn test_sparse_kary_merkle_tree_insert_poseidon() -> Result<()> {
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

            // Create entries with unique Field keys to avoid collisions.
            let entries: Vec<_> = (0..num_entries)
                .map(|i| {
                    let key = generate_unique_field_key::<CurrentEnvironment>(i);
                    let value = vec![Field::<CurrentEnvironment>::rand(rng)];
                    (key, value)
                })
                .collect();

            // Check the Sparse K-ary Merkle tree.
            check_sparse_kary_merkle_tree_insert::<CurrentEnvironment, KH, LH, PH, DEPTH, 2>(
                &key_hasher,
                &leaf_hasher,
                &path_hasher,
                &entries,
                rng,
            )?;
        }
        Ok(())
    }

    let mut rng = TestRng::default();

    // Ensure DEPTH = 0 fails.
    assert!(run_test::<0>(&mut rng).is_err());
    // Test critical depths for sparse k-ary trees.
    run_tests!(&mut rng, [1, 2, 3, 4, 5, 7, 8, 10]);
    Ok(())
}

#[test]
fn test_sparse_kary_merkle_tree_insert_empty() -> Result<()> {
    type KH = BHP1024<CurrentEnvironment>;
    type LH = BHP1024<CurrentEnvironment>;
    type PH = BHP512<CurrentEnvironment>;

    let key_hasher = KH::setup("SparseKaryKeyHash0")?;
    let leaf_hasher = LH::setup("SparseKaryLeafHash0")?;
    let path_hasher = PH::setup("SparseKaryPathHash0")?;

    // Test empty tree.
    let sparse_kary_merkle_tree = SparseKaryMerkleTree::<CurrentEnvironment, PH, KH, LH, 32, 2>::new(&path_hasher, &key_hasher, &leaf_hasher, false)?;
    assert!(sparse_kary_merkle_tree.is_empty());
    assert_eq!(0, sparse_kary_merkle_tree.len());

    // Test single entry.
    let mut rng = TestRng::default();
    let key = Field::<CurrentEnvironment>::rand(&mut rng).to_bits_le();
    let value = Field::<CurrentEnvironment>::rand(&mut rng).to_bits_le();

    let mut sparse_kary_merkle_tree = SparseKaryMerkleTree::<CurrentEnvironment, PH, KH, LH, 32, 2>::new(&path_hasher, &key_hasher, &leaf_hasher, false)?;
    sparse_kary_merkle_tree.insert(key.clone(), value.clone())?;
    assert!(!sparse_kary_merkle_tree.is_empty());
    assert_eq!(1, sparse_kary_merkle_tree.len());

    let proof = sparse_kary_merkle_tree.prove(&key)?;
    assert!(sparse_kary_merkle_tree.verify(&proof, sparse_kary_merkle_tree.root(), &key, &value));

    Ok(())
}

