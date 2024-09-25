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

use circuit::{collections::kary_merkle_tree::*, AleoV0, Eject, Environment, Inject, Mode};
use console::{
    algorithms::Sha3_256,
    collections::kary_merkle_tree::KaryMerkleTree,
    network::{
        prelude::{TestRng, ToBits, Uniform},
        MainnetV0,
    },
    types::Field,
};
use synthesizer_snark::{ProvingKey, UniversalSRS};

use criterion::Criterion;

type CurrentNetwork = MainnetV0;
type CurrentAleo = AleoV0;

type NativePathHasher = Sha3_256;
type NativeLeafHasher = Sha3_256;
type CircuitPathHasher = circuit::Sha3_256<AleoV0>;
type CircuitLeafHasher = circuit::Sha3_256<AleoV0>;

const DEPTH: u8 = 8;
const ARITY: u8 = 8;

/// Generates the specified number of random Merkle tree leaves.
macro_rules! generate_leaves {
    ($num_leaves:expr, $rng:expr) => {{ (0..$num_leaves).map(|_| Field::<MainnetV0>::rand($rng).to_bits_le()).collect::<Vec<_>>() }};
}

fn batch_prove(c: &mut Criterion) {
    let mut rng = TestRng::default();

    // Initialize the hashers.
    let native_path_hasher = NativePathHasher::default();
    let native_leaf_hasher = NativeLeafHasher::default();
    let circuit_path_hasher = CircuitPathHasher::new();
    let circuit_leaf_hasher = CircuitLeafHasher::new();

    // Determine the maximum number of leaves.
    let max_num_leaves = (ARITY as u32).pow(DEPTH as u32);
    // Initialize the leaves.
    let leaves = generate_leaves!(max_num_leaves, &mut rng);

    // Initialize the tree.
    let merkle_tree =
        KaryMerkleTree::<_, _, DEPTH, ARITY>::new(&native_leaf_hasher, &native_path_hasher, &leaves).unwrap();
    // Initialize the leaf.
    let merkle_leaf = leaves[0].clone();
    // Initialize the Merkle path.
    let merkle_path = merkle_tree.prove(0, &merkle_leaf).unwrap();

    // Start the timer.
    let timer = std::time::Instant::now();

    // Construct the circuit.
    CurrentAleo::reset();

    // Initialize the Merkle path circuit.
    let path =
        KaryMerklePath::<CurrentAleo, circuit::Sha3_256<CurrentAleo>, DEPTH, ARITY>::new(Mode::Private, merkle_path);
    // Initialize the Merkle root.
    let root = <CircuitPathHasher as PathHash<CurrentAleo>>::Hash::new(Mode::Private, *merkle_tree.root());
    // Initialize the Merkle leaf.
    let leaf: Vec<_> = Inject::new(Mode::Private, merkle_leaf);

    // Verify the merkle path.
    let candidate = path.verify(&circuit_leaf_hasher, &circuit_path_hasher, &root, &leaf);
    assert!(candidate.eject_value());

    // Construct the assignment.
    let assignment = CurrentAleo::eject_assignment_and_reset();

    println!(" • Synthesized the circuit in: {} ms", timer.elapsed().as_millis());

    // Load the universal srs.
    let universal_srs = UniversalSRS::<CurrentNetwork>::load().unwrap();
    // Construct the proving key.
    let (proving_key, _) = universal_srs.to_circuit_key("KaryMerklePathVerification", &assignment).unwrap();

    // Bench the proof construction.
    for num_assignments in &[1, 2, 4, 8] {
        // Construct the assignments.
        let assignments =
            [(proving_key.clone(), (0..*num_assignments).map(|_| assignment.clone()).collect::<Vec<_>>())];

        c.bench_function(&format!("KaryMerkleTree batch prove {num_assignments} assignments"), |b| {
            b.iter(|| {
                let _proof = ProvingKey::prove_batch("ProveKaryMerkleTree", &assignments, &mut rng).unwrap();
            })
        });
    }
}

criterion_group! {
    name = kary_merkle_tree;
    config = Criterion::default().sample_size(10);
    targets = batch_prove
}
criterion_main!(kary_merkle_tree);
