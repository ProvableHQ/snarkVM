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

use super::*;
use snarkvm_console_algorithms::{Poseidon, BHP1024, BHP512};
use snarkvm_console_types::prelude::Console;

type CurrentEnvironment = Console;

const ITERATIONS: u128 = 10;

/// Runs the following test:
/// 1. Construct the Merkle tree for the leaves.
/// 2. Add the additional leaves to the a clone of the original Merkle tree.
/// 3. Remove the additional leaves from the clone of the original Merkle tree.
/// 4. Check that the two Merkle trees are equal.
fn check_merkle_tree<E: Environment, LH: LeafHash<Hash = PH::Hash>, PH: PathHash<Hash = Field<E>>, const DEPTH: u8>(
    leaf_hasher: &LH,
    path_hasher: &PH,
    leaves: &[LH::Leaf],
    additional_leaves: &[LH::Leaf],
) -> Result<()> {
    // Construct the Merkle tree for the given leaves.
    let merkle_tree = MerkleTree::<E, LH, PH, DEPTH>::new(leaf_hasher, path_hasher, leaves)?;
    assert_eq!(leaves.len(), merkle_tree.number_of_leaves);

    // Append additional leaves to a new Merkle tree.
    let mut new_merkle_tree = merkle_tree.prepare_append(additional_leaves)?;
    assert_eq!(leaves.len() + additional_leaves.len(), new_merkle_tree.number_of_leaves);

    // Remove the additional leaves from the new Merkle tree.
    if !additional_leaves.is_empty() {
        new_merkle_tree.remove_last_n(additional_leaves.len())?;
    }
    assert_eq!(new_merkle_tree.number_of_leaves, merkle_tree.number_of_leaves);

    // Ensure that the new Merkle tree has the same root as the original Merkle tree.
    assert_eq!(new_merkle_tree.root(), merkle_tree.root());

    // Ensure that the new merkle tree has the same values as the original Merkle tree.
    assert_eq!(new_merkle_tree.tree(), merkle_tree.tree());

    // Check each leaf in the new Merkle tree is equivalent to the original Merkle tree.
    if !leaves.is_empty() {
        for (leaf_index, leaf) in leaves.iter().enumerate() {
            // Compute a Merkle proof for the leaf.
            let proof = merkle_tree.prove(leaf_index, leaf)?;

            // Compute a Merkle proof for the leaf using the new merkle tree.
            let new_proof = new_merkle_tree.prove(leaf_index, leaf)?;

            // Ensure that the proofs are equivalent.
            assert_eq!(proof, new_proof);
        }
    }

    Ok(())
}

#[test]
fn test_merkle_tree_bhp_remove() -> Result<()> {
    fn run_test<const DEPTH: u8>(rng: &mut TestRng) -> Result<()> {
        type LH = BHP1024<CurrentEnvironment>;
        type PH = BHP512<CurrentEnvironment>;

        let leaf_hasher = LH::setup("AleoMerkleTreeTest0")?;
        let path_hasher = PH::setup("AleoMerkleTreeTest1")?;

        for i in 0..ITERATIONS {
            for j in 0..ITERATIONS {
                // Determine the leaves and additional leaves.
                let num_leaves = core::cmp::min(2u128.pow(DEPTH as u32), i);
                let num_additional_leaves = core::cmp::min(2u128.pow(DEPTH as u32) - num_leaves, j);

                // Check the Merkle tree.
                check_merkle_tree::<CurrentEnvironment, LH, PH, DEPTH>(
                    &leaf_hasher,
                    &path_hasher,
                    &(0..num_leaves)
                        .map(|_| Field::<CurrentEnvironment>::rand(rng).to_bits_le())
                        .collect::<Vec<Vec<bool>>>(),
                    &(0..num_additional_leaves)
                        .map(|_| Field::<CurrentEnvironment>::rand(rng).to_bits_le())
                        .collect::<Vec<Vec<bool>>>(),
                )?;
            }
        }

        // Test removing many leaves from the merkle tree (spanning powers of two).
        for i in 1..ITERATIONS {
            if i >= DEPTH as u128 {
                continue;
            }

            // Determine the leaves and additional leaves.
            let limit_depth = core::cmp::min(DEPTH, 16).saturating_sub(u8::try_from(i)?);
            let num_leaves = core::cmp::min(2u128.pow(DEPTH as u32), 2u128.pow(limit_depth as u32));

            let next_power_of_two = (0..i).fold(num_leaves, |acc, _| acc.next_power_of_two());
            let num_additional_leaves = core::cmp::min(2u128.pow(DEPTH as u32) - num_leaves, next_power_of_two);

            // Check the Merkle tree.
            check_merkle_tree::<CurrentEnvironment, LH, PH, DEPTH>(
                &leaf_hasher,
                &path_hasher,
                &(0..num_leaves)
                    .map(|_| Field::<CurrentEnvironment>::rand(rng).to_bits_le())
                    .collect::<Vec<Vec<bool>>>(),
                &(0..num_additional_leaves)
                    .map(|_| Field::<CurrentEnvironment>::rand(rng).to_bits_le())
                    .collect::<Vec<Vec<bool>>>(),
            )?;
        }

        Ok(())
    }

    let mut rng = TestRng::default();

    // Ensure DEPTH = 0 fails.
    assert!(run_test::<0>(&mut rng).is_err());
    // Spot check important depths.
    assert!(run_test::<1>(&mut rng).is_ok());
    assert!(run_test::<2>(&mut rng).is_ok());
    assert!(run_test::<3>(&mut rng).is_ok());
    assert!(run_test::<4>(&mut rng).is_ok());
    assert!(run_test::<5>(&mut rng).is_ok());
    assert!(run_test::<6>(&mut rng).is_ok());
    assert!(run_test::<7>(&mut rng).is_ok());
    assert!(run_test::<8>(&mut rng).is_ok());
    assert!(run_test::<9>(&mut rng).is_ok());
    assert!(run_test::<10>(&mut rng).is_ok());
    assert!(run_test::<15>(&mut rng).is_ok());
    assert!(run_test::<16>(&mut rng).is_ok());
    assert!(run_test::<17>(&mut rng).is_ok());
    assert!(run_test::<31>(&mut rng).is_ok());
    assert!(run_test::<32>(&mut rng).is_ok());
    assert!(run_test::<64>(&mut rng).is_ok());
    Ok(())
}

#[test]
fn test_merkle_tree_poseidon_remove() -> Result<()> {
    fn run_test<const DEPTH: u8>(rng: &mut TestRng) -> Result<()> {
        type LH = Poseidon<CurrentEnvironment, 4>;
        type PH = Poseidon<CurrentEnvironment, 2>;

        let leaf_hasher = LH::setup("AleoMerkleTreeTest0")?;
        let path_hasher = PH::setup("AleoMerkleTreeTest1")?;

        for i in 0..ITERATIONS {
            for j in 0..ITERATIONS {
                // Determine the leaves and additional leaves.
                let num_leaves = core::cmp::min(2u128.pow(DEPTH as u32), i);
                let num_additional_leaves = core::cmp::min(2u128.pow(DEPTH as u32) - num_leaves, j);

                // Check the Merkle tree.
                check_merkle_tree::<CurrentEnvironment, LH, PH, DEPTH>(
                    &leaf_hasher,
                    &path_hasher,
                    &(0..num_leaves).map(|_| vec![Uniform::rand(rng)]).collect::<Vec<_>>(),
                    &(0..num_additional_leaves).map(|_| vec![Uniform::rand(rng)]).collect::<Vec<_>>(),
                )?;
            }
        }

        // Test removing many leaves from the merkle tree (spanning powers of two).
        for i in 1..ITERATIONS {
            if i >= DEPTH as u128 {
                continue;
            }

            // Determine the leaves and additional leaves.
            let limit_depth = core::cmp::min(DEPTH, 16).saturating_sub(u8::try_from(i)?);
            let num_leaves = core::cmp::min(2u128.pow(DEPTH as u32), 2u128.pow(limit_depth as u32));

            let next_power_of_two = (0..i).fold(num_leaves, |acc, _| acc.next_power_of_two());
            let num_additional_leaves = core::cmp::min(2u128.pow(DEPTH as u32) - num_leaves, next_power_of_two);

            // Check the Merkle tree.
            check_merkle_tree::<CurrentEnvironment, LH, PH, DEPTH>(
                &leaf_hasher,
                &path_hasher,
                &(0..num_leaves).map(|_| vec![Uniform::rand(rng)]).collect::<Vec<_>>(),
                &(0..num_additional_leaves).map(|_| vec![Uniform::rand(rng)]).collect::<Vec<_>>(),
            )?;
        }

        Ok(())
    }

    let mut rng = TestRng::default();

    // Ensure DEPTH = 0 fails.
    assert!(run_test::<0>(&mut rng).is_err());
    // Spot check important depths.
    assert!(run_test::<1>(&mut rng).is_ok());
    assert!(run_test::<2>(&mut rng).is_ok());
    assert!(run_test::<3>(&mut rng).is_ok());
    assert!(run_test::<4>(&mut rng).is_ok());
    assert!(run_test::<5>(&mut rng).is_ok());
    assert!(run_test::<6>(&mut rng).is_ok());
    assert!(run_test::<7>(&mut rng).is_ok());
    assert!(run_test::<8>(&mut rng).is_ok());
    assert!(run_test::<9>(&mut rng).is_ok());
    assert!(run_test::<10>(&mut rng).is_ok());
    assert!(run_test::<15>(&mut rng).is_ok());
    assert!(run_test::<16>(&mut rng).is_ok());
    assert!(run_test::<17>(&mut rng).is_ok());
    assert!(run_test::<31>(&mut rng).is_ok());
    assert!(run_test::<32>(&mut rng).is_ok());
    assert!(run_test::<64>(&mut rng).is_ok());
    Ok(())
}
