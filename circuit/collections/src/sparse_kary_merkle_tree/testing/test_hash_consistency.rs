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

use super::super::*;
use crate::sparse_kary_merkle_tree::helpers::{KeyHash as CircuitKeyHash, LeafHash as CircuitLeafHash, PathHash as CircuitPathHash};
use snarkvm_circuit_algorithms::Poseidon;
use snarkvm_circuit_types::environment::Circuit;
use snarkvm_utilities::{TestRng, Uniform};

const DOMAIN: &str = "HashConsistencyTest";

#[test]
fn test_key_hash_trait_consistency() {
    // This test verifies that the KeyHash TRAIT implementation matches console behavior
    type NativePoseidon = console::algorithms::Poseidon<<Circuit as Environment>::Network, 2>;
    type CircuitPoseidon = Poseidon<Circuit, 2>;
    
    let mut rng = TestRng::default();
    
    let native_hasher = NativePoseidon::setup(DOMAIN).unwrap();
    let circuit_hasher = CircuitPoseidon::constant(native_hasher.clone());
    
    for _ in 0..5 {
        let key = console::Field::<<Circuit as Environment>::Network>::rand(&mut rng);
        
        // Console: Use the KeyHash trait
        let console_result = console::sparse_kary_merkle_tree::KeyHash::hash_key(
            &native_hasher,
            &key
        ).unwrap();
        
        // Circuit: Use the KeyHash trait (tests our trait implementation!)
        Circuit::scope("KeyHash trait", || {
            let circuit_key = Field::new(Mode::Private, key);
            let circuit_result = CircuitKeyHash::hash_key(&circuit_hasher, &circuit_key);
            
            // CRITICAL: If this fails, the circuit KeyHash trait is implemented incorrectly!
            assert_eq!(console_result, circuit_result.eject_value(),
                "Circuit KeyHash trait produces different output than console! \
                 This means the circuit helper implementation is WRONG!");
        });
        Circuit::reset();
    }
}

#[test]
fn test_leaf_hash_trait_consistency() {
    type NativePoseidon = console::algorithms::Poseidon<<Circuit as Environment>::Network, 4>;
    type CircuitPoseidon = Poseidon<Circuit, 4>;
    
    let mut rng = TestRng::default();
    
    let native_hasher = NativePoseidon::setup(DOMAIN).unwrap();
    let circuit_hasher = CircuitPoseidon::constant(native_hasher.clone());
    
    for _ in 0..5 {
        let leaf = vec![console::Field::<<Circuit as Environment>::Network>::rand(&mut rng)];
        
        // Console: Use the LeafHash trait
        let console_result = console::sparse_kary_merkle_tree::LeafHash::hash_leaf(
            &native_hasher,
            &leaf
        ).unwrap();
        
        // Circuit: Use the LeafHash trait
        Circuit::scope("LeafHash trait", || {
            let circuit_leaf: Vec<_> = Inject::new(Mode::Private, leaf.clone());
            let circuit_result = CircuitLeafHash::hash_leaf(&circuit_hasher, &circuit_leaf);
            
            assert_eq!(console_result, circuit_result.eject_value(),
                "Circuit LeafHash trait produces different output! Implementation is WRONG!");
        });
        Circuit::reset();
    }
}

#[test]
fn test_path_hash_trait_consistency() {
    type NativePoseidon = console::algorithms::Poseidon<<Circuit as Environment>::Network, 2>;
    type CircuitPoseidon = Poseidon<Circuit, 2>;
    
    let mut rng = TestRng::default();
    
    let native_hasher = NativePoseidon::setup(DOMAIN).unwrap();
    let circuit_hasher = CircuitPoseidon::constant(native_hasher.clone());
    
    const ARITY: usize = 4;
    
    for _ in 0..5 {
        let children: Vec<_> = (0..ARITY)
            .map(|_| console::Field::<<Circuit as Environment>::Network>::rand(&mut rng))
            .collect();
        
        // Console: Use the PathHash trait
        let console_result = console::sparse_kary_merkle_tree::PathHash::hash_children(
            &native_hasher,
            &children
        ).unwrap();
        
        // Circuit: Use the PathHash trait
        Circuit::scope("PathHash trait", || {
            let circuit_children: Vec<_> = children.iter()
                .map(|&c| Field::new(Mode::Private, c))
                .collect();
            let circuit_result = CircuitPathHash::hash_children(&circuit_hasher, &circuit_children);
            
            assert_eq!(console_result, circuit_result.eject_value(),
                "Circuit PathHash trait produces different output! Implementation is WRONG!");
        });
        Circuit::reset();
    }
}

#[test]
fn test_end_to_end_proof_verification() {
    // End-to-end test: Console generates proof, circuit verifies it.
    // Both use the same hasher setup to ensure compatibility.
    
    type NativePoseidon2 = console::algorithms::Poseidon<<Circuit as Environment>::Network, 2>;
    type NativePoseidon4 = console::algorithms::Poseidon<<Circuit as Environment>::Network, 4>;
    type CircuitPoseidon2 = Poseidon<Circuit, 2>;
    type CircuitPoseidon4 = Poseidon<Circuit, 4>;
    
    let mut rng = TestRng::default();
    
    let native_key_hasher = NativePoseidon2::setup("E2ETest0").unwrap();
    let native_leaf_hasher = NativePoseidon4::setup("E2ETest1").unwrap();
    let native_path_hasher = NativePoseidon2::setup("E2ETest2").unwrap();
    
    let circuit_key_hasher = CircuitPoseidon2::constant(native_key_hasher.clone());
    let circuit_leaf_hasher = CircuitPoseidon4::constant(native_leaf_hasher.clone());
    let circuit_path_hasher = CircuitPoseidon2::constant(native_path_hasher.clone());
    
    let mut tree = console::sparse_kary_merkle_tree::SparseKaryMerkleTree::<
        _,
        _,
        _,
        <Circuit as Environment>::Network,
        16,
        4,
    >::new(&native_key_hasher, &native_leaf_hasher, &native_path_hasher).unwrap();
    
    let key = Uniform::rand(&mut rng);
    let leaf = vec![Uniform::rand(&mut rng)];
    
    tree.update(&key, &leaf).unwrap();
    let console_proof = tree.prove(&key, &leaf).unwrap();
    
    assert!(console_proof.verify(
        &native_key_hasher,
        &native_leaf_hasher,
        &native_path_hasher,
        tree.root(),
        &key,
        &leaf
    ));
    
    Circuit::scope("Circuit verifies console proof", || {
        let circuit_path = SparseKaryMerklePath::<Circuit, CircuitPoseidon2, 16, 4>::new(
            Mode::Private,
            console_proof,
        );
        let circuit_root = Field::new(Mode::Public, *tree.root());
        let circuit_key = Field::new(Mode::Private, key);
        let circuit_leaf: Vec<_> = Inject::new(Mode::Private, leaf);
        
        let result = circuit_path.verify(
            &circuit_key_hasher,
            &circuit_leaf_hasher,
            &circuit_path_hasher,
            &circuit_root,
            &circuit_key,
            &circuit_leaf,
        );
        
        assert!(result.eject_value(),
            "Circuit must verify console-generated proof!");
    });
}

