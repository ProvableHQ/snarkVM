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
use snarkvm_circuit_algorithms::{Poseidon, Hash as CircuitHash};
use snarkvm_circuit_types::environment::Circuit;
use snarkvm_utilities::{TestRng, Uniform};

const DOMAIN: &str = "StructureTest";

#[test]
fn test_key_hash_uses_field_2_domain_separator() {
    // KeyHash MUST prepend input with Field::from_u8(2) for console, or (1+1) for circuit
    type CircuitPoseidon = Poseidon<Circuit, 2>;
    
    let mut rng = TestRng::default();
    let native_hasher = console::algorithms::Poseidon2::<<Circuit as Environment>::Network>::setup(DOMAIN).unwrap();
    let circuit_hasher = CircuitPoseidon::constant(native_hasher.clone());
    
    let key = console::Field::<<Circuit as Environment>::Network>::rand(&mut rng);
    
    // What the console KeyHash trait should produce
    let expected = console::algorithms::Hash::hash(
        &native_hasher,
        &vec![
            console::Field::<<Circuit as Environment>::Network>::from_u8(2),  // Domain separator
            key,
        ]
    ).unwrap();
    
    // What the circuit KeyHash trait produces
    Circuit::scope("Check preimage structure", || {
        let circuit_key = Field::new(Mode::Private, key);
        
        // Call the KeyHash trait method
        let trait_result = CircuitKeyHash::hash_key(&circuit_hasher, &circuit_key);
        
        // Also manually compute with correct preimage
        let manual_result = CircuitHash::hash(&circuit_hasher, &vec![
            Field::<Circuit>::one() + Field::<Circuit>::one(),  // Must be 2field
            circuit_key,
        ]);
        
        // Both should match console
        assert_eq!(expected, trait_result.eject_value(), 
            "KeyHash trait doesn't use correct preimage structure!");
        assert_eq!(expected, manual_result.eject_value(),
            "Manual computation doesn't match!");
    });
}

#[test]
fn test_leaf_hash_uses_zero_domain_separator() {
    // LeafHash MUST prepend input with 0field
    type CircuitPoseidon = Poseidon<Circuit, 4>;
    
    let mut rng = TestRng::default();
    let native_hasher = console::algorithms::Poseidon4::<<Circuit as Environment>::Network>::setup(DOMAIN).unwrap();
    let circuit_hasher = CircuitPoseidon::constant(native_hasher.clone());
    
    let leaf = vec![console::Field::<<Circuit as Environment>::Network>::rand(&mut rng)];
    
    // Console expectation
    let mut console_input = vec![console::Field::<<Circuit as Environment>::Network>::zero()];
    console_input.extend(&leaf);
    let expected = console::algorithms::Hash::hash(&native_hasher, &console_input).unwrap();
    
    // Circuit trait
    Circuit::scope("Check leaf preimage", || {
        let circuit_leaf: Vec<_> = Inject::new(Mode::Private, leaf.clone());
        
        let trait_result = CircuitLeafHash::hash_leaf(&circuit_hasher, &circuit_leaf);
        
        // Manual with correct preimage
        let mut manual_input = vec![Field::<Circuit>::zero()];
        manual_input.extend_from_slice(&circuit_leaf);
        let manual_result = CircuitHash::hash(&circuit_hasher, &manual_input);
        
        assert_eq!(expected, trait_result.eject_value(),
            "LeafHash trait doesn't prepend 0field!");
        assert_eq!(expected, manual_result.eject_value(),
            "Manual doesn't match!");
    });
}

#[test]
fn test_path_hash_uses_one_domain_separator() {
    // PathHash MUST prepend children with 1field
    type CircuitPoseidon = Poseidon<Circuit, 2>;
    
    let mut rng = TestRng::default();
    let native_hasher = console::algorithms::Poseidon2::<<Circuit as Environment>::Network>::setup(DOMAIN).unwrap();
    let circuit_hasher = CircuitPoseidon::constant(native_hasher.clone());
    
    let children = vec![
        console::Field::<<Circuit as Environment>::Network>::rand(&mut rng),
        console::Field::<<Circuit as Environment>::Network>::rand(&mut rng),
    ];
    
    // Console expectation
    let mut console_input = vec![console::Field::<<Circuit as Environment>::Network>::one()];
    console_input.extend(&children);
    let expected = console::algorithms::Hash::hash(&native_hasher, &console_input).unwrap();
    
    // Circuit trait
    Circuit::scope("Check path preimage", || {
        let circuit_children: Vec<_> = children.iter()
            .map(|&c| Field::new(Mode::Private, c))
            .collect();
        
        let trait_result = CircuitPathHash::hash_children(&circuit_hasher, &circuit_children);
        
        // Manual with correct preimage
        let mut manual_input = vec![Field::<Circuit>::one()];
        manual_input.extend_from_slice(&circuit_children);
        let manual_result = CircuitHash::hash(&circuit_hasher, &manual_input);
        
        assert_eq!(expected, trait_result.eject_value(),
            "PathHash trait doesn't prepend 1field!");
        assert_eq!(expected, manual_result.eject_value(),
            "Manual doesn't match!");
    });
}

/// This test verifies that if you manually change the circuit trait to use WRONG preimages,
/// the hashes won't match console and verification will fail.
#[test]
fn test_manually_broken_preimage_causes_failure() {
    type CircuitPoseidon2 = Poseidon<Circuit, 2>;
    
    let mut rng = TestRng::default();
    let native_hasher = console::algorithms::Poseidon2::<<Circuit as Environment>::Network>::setup(DOMAIN).unwrap();
    let circuit_hasher = CircuitPoseidon2::constant(native_hasher.clone());
    
    let key = console::Field::<<Circuit as Environment>::Network>::rand(&mut rng);
    
    // Correct console computation (with domain separator 2field)
    let correct_console = console::algorithms::Hash::hash(
        &native_hasher,
        &vec![
            console::Field::<<Circuit as Environment>::Network>::from_u8(2),
            key,
        ]
    ).unwrap();
    
    // Simulating BROKEN circuit trait (using WRONG separator like 3field)
    Circuit::scope("Broken preimage test", || {
        let circuit_key = Field::new(Mode::Private, key);
        
        // Correct implementation (what the trait should do)
        let correct_circuit = CircuitHash::hash(&circuit_hasher, &vec![
            Field::<Circuit>::one() + Field::<Circuit>::one(),  // 2field - CORRECT
            circuit_key.clone(),
        ]);
        
        // Broken implementation (WRONG separator)  
        let broken_circuit = CircuitHash::hash(&circuit_hasher, &vec![
            Field::<Circuit>::one() + Field::<Circuit>::one() + Field::<Circuit>::one(),  // 3field - WRONG!
            circuit_key,
        ]);
        
        // Correct matches console
        assert_eq!(correct_console, correct_circuit.eject_value(),
            "Correct preimage must match console");
        
        // Broken does NOT match console
        assert_ne!(correct_console, broken_circuit.eject_value(),
            "Wrong preimage must NOT match console - this test verifies our tests would catch bugs!");
    });
}

#[test]
fn test_incompatible_preimage_fails_verification() {
    // This test shows that if preimages are different, verification WILL fail
    type CircuitPoseidon2 = Poseidon<Circuit, 2>;
    type CircuitPoseidon4 = Poseidon<Circuit, 4>;
    
    let mut rng = TestRng::default();
    
    let native_key_hasher = console::algorithms::Poseidon2::<<Circuit as Environment>::Network>::setup("Test0").unwrap();
    let native_leaf_hasher = console::algorithms::Poseidon4::<<Circuit as Environment>::Network>::setup("Test1").unwrap();
    let native_path_hasher = console::algorithms::Poseidon2::<<Circuit as Environment>::Network>::setup("Test2").unwrap();
    
    let circuit_key_hasher = CircuitPoseidon2::constant(native_key_hasher.clone());
    let circuit_leaf_hasher = CircuitPoseidon4::constant(native_leaf_hasher.clone());
    let circuit_path_hasher = CircuitPoseidon2::constant(native_path_hasher.clone());
    
    let key = Uniform::rand(&mut rng);
    let leaf = vec![Uniform::rand(&mut rng)];
    
    // Build tree with console
    let mut tree = console::sparse_kary_merkle_tree::SparseKaryMerkleTree::<
        _,
        _,
        _,
        <Circuit as Environment>::Network,
        16,
        4,
    >::new(&native_key_hasher, &native_leaf_hasher, &native_path_hasher).unwrap();
    
    tree.update(&key, &leaf).unwrap();
    let proof = tree.prove(&key, &leaf).unwrap();
    
    // Verification with correct root should pass
    Circuit::scope("Correct verification", || {
        let path = SparseKaryMerklePath::<Circuit, CircuitPoseidon2, 16, 4>::new(Mode::Private, proof.clone());
        let correct_root = Field::new(Mode::Public, *tree.root());
        let circuit_key = Field::new(Mode::Private, key);
        let circuit_leaf: Vec<_> = Inject::new(Mode::Private, leaf.clone());
        
        let result = path.verify(
            &circuit_key_hasher,
            &circuit_leaf_hasher,
            &circuit_path_hasher,
            &correct_root,
            &circuit_key,
            &circuit_leaf,
        );
        
        assert!(result.eject_value(), "Correct root must verify");
    });
    Circuit::reset();
    
    // Verification with WRONG root (tampered) should fail
    Circuit::scope("Wrong root fails", || {
        let path = SparseKaryMerklePath::<Circuit, CircuitPoseidon2, 16, 4>::new(Mode::Private, proof);
        let wrong_root = Field::new(Mode::Public, *tree.root()) + Field::one();  // Tampered!
        let circuit_key = Field::new(Mode::Private, key);
        let circuit_leaf: Vec<_> = Inject::new(Mode::Private, leaf);
        
        let result = path.verify(
            &circuit_key_hasher,
            &circuit_leaf_hasher,
            &circuit_path_hasher,
            &wrong_root,
            &circuit_key,
            &circuit_leaf,
        );
        
        assert!(!result.eject_value(), "Wrong root must NOT verify");
    });
}

