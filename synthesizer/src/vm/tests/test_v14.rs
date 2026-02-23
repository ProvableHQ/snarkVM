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

use super::*;

use crate::vm::test_helpers::*;

use circuit::{Circuit, Environment, Inject, Mode};
use console::{account::ViewKey, algorithms::U8, network::ConsensusVersion, program::Value};
use snarkvm_ledger_committee::MIN_VALIDATOR_STAKE;
use snarkvm_synthesizer_program::Program;
use snarkvm_synthesizer_snark::{ProvingKey, UniversalSRS};
use snarkvm_utilities::TestRng;

use std::sync::OnceLock;

// This test verifies that:
// - programs using syntax introduced in `V14` cannot be deployed before `V14`.
// - programs using syntax introduced in `V14` can be deployed at and after `V14`.
// - a program with an array larger than 2048 cannot be deployed after `V14`.
#[test]
fn test_deployments_for_v14_features() {
    // Define the programs.
    let programs = vec![
        // A program with an array larger than 512 elements.
        r"
program uses_large_arrays.aleo;

mapping data:
    key as [u8; 513u32].public;
    value as u32.public;

function dummy:

constructor:
    assert.eq true true;
",
        // A program that uses the `snark.verify` opcode.
        r"
program uses_snark_verify.aleo;

function dummy:
    input r0 as  [u8; 8u32].public;
    input r1 as [field; 8u32].public;
    input r2 as [u8; 8u32].public;
    async dummy r0 r1 r2 into r3;
    output r3 as uses_snark_verify.aleo/dummy.future;

finalize dummy:
    input r0 as  [u8; 8u32].public;
    input r1 as [field; 8u32].public;
    input r2 as [u8; 8u32].public;
    snark.verify r0 1u8 r1 r2 into r3;
    assert.eq r3 true;

constructor:
    assert.eq true true;
",
    ];

    // Initialize an RNG.
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at one less than the V14 height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();
    let num_programs = u32::try_from(programs.len()).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(v14_height - num_programs, rng);

    // Deploy each program and ensure it fails.
    for program in &programs {
        let program = Program::from_str(program).unwrap();
        let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
        let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
        assert_eq!(block.transactions().num_accepted(), 0);
        assert_eq!(block.transactions().num_rejected(), 0);
        assert_eq!(block.aborted_transaction_ids().len(), 1);
        vm.add_next_block(&block).unwrap();
    }

    // Verify that we are at the expected height.
    assert_eq!(vm.block_store().current_block_height(), v14_height);

    // Deploy each program and ensure it succeeds.
    for program in &programs {
        let program = Program::from_str(program).unwrap();
        let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
        let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
        assert_eq!(block.transactions().num_accepted(), 1);
        assert_eq!(block.transactions().num_rejected(), 0);
        assert_eq!(block.aborted_transaction_ids().len(), 0);
        vm.add_next_block(&block).unwrap();
    }
}

#[test]
fn test_snark_verify() {
    // Define the verification program.
    let program = Program::from_str(
        r"
program test_snark_verify.aleo;

function verify_proof:
    input r0 as [u8; 673u32].private;
    input r1 as [field; 2u32].private;
    input r2 as [u8; 957u32].private;
    async verify_proof r0 r1 r2 into r3;
    output r3 as test_snark_verify.aleo/verify_proof.future;
finalize verify_proof:
    input r0 as [u8; 673u32].public;
    input r1 as [field; 2u32].public;
    input r2 as [u8; 957u32].public;
    snark.verify r0 2u8 r1 r2 into r3;
    assert.eq r3 true;

constructor:
    assert.eq true true;
    ",
    )
    .unwrap();

    // Initialize an RNG.
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at the V14 height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(v14_height, rng);

    // Deploy the program.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    // Sample a Varuna assignment
    let assignment = snarkvm_synthesizer_snark::test_helpers::sample_assignment();

    // Varuna setup, prove, and verify.
    let srs = UniversalSRS::<CurrentNetwork>::load().unwrap();
    let (proving_key, verifying_key) = srs.to_circuit_key("test", &assignment).unwrap();
    let verifying_key_bytes = verifying_key.to_bytes_le().unwrap();

    // Construct the proof
    let varuna_version = VarunaVersion::V2;
    let proof = proving_key.prove("test", varuna_version, &assignment, &mut TestRng::default()).unwrap();
    let proof_bytes = proof.to_bytes_le().unwrap();

    let one = <Circuit as circuit::Environment>::BaseField::one();
    let zero = <Circuit as circuit::Environment>::BaseField::zero();
    let public_inputs = vec![one, one];
    let invalid_public_inputs = vec![one, zero];
    assert!(verifying_key.verify("test", varuna_version, &public_inputs, &proof));
    assert!(!verifying_key.verify("test", varuna_version, &invalid_public_inputs, &proof));

    // Construct the inputs for the execution.
    let verifying_key_input = Value::Plaintext(Plaintext::Array(
        verifying_key_bytes
            .into_iter()
            .map(|byte| Plaintext::from(Literal::<CurrentNetwork>::U8(U8::new(byte))))
            .collect(),
        OnceLock::new(),
    ));
    let verification_inputs = Value::Plaintext(Plaintext::Array(
        public_inputs
            .into_iter()
            .map(|field| Plaintext::from(Literal::<CurrentNetwork>::Field(Field::new(field))))
            .collect(),
        OnceLock::new(),
    ));
    let invalid_verification_inputs = Value::Plaintext(Plaintext::Array(
        invalid_public_inputs
            .into_iter()
            .map(|field| Plaintext::from(Literal::<CurrentNetwork>::Field(Field::new(field))))
            .collect(),
        OnceLock::new(),
    ));
    let proof_input = Value::Plaintext(Plaintext::Array(
        proof_bytes.into_iter().map(|byte| Plaintext::from(Literal::<CurrentNetwork>::U8(U8::new(byte)))).collect(),
        OnceLock::new(),
    ));

    // Execute a transaction that verifies the proof correctly.
    let valid_execution = {
        let inputs = vec![verifying_key_input.clone(), verification_inputs, proof_input.clone()];

        vm.execute(
            &caller_private_key,
            (program.id().to_string(), "verify_proof"),
            inputs.into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap()
    };
    let valid_execution_id = valid_execution.id();

    // Execute a transaction that fails to verify the proof.
    let invalid_execution = {
        let inputs = vec![verifying_key_input, invalid_verification_inputs, proof_input];

        vm.execute(
            &caller_private_key,
            (program.id().to_string(), "verify_proof"),
            inputs.into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap()
    };
    let invalid_execution_id = invalid_execution.id();

    let block = sample_next_block(&vm, &caller_private_key, &[valid_execution, invalid_execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 1);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    assert!(vm.transaction_store().contains_transaction_id(&valid_execution_id).unwrap());
    assert!(vm.block_store().contains_rejected_or_aborted_transaction_id(&invalid_execution_id).unwrap());
}

#[test]
fn test_snark_verify_batch() {
    // Define the verification program.
    let program = Program::from_str(
        r"
program test_snark_verify_batch.aleo;

function verify_batch_proof:
    input r0 as [[u8; 673u32]; 1u32].private;
    input r1 as [[[field; 2u32]; 2u32]; 1u32].private;
    input r2 as [u8; 1101u32].private;
    async verify_batch_proof r0 r1 r2 into r3;
    output r3 as test_snark_verify_batch.aleo/verify_batch_proof.future;
finalize verify_batch_proof:
    input r0 as [[u8; 673u32]; 1u32].public;
    input r1 as [[[field; 2u32]; 2u32]; 1u32].public;
    input r2 as [u8; 1101u32].public;
    snark.verify.batch r0 2u8 r1 r2 into r3;
    assert.eq r3 true;

constructor:
    assert.eq true true;
    ",
    )
    .unwrap();

    // Initialize an RNG.
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at the V14 height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(v14_height, rng);

    // Deploy the program.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    // Sample a Varuna assignment
    let assignment = snarkvm_synthesizer_snark::test_helpers::sample_assignment();

    // Varuna setup, prove, and verify.
    let srs = UniversalSRS::<CurrentNetwork>::load().unwrap();
    let (proving_key, verifying_key) = srs.to_circuit_key("test", &assignment).unwrap();
    let verifying_key_bytes = verifying_key.to_bytes_le().unwrap();

    // Construct the batch_proof
    let varuna_version = VarunaVersion::V2;
    let assignments = vec![(proving_key.clone(), vec![assignment.clone(); 2])];
    let batch_proof = ProvingKey::prove_batch("test", varuna_version, &assignments, &mut TestRng::default()).unwrap();
    let batch_proof_bytes = batch_proof.to_bytes_le().unwrap();

    println!("verifying_key_size: {}", verifying_key_bytes.len());
    println!("proof size: {}", batch_proof_bytes.len());

    let one = <Circuit as circuit::Environment>::BaseField::one();
    let zero = <Circuit as circuit::Environment>::BaseField::zero();
    let public_inputs = vec![one, one];
    let invalid_public_inputs = vec![one, zero];

    let valid_inputs = vec![(verifying_key.clone(), vec![public_inputs.clone(); 2])];
    let invalid_inputs = vec![(verifying_key.clone(), vec![invalid_public_inputs.clone(); 2])];
    assert!(VerifyingKey::verify_batch("test", varuna_version, valid_inputs, &batch_proof).is_ok());
    assert!(VerifyingKey::verify_batch("test", varuna_version, invalid_inputs, &batch_proof).is_err());

    // Construct the inputs for the execution.
    let verifying_key_input = Value::Plaintext(Plaintext::Array(
        vec![Plaintext::Array(
            verifying_key_bytes
                .into_iter()
                .map(|byte| Plaintext::from(Literal::<CurrentNetwork>::U8(U8::new(byte))))
                .collect(),
            OnceLock::new(),
        )],
        OnceLock::new(),
    ));
    let verification_inputs = Value::Plaintext(Plaintext::Array(
        vec![Plaintext::Array(
            vec![
                Plaintext::Array(
                    public_inputs
                        .clone()
                        .into_iter()
                        .map(|field| Plaintext::from(Literal::<CurrentNetwork>::Field(Field::new(field))))
                        .collect(),
                    OnceLock::new(),
                )
                .clone();
                2
            ],
            OnceLock::new(),
        )],
        OnceLock::new(),
    ));
    let invalid_verification_inputs = Value::Plaintext(Plaintext::Array(
        vec![Plaintext::Array(
            vec![
                Plaintext::Array(
                    invalid_public_inputs
                        .into_iter()
                        .map(|field| Plaintext::from(Literal::<CurrentNetwork>::Field(Field::new(field))))
                        .collect(),
                    OnceLock::new(),
                )
                .clone();
                2
            ],
            OnceLock::new(),
        )],
        OnceLock::new(),
    ));
    let proof_input = Value::Plaintext(Plaintext::Array(
        batch_proof_bytes
            .into_iter()
            .map(|byte| Plaintext::from(Literal::<CurrentNetwork>::U8(U8::new(byte))))
            .collect(),
        OnceLock::new(),
    ));

    // Execute a transaction that verifies the proof correctly.
    let valid_execution = {
        let inputs = vec![verifying_key_input.clone(), verification_inputs.clone(), proof_input.clone()];
        vm.execute(
            &caller_private_key,
            (program.id().to_string(), "verify_batch_proof"),
            inputs.into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap()
    };
    let valid_execution_id = valid_execution.id();

    // Execute a transaction that fails to verify the proof.
    let invalid_execution = {
        let inputs = vec![verifying_key_input.clone(), invalid_verification_inputs, proof_input.clone()];
        vm.execute(
            &caller_private_key,
            (program.id().to_string(), "verify_batch_proof"),
            inputs.into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap()
    };
    let invalid_execution_id = invalid_execution.id();

    let block = sample_next_block(&vm, &caller_private_key, &[valid_execution, invalid_execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 1);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    assert!(vm.transaction_store().contains_transaction_id(&valid_execution_id).unwrap());
    assert!(vm.block_store().contains_rejected_or_aborted_transaction_id(&invalid_execution_id).unwrap());
}

#[test]
fn test_snark_verify_batch_padded_inputs() {
    // Define the verification program.
    let program = Program::from_str(
        r"
program test_snark_verify_padded.aleo;

function verify_batch_proof:
    input r0 as [[u8; 673u32]; 1u32].private;
    input r1 as [[[field; 20u32]; 2u32]; 1u32].private;
    input r2 as [u8; 1101u32].private;
    async verify_batch_proof r0 r1 r2 into r3;
    output r3 as test_snark_verify_padded.aleo/verify_batch_proof.future;
finalize verify_batch_proof:
    input r0 as [[u8; 673u32]; 1u32].public;
    input r1 as [[[field; 20u32]; 2u32]; 1u32].public;
    input r2 as [u8; 1101u32].public;
    snark.verify.batch r0 2u8 r1 r2 into r3;
    assert.eq r3 true;

constructor:
    assert.eq true true;
    ",
    )
    .unwrap();

    // Initialize an RNG.
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at the V14 height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(v14_height, rng);

    // Deploy the program.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    // Sample a Varuna assignment
    let assignment = snarkvm_synthesizer_snark::test_helpers::sample_assignment();

    // Varuna setup, prove, and verify.
    let srs = UniversalSRS::<CurrentNetwork>::load().unwrap();
    let (proving_key, verifying_key) = srs.to_circuit_key("test", &assignment).unwrap();
    let verifying_key_bytes = verifying_key.to_bytes_le().unwrap();

    // Construct the batch_proof
    let varuna_version = VarunaVersion::V2;
    let assignments = vec![(proving_key.clone(), vec![assignment.clone(); 2])];
    let batch_proof = ProvingKey::prove_batch("test", varuna_version, &assignments, &mut TestRng::default()).unwrap();
    let batch_proof_bytes = batch_proof.to_bytes_le().unwrap();

    println!("verifying_key_size: {}", verifying_key_bytes.len());
    println!("proof size: {}", batch_proof_bytes.len());

    let one = <Circuit as circuit::Environment>::BaseField::one();
    let zero = <Circuit as circuit::Environment>::BaseField::zero();
    let public_inputs = vec![one, one];
    let invalid_public_inputs = vec![one, zero];

    let valid_inputs = vec![(verifying_key.clone(), vec![public_inputs.clone(); 2])];
    let invalid_inputs = vec![(verifying_key.clone(), vec![invalid_public_inputs.clone(); 2])];
    assert!(VerifyingKey::verify_batch("test", varuna_version, valid_inputs, &batch_proof).is_ok());
    assert!(VerifyingKey::verify_batch("test", varuna_version, invalid_inputs, &batch_proof).is_err());

    // Pad the public inputs to match the expected size.
    let mut padded_public_inputs = public_inputs.clone();
    padded_public_inputs.resize(20, zero);

    // Pad the public inputs to match the expected size.
    let mut padded_invalid_public_inputs = public_inputs.clone();
    padded_invalid_public_inputs.resize(20, one);

    // Construct the inputs for the execution.
    let verifying_key_input = Value::Plaintext(Plaintext::Array(
        vec![Plaintext::Array(
            verifying_key_bytes
                .into_iter()
                .map(|byte| Plaintext::from(Literal::<CurrentNetwork>::U8(U8::new(byte))))
                .collect(),
            OnceLock::new(),
        )],
        OnceLock::new(),
    ));
    let verification_inputs = Value::Plaintext(Plaintext::Array(
        vec![Plaintext::Array(
            vec![
                Plaintext::Array(
                    padded_public_inputs
                        .clone()
                        .into_iter()
                        .map(|field| Plaintext::from(Literal::<CurrentNetwork>::Field(Field::new(field))))
                        .collect(),
                    OnceLock::new(),
                )
                .clone();
                2
            ],
            OnceLock::new(),
        )],
        OnceLock::new(),
    ));
    let invalid_verification_inputs = Value::Plaintext(Plaintext::Array(
        vec![Plaintext::Array(
            vec![
                Plaintext::Array(
                    padded_invalid_public_inputs
                        .into_iter()
                        .map(|field| Plaintext::from(Literal::<CurrentNetwork>::Field(Field::new(field))))
                        .collect(),
                    OnceLock::new(),
                )
                .clone();
                2
            ],
            OnceLock::new(),
        )],
        OnceLock::new(),
    ));
    let proof_input = Value::Plaintext(Plaintext::Array(
        batch_proof_bytes
            .into_iter()
            .map(|byte| Plaintext::from(Literal::<CurrentNetwork>::U8(U8::new(byte))))
            .collect(),
        OnceLock::new(),
    ));

    // Execute a transaction that verifies the proof correctly.
    let valid_execution = {
        let inputs = vec![verifying_key_input.clone(), verification_inputs.clone(), proof_input.clone()];
        vm.execute(
            &caller_private_key,
            (program.id().to_string(), "verify_batch_proof"),
            inputs.into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap()
    };
    let valid_execution_id = valid_execution.id();

    // Execute a transaction that fails to verify the proof.
    let invalid_execution = {
        let inputs = vec![verifying_key_input.clone(), invalid_verification_inputs, proof_input.clone()];
        vm.execute(
            &caller_private_key,
            (program.id().to_string(), "verify_batch_proof"),
            inputs.into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap()
    };
    let invalid_execution_id = invalid_execution.id();

    let block = sample_next_block(&vm, &caller_private_key, &[valid_execution, invalid_execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 1);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    assert!(vm.transaction_store().contains_transaction_id(&valid_execution_id).unwrap());
    assert!(vm.block_store().contains_rejected_or_aborted_transaction_id(&invalid_execution_id).unwrap());
}

#[test]
fn test_snark_verify_batch_multiple_circuits() {
    // Define the verification program.
    let program = Program::from_str(
        r"
program test_snark_verify_batch.aleo;

function verify_batch_proof:
    input r0 as [[u8; 673u32]; 2u32].private;
    input r1 as [[[field; 2u32]; 2u32]; 2u32].private;
    input r2 as [u8; 1733u32].private;
    async verify_batch_proof r0 r1 r2 into r3;
    output r3 as test_snark_verify_batch.aleo/verify_batch_proof.future;
finalize verify_batch_proof:
    input r0 as [[u8; 673u32]; 2u32].public;
    input r1 as [[[field; 2u32]; 2u32]; 2u32].public;
    input r2 as [u8; 1733u32].public;
    snark.verify.batch r0 2u8 r1 r2 into r3;
    assert.eq r3 true;

constructor:
    assert.eq true true;
    ",
    )
    .unwrap();

    // Initialize an RNG.
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at the V14 height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(v14_height, rng);

    // Deploy the program.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    // Sample two Varuna assignments
    let assignment = snarkvm_synthesizer_snark::test_helpers::sample_assignment();
    let assignment_2 = {
        let _candidate_output = snarkvm_synthesizer_snark::test_helpers::create_example_circuit::<Circuit>();
        let _ = circuit::Field::<CurrentAleo>::new(Mode::Private, Field::from_u8(100));
        Circuit::eject_assignment_and_reset()
    };

    // Varuna setup, prove, and verify.
    let srs = UniversalSRS::<CurrentNetwork>::load().unwrap();
    let (proving_key, verifying_key) = srs.to_circuit_key("test", &assignment).unwrap();
    let (proving_key_2, verifying_key_2) = srs.to_circuit_key("test_2", &assignment_2).unwrap();
    let verifying_key_bytes = verifying_key.to_bytes_le().unwrap();
    let verifying_key_2_bytes = verifying_key_2.to_bytes_le().unwrap();

    // Construct the batch_proof
    let varuna_version = VarunaVersion::V2;
    let assignments = vec![
        (proving_key.clone(), vec![assignment.clone(); 2]),
        (proving_key_2.clone(), vec![assignment_2.clone(); 2]),
    ];
    let batch_proof = ProvingKey::prove_batch("test", varuna_version, &assignments, &mut TestRng::default()).unwrap();
    let batch_proof_bytes = batch_proof.to_bytes_le().unwrap();

    println!("verifying_key_size: {}", verifying_key_bytes.len());
    println!("proof size: {}", batch_proof_bytes.len());

    let one = <Circuit as circuit::Environment>::BaseField::one();
    let zero = <Circuit as circuit::Environment>::BaseField::zero();
    let public_inputs = vec![one, one];
    let invalid_public_inputs = vec![one, zero];

    let valid_inputs = vec![
        (verifying_key.clone(), vec![public_inputs.clone(); 2]),
        (verifying_key_2.clone(), vec![public_inputs.clone(); 2]),
    ];
    let invalid_inputs = vec![
        (verifying_key.clone(), vec![invalid_public_inputs.clone(); 2]),
        (verifying_key_2.clone(), vec![invalid_public_inputs.clone(); 2]),
    ];
    assert!(VerifyingKey::verify_batch("test", varuna_version, valid_inputs, &batch_proof).is_ok());
    assert!(VerifyingKey::verify_batch("test", varuna_version, invalid_inputs, &batch_proof).is_err());

    // Construct the inputs for the execution.
    let verifying_key_input = Value::Plaintext(Plaintext::Array(
        vec![
            Plaintext::Array(
                verifying_key_bytes
                    .into_iter()
                    .map(|byte| Plaintext::from(Literal::<CurrentNetwork>::U8(U8::new(byte))))
                    .collect(),
                OnceLock::new(),
            ),
            Plaintext::Array(
                verifying_key_2_bytes
                    .into_iter()
                    .map(|byte| Plaintext::from(Literal::<CurrentNetwork>::U8(U8::new(byte))))
                    .collect(),
                OnceLock::new(),
            ),
        ],
        OnceLock::new(),
    ));
    let verification_inputs = Value::Plaintext(Plaintext::Array(
        vec![
            Plaintext::Array(
                vec![
                    Plaintext::Array(
                        public_inputs
                            .clone()
                            .into_iter()
                            .map(|field| Plaintext::from(Literal::<CurrentNetwork>::Field(Field::new(field))))
                            .collect(),
                        OnceLock::new(),
                    )
                    .clone();
                    2
                ],
                OnceLock::new(),
            )
            .clone();
            2
        ],
        OnceLock::new(),
    ));
    let invalid_verification_inputs = Value::Plaintext(Plaintext::Array(
        vec![
            Plaintext::Array(
                vec![
                    Plaintext::Array(
                        invalid_public_inputs
                            .into_iter()
                            .map(|field| Plaintext::from(Literal::<CurrentNetwork>::Field(Field::new(field))))
                            .collect(),
                        OnceLock::new(),
                    )
                    .clone();
                    2
                ],
                OnceLock::new(),
            )
            .clone();
            2
        ],
        OnceLock::new(),
    ));
    let proof_input = Value::Plaintext(Plaintext::Array(
        batch_proof_bytes
            .into_iter()
            .map(|byte| Plaintext::from(Literal::<CurrentNetwork>::U8(U8::new(byte))))
            .collect(),
        OnceLock::new(),
    ));

    // Execute a transaction that verifies the proof correctly.
    let valid_execution = {
        let inputs = vec![verifying_key_input.clone(), verification_inputs.clone(), proof_input.clone()];
        vm.execute(
            &caller_private_key,
            (program.id().to_string(), "verify_batch_proof"),
            inputs.into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap()
    };
    let valid_execution_id = valid_execution.id();

    // Execute a transaction that fails to verify the proof.
    let invalid_execution = {
        let inputs = vec![verifying_key_input.clone(), invalid_verification_inputs, proof_input.clone()];
        vm.execute(
            &caller_private_key,
            (program.id().to_string(), "verify_batch_proof"),
            inputs.into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap()
    };
    let invalid_execution_id = invalid_execution.id();

    let block = sample_next_block(&vm, &caller_private_key, &[valid_execution, invalid_execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 1);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    assert!(vm.transaction_store().contains_transaction_id(&valid_execution_id).unwrap());
    assert!(vm.block_store().contains_rejected_or_aborted_transaction_id(&invalid_execution_id).unwrap());
}

#[test]
fn test_snark_verify_batch_via_mapping() {
    // Define the verification program.
    let program = Program::from_str(
        r"
program test_snark_verify_mapping.aleo;

mapping verifying_keys:
    key as u8.public;
    value as [[u8; 673u32]; 1u32].public;

mapping proofs:
    key as u8.public;
    value as [u8; 1101u32].public;

function store_verifying_key_and_proof:
    input r0 as [[u8; 673u32]; 1u32].private;
    input r1 as [u8; 1101u32].private;
    async store_verifying_key_and_proof r0 r1 into r2;
    output r2 as test_snark_verify_mapping.aleo/store_verifying_key_and_proof.future;

finalize store_verifying_key_and_proof:
    input r0 as [[u8; 673u32]; 1u32].public;
    input r1 as [u8; 1101u32].public;
    set r0 into verifying_keys[0u8];
    set r1 into proofs[0u8];

function verify_batch_proof:
    input r0 as [[[field; 2u32]; 2u32]; 1u32].private;
    async verify_batch_proof r0 into r1;
    output r1 as test_snark_verify_mapping.aleo/verify_batch_proof.future;
finalize verify_batch_proof:
    input r0 as [[[field; 2u32]; 2u32]; 1u32].public;
    get verifying_keys[0u8] into r1;
    get proofs[0u8] into r2;
    snark.verify.batch r1 2u8 r0 r2 into r3;
    assert.eq r3 true;

constructor:
    assert.eq true true;
    ",
    )
    .unwrap();

    // Initialize an RNG.
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at the V14 height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(v14_height, rng);

    // Deploy the program.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    // Sample a Varuna assignment
    let assignment = snarkvm_synthesizer_snark::test_helpers::sample_assignment();

    // Varuna setup, prove, and verify.
    let srs = UniversalSRS::<CurrentNetwork>::load().unwrap();
    let (proving_key, verifying_key) = srs.to_circuit_key("test", &assignment).unwrap();
    let verifying_key_bytes = verifying_key.to_bytes_le().unwrap();

    // Construct the batch_proof
    let varuna_version = VarunaVersion::V2;
    let assignments = vec![(proving_key.clone(), vec![assignment.clone(); 2])];
    let batch_proof = ProvingKey::prove_batch("test", varuna_version, &assignments, &mut TestRng::default()).unwrap();
    let batch_proof_bytes = batch_proof.to_bytes_le().unwrap();

    let one = <Circuit as circuit::Environment>::BaseField::one();
    let zero = <Circuit as circuit::Environment>::BaseField::zero();
    let public_inputs = vec![one, one];
    let invalid_public_inputs = vec![one, zero];

    let valid_inputs = vec![(verifying_key.clone(), vec![public_inputs.clone(); 2])];
    let invalid_inputs = vec![(verifying_key.clone(), vec![invalid_public_inputs.clone(); 2])];
    assert!(VerifyingKey::verify_batch("test", varuna_version, valid_inputs, &batch_proof).is_ok());
    assert!(VerifyingKey::verify_batch("test", varuna_version, invalid_inputs, &batch_proof).is_err());

    // Construct the inputs for the execution.
    let verifying_key_input = Value::Plaintext(Plaintext::Array(
        vec![Plaintext::Array(
            verifying_key_bytes
                .into_iter()
                .map(|byte| Plaintext::from(Literal::<CurrentNetwork>::U8(U8::new(byte))))
                .collect(),
            OnceLock::new(),
        )],
        OnceLock::new(),
    ));
    let verification_inputs = Value::Plaintext(Plaintext::Array(
        vec![Plaintext::Array(
            vec![
                Plaintext::Array(
                    public_inputs
                        .clone()
                        .into_iter()
                        .map(|field| Plaintext::from(Literal::<CurrentNetwork>::Field(Field::new(field))))
                        .collect(),
                    OnceLock::new(),
                )
                .clone();
                2
            ],
            OnceLock::new(),
        )],
        OnceLock::new(),
    ));
    let invalid_verification_inputs = Value::Plaintext(Plaintext::Array(
        vec![Plaintext::Array(
            vec![
                Plaintext::Array(
                    invalid_public_inputs
                        .into_iter()
                        .map(|field| Plaintext::from(Literal::<CurrentNetwork>::Field(Field::new(field))))
                        .collect(),
                    OnceLock::new(),
                )
                .clone();
                2
            ],
            OnceLock::new(),
        )],
        OnceLock::new(),
    ));
    let proof_input = Value::Plaintext(Plaintext::Array(
        batch_proof_bytes
            .into_iter()
            .map(|byte| Plaintext::from(Literal::<CurrentNetwork>::U8(U8::new(byte))))
            .collect(),
        OnceLock::new(),
    ));

    // Store the verifying key and proof via the mapping.
    let store_vk_and_proof = {
        let inputs = vec![verifying_key_input.clone(), proof_input.clone()];
        vm.execute(
            &caller_private_key,
            (program.id().to_string(), "store_verifying_key_and_proof"),
            inputs.into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap()
    };
    let block = sample_next_block(&vm, &caller_private_key, &[store_vk_and_proof], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block).unwrap();

    // Execute a transaction that verifies the proof correctly.
    let valid_execution = {
        let inputs = vec![verification_inputs.clone()];
        vm.execute(
            &caller_private_key,
            (program.id().to_string(), "verify_batch_proof"),
            inputs.into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap()
    };
    let valid_execution_id = valid_execution.id();

    // Execute a transaction that fails to verify the proof.
    let invalid_execution = {
        let inputs = vec![invalid_verification_inputs];
        vm.execute(
            &caller_private_key,
            (program.id().to_string(), "verify_batch_proof"),
            inputs.into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap()
    };
    let invalid_execution_id = invalid_execution.id();

    let block = sample_next_block(&vm, &caller_private_key, &[valid_execution, invalid_execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 1);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    assert!(vm.transaction_store().contains_transaction_id(&valid_execution_id).unwrap());
    assert!(vm.block_store().contains_rejected_or_aborted_transaction_id(&invalid_execution_id).unwrap());
}

#[test]
fn test_increased_argument_bitsize() {
    // Define the programs.
    let program = Program::from_str(
        r"
program test_large_argument.aleo;

function large_argument_input:
    input r0 as [[u8; 512u32]; 3u32].private;
    async large_argument_input r0 into r1;
    output r1 as test_large_argument.aleo/large_argument_input.future;

finalize large_argument_input:
    input r0 as [[u8; 512u32]; 3u32].public;
    assert.eq true true;

constructor:
    assert.eq true true;
    ",
    )
    .unwrap();

    // Initialize an RNG.
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at one less than the V13 height.
    let v13_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V13).unwrap();
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(v13_height, rng);

    // Create the deployment.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    assert!(vm.check_transaction(&deployment, None, rng).is_err());

    // Advance the VM to the V14 height.
    while vm.block_store().current_block_height() < v14_height {
        let block = sample_next_block(&vm, &caller_private_key, &[], rng).unwrap();
        vm.add_next_block(&block).unwrap();
    }

    // Verify that we are at the expected height.
    assert_eq!(vm.block_store().current_block_height(), v14_height);

    // Ensure that the deployment is now valid.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    assert!(vm.check_transaction(&deployment, None, rng).is_ok());

    // Add the deployment block.
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);

    vm.add_next_block(&block).unwrap();

    // Construct the inputs for the execution.
    let input = Value::Plaintext(Plaintext::Array(
        vec![
            Plaintext::Array(
                vec![0u8; 512]
                    .into_iter()
                    .map(|byte| Plaintext::from(Literal::<CurrentNetwork>::U8(U8::new(byte))))
                    .collect(),
                OnceLock::new(),
            )
            .clone();
            3
        ],
        OnceLock::new(),
    ));

    // Execute a transaction that verifies.
    let execution = vm
        .execute(
            &caller_private_key,
            (program.id().to_string(), "large_argument_input"),
            [input].into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    assert!(vm.check_transaction(&execution, None, rng).is_ok());

    // Add the execution block.
    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);

    vm.add_next_block(&block).unwrap();
}
/// Generates a large program string that exceeds the V13 size limit (100KB) but fits within V14 (512KB).
fn generate_large_program() -> String {
    let mut program = String::from(
        "program large_program_generated.aleo;

constructor:
    assert.eq true true;

function compute:
    input r0 as u64.public;
",
    );

    // Generate cast instructions to create large arrays.
    // Each cast with 32 elements is ~200+ bytes, so we need fewer instructions.
    let mut reg = 1u32;
    while program.len() < 110_000 {
        // Create a 32-element array from r0.
        program.push_str(&format!(
            "    cast r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 into r{reg} as [u64; 32u32];\n"
        ));
        reg += 1;
    }

    program
}

// This test verifies that a large program that is over the previous size limit can be deployed after V14.
#[test]
fn test_deploy_large_program_v14() {
    // Initialize an RNG.
    let rng = &mut TestRng::default();

    let large_program_str = generate_large_program();
    let large_program = Program::from_str(&large_program_str).unwrap();

    println!("Generated large program size: {} bytes", large_program_str.len());

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at the V13 height.
    let v13_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V13).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(v13_height, rng);

    // Ensure that the program is too large to be deployed at V13.
    let deployment = vm.deploy(&caller_private_key, &large_program, None, 0, None, rng).unwrap();
    let deployment_id = deployment.id();
    assert!(vm.check_transaction(&deployment, None, rng).is_err());
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids(), &[deployment_id]);
    assert_eq!(block.aborted_transaction_ids().len(), 1);

    // Advance to the V14 height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();
    while vm.block_store().current_block_height() < v14_height {
        let block = sample_next_block(&vm, &caller_private_key, &[], rng).unwrap();
        vm.add_next_block(&block).unwrap();
    }

    // Ensure that the program can be deployed at V14.
    let deployment = vm.deploy(&caller_private_key, &large_program, None, 0, None, rng).unwrap();
    assert!(vm.check_transaction(&deployment, None, rng).is_ok());

    // Add the deployment block.
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
}

// This test verifies serialization round-trips for large program deployment transactions at V13 and V14.
#[test]
fn test_deploy_large_program_v14_serialization() {
    // Initialize an RNG.
    let rng = &mut TestRng::default();

    let large_program_str = generate_large_program();
    let large_program = Program::from_str(&large_program_str).unwrap();

    println!("Generated large program size: {} bytes", large_program_str.len());

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at the V13 height.
    let v13_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V13).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(v13_height, rng);

    // Create a deployment transaction for the large program at V13.
    let deployment = vm.deploy(&caller_private_key, &large_program, None, 0, None, rng).unwrap();
    let deployment_id = deployment.id();

    // Verify bytes serialization round-trip at V13.
    let deployment_bytes = deployment.to_bytes_le().unwrap();
    let recovered_from_bytes = Transaction::<CurrentNetwork>::read_le(&deployment_bytes[..]).unwrap();
    assert_eq!(deployment, recovered_from_bytes);

    // Verify string (JSON) serialization round-trip at V13.
    let deployment_string = deployment.to_string();
    let recovered_from_string = Transaction::<CurrentNetwork>::from_str(&deployment_string).unwrap();
    assert_eq!(deployment, recovered_from_string);

    // Ensure that the program is too large to pass check_transaction at V13.
    assert!(vm.check_transaction(&deployment, None, rng).is_err());

    // Create block and verify the transaction is aborted.
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids(), &[deployment_id]);
    assert_eq!(block.aborted_transaction_ids().len(), 1);

    // Advance to the V14 height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();
    while vm.block_store().current_block_height() < v14_height {
        let block = sample_next_block(&vm, &caller_private_key, &[], rng).unwrap();
        vm.add_next_block(&block).unwrap();
    }

    // Create a new deployment transaction for the large program at V14.
    let deployment = vm.deploy(&caller_private_key, &large_program, None, 0, None, rng).unwrap();

    // Verify bytes serialization round-trip at V14.
    let deployment_bytes = deployment.to_bytes_le().unwrap();
    let recovered_from_bytes = Transaction::<CurrentNetwork>::read_le(&deployment_bytes[..]).unwrap();
    assert_eq!(deployment, recovered_from_bytes);

    // Verify string (JSON) serialization round-trip at V14.
    let deployment_string = deployment.to_string();
    let recovered_from_string = Transaction::<CurrentNetwork>::from_str(&deployment_string).unwrap();
    assert_eq!(deployment, recovered_from_string);

    // Ensure that the program passes check_transaction at V14.
    assert!(vm.check_transaction(&deployment, None, rng).is_ok());

    // Create block and verify the transaction is accepted.
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);

    // Add block to the VM.
    vm.add_next_block(&block).unwrap();
}

#[cfg(feature = "test")]
#[test]
fn test_aleo_generators_migration() {
    let rng = &mut TestRng::default();

    // Initialize the VM.
    let vm = sample_vm();
    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);
    // Update the VM.
    vm.add_next_block(&genesis).unwrap();

    // Fetch the private key.
    let private_key = sample_genesis_private_key(rng);
    let view_key = ViewKey::try_from(&private_key).unwrap();
    let address = Address::try_from(&view_key).unwrap();

    // Deploy a test program to the ledger.
    let program_id = ProgramID::<CurrentNetwork>::from_str("dummy_program.aleo").unwrap();
    let program = Program::<CurrentNetwork>::from_str(&format!(
        r"
    program {program_id};
    function foo:
        input r0 as scalar.public;
        input r1 as address.public;
        mul aleo::GENERATOR r0 into r2;
        mul aleo::GENERATOR_POWERS[0u32] r0 into r3;
        assert.eq aleo::GENERATOR aleo::GENERATOR_POWERS[0u32];
        assert.eq r2 r3;
        cast r2 into r4 as address;
        assert.eq r1 r4;
        async foo r0 r1 into r5;
        output r5 as {program_id}/foo.future;
    finalize foo:
        input r0 as scalar.public;
        input r1 as address.public;
        mul aleo::GENERATOR r0 into r2;
        mul aleo::GENERATOR_POWERS[0u32] r0 into r3;
        assert.eq r2 r3;
        assert.eq aleo::GENERATOR aleo::GENERATOR_POWERS[0u32];
        cast r2 into r4 as address;
        assert.eq r1 r4;

    function foo_2:
        input r0 as scalar.public;
        input r1 as address.public;
        async foo_2 r0 r1 into r2;
        output r2 as {program_id}/foo_2.future;
    finalize foo_2:
        input r0 as scalar.public;
        input r1 as address.public;
        mul aleo::GENERATOR r0 into r2;
        cast r2 into r3 as address;
        assert.eq r1 r3;

    function will_fail:
        input r0 as scalar.public;
        async will_fail r0 into r1;
        output r1 as {program_id}/will_fail.future;

    finalize will_fail:
        input r0 as scalar.public;
        mul aleo::GENERATOR_POWERS[256u32] r0 into r1;

    constructor:
        assert.eq edition 0u16;",
    ))
    .unwrap();

    // Advance the ledger past ConsensusV9 where the new varuna version and deployment version starts to take place.
    let transactions: [Transaction<CurrentNetwork>; 0] = [];
    while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V9).unwrap() {
        // Call the function
        let next_block = sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
        vm.add_next_block(&next_block).unwrap();
    }

    // Construct the deployment transaction.
    let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

    // Advance the ledger past ConsensusV14 where the new generator opcodes are enabled.
    let transactions: [Transaction<CurrentNetwork>; 0] = [];
    while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap() {
        // Ensure that the deployment is invalid.
        assert!(vm.check_transaction(&deployment, None, rng).is_err());

        // Call the function
        let next_block = sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
        vm.add_next_block(&next_block).unwrap();
    }

    // Ensure that the deployment is valid after ConsensusVersion::V14.
    assert!(vm.check_transaction(&deployment, None, rng).is_ok());

    // Deploy the program.
    let next_block = sample_next_block(&vm, &private_key, &[deployment], rng).unwrap();
    vm.add_next_block(&next_block).unwrap();

    // Construct the input with the valid view key derivation.
    let inputs = [
        Value::<CurrentNetwork>::Plaintext(Plaintext::from(Literal::Scalar(*view_key))),
        Value::<CurrentNetwork>::Plaintext(Plaintext::from(Literal::Address(address))),
    ];
    // Create the execution transaction.
    let valid_transaction =
        vm.execute(&private_key, (&program_id.to_string(), "foo"), inputs.into_iter(), None, 0, None, rng).unwrap();
    let valid_tx_id = valid_transaction.id();

    // Create the execution transaction that will fail to execute.
    let new_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let new_address = Address::try_from(&new_private_key).unwrap();
    let inputs = [
        Value::<CurrentNetwork>::Plaintext(Plaintext::from(Literal::Scalar(*view_key))),
        Value::<CurrentNetwork>::Plaintext(Plaintext::from(Literal::Address(new_address))),
    ];
    assert!(
        vm.execute(&private_key, (&program_id.to_string(), "foo"), inputs.into_iter(), None, 0, None, rng).is_err()
    );

    // Create the execution transaction that will fail in finalize.
    let inputs = [
        Value::<CurrentNetwork>::Plaintext(Plaintext::from(Literal::Scalar(*view_key))),
        Value::<CurrentNetwork>::Plaintext(Plaintext::from(Literal::Address(new_address))),
    ];
    let invalid_transaction =
        vm.execute(&private_key, (&program_id.to_string(), "foo_2"), inputs.into_iter(), None, 0, None, rng).unwrap();
    let invalid_tx_id = invalid_transaction.id();

    // Construct another invalid transaction that will fail in finalize.
    let inputs = [Value::<CurrentNetwork>::Plaintext(Plaintext::from(Literal::Scalar(*view_key)))];
    let invalid_transaction_2 = vm
        .execute(&private_key, (&program_id.to_string(), "will_fail"), inputs.into_iter(), None, 0, None, rng)
        .unwrap();
    let invalid_tx_2_id = invalid_transaction_2.id();

    // Construct a block with both transactions.
    let next_block =
        sample_next_block(&vm, &private_key, &[valid_transaction, invalid_transaction, invalid_transaction_2], rng)
            .unwrap();
    vm.add_next_block(&next_block).unwrap();

    // Ensure that the valid transaction was accepted and the invalid ones were rejected.
    assert_eq!(next_block.transactions().num_accepted(), 1);
    assert_eq!(next_block.transactions().num_rejected(), 2);
    assert!(vm.block_store().get_confirmed_transaction(&valid_tx_id).unwrap().unwrap().is_accepted());
    assert!(vm.block_store().get_confirmed_transaction(&invalid_tx_id).unwrap().unwrap().is_rejected());
    assert!(vm.block_store().get_confirmed_transaction(&invalid_tx_2_id).unwrap().unwrap().is_rejected());
}

#[cfg(feature = "test")]
#[test]
fn test_max_writes_migration() {
    let rng = &mut TestRng::default();

    // Initialize the VM.
    let vm = sample_vm();
    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);
    // Update the VM.
    vm.add_next_block(&genesis).unwrap();

    // Fetch the private key.
    let private_key = sample_genesis_private_key(rng);

    // Create a program that hits the max writes limit.
    let mut program_string = String::from(
        "program test_max_writes.aleo;

mapping foo:
    key as u16.public;
    value as field.public;

constructor:
    assert.eq true true;

function compute:
    input r0 as u8.public;
    async compute r0 into r1;
    output r1 as test_max_writes.aleo/compute.future;

finalize compute:
    input r0 as u8.public;
",
    );

    // Create a program that exceeds the max writes limit.
    let mut invalid_program_string = String::from(
        "program test_max_writes.aleo;

mapping foo:
    key as u16.public;
    value as field.public;

constructor:
    assert.eq true true;

function compute:
    input r0 as u8.public;
    async compute r0 into r1;
    output r1 as test_max_writes.aleo/compute.future;

finalize compute:
    input r0 as u8.public;
    set 0field into foo[0u16];
",
    );

    for i in 0..CurrentNetwork::LATEST_MAX_WRITES() {
        program_string.push_str(&format!("    set 0field into foo[{i}u16];\n"));
        invalid_program_string.push_str(&format!("    set 0field into foo[{i}u16];\n"));
    }

    let program = Program::<CurrentNetwork>::from_str(&program_string).unwrap();

    // Ensure that the program that exceeds max writes fails to parse.
    assert!(Program::<CurrentNetwork>::from_str(&invalid_program_string).is_err());

    // Advance the ledger past ConsensusV9 where the new varuna version and deployment version starts to take place.
    let transactions: [Transaction<CurrentNetwork>; 0] = [];
    while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V9).unwrap() {
        // Call the function
        let next_block = sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
        vm.add_next_block(&next_block).unwrap();
    }

    // Construct the deployment transaction.
    let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

    // Advance the ledger past ConsensusV14 where the increase to max writes starts.
    let transactions: [Transaction<CurrentNetwork>; 0] = [];
    while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap() {
        // Ensure that the deployment is invalid.
        assert!(vm.check_transaction(&deployment, None, rng).is_err());

        let next_block = sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
        vm.add_next_block(&next_block).unwrap();
    }

    // Ensure that the deployment is valid after ConsensusVersion::V14.
    assert!(vm.check_transaction(&deployment, None, rng).is_ok());

    // Deploy the program.
    let next_block = sample_next_block(&vm, &private_key, &[deployment], rng).unwrap();
    vm.add_next_block(&next_block).unwrap();

    // Ensure that the valid transaction was accepted.
    assert_eq!(next_block.transactions().num_accepted(), 1);

    // Create the execution transaction that hits the max writes limit.
    let inputs = [Value::<CurrentNetwork>::Plaintext(Plaintext::from(Literal::U8(U8::new(1u8))))];
    let transaction =
        vm.execute(&private_key, (program.id(), "compute"), inputs.into_iter(), None, 0, None, rng).unwrap();
    let next_block = sample_next_block(&vm, &private_key, &[transaction], rng).unwrap();
    vm.add_next_block(&next_block).unwrap();

    // Ensure that the valid transaction was accepted.
    assert_eq!(next_block.transactions().num_accepted(), 1);
}

#[test]
fn test_max_writes_exceeds_finalize_amount() {
    const NUM_DEPLOYMENTS: usize = 31;

    let rng = &mut TestRng::default();

    // Initialize the VM.
    let vm = sample_vm();
    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);
    // Update the VM.
    vm.add_next_block(&genesis).unwrap();

    // Fetch the private key.
    let private_key = sample_genesis_private_key(rng);

    // Advance the ledger past ConsensusV14 where the increase to max writes starts.
    let transactions: [Transaction<CurrentNetwork>; 0] = [];
    while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap() {
        let next_block = sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
        vm.add_next_block(&next_block).unwrap();
    }

    // Deploy the base program.
    let program = Program::from_str(
        r"
program program_layer_0.aleo;

constructor:
    assert.eq true true;

mapping m:
    key as u8.public;
    value as u32.public;

function do:
    input r0 as u32.public;
    async do r0 into r1;
    output r1 as program_layer_0.aleo/do.future;

finalize do:
    input r0 as u32.public;
    set r0 into m[0u8];",
    )
    .unwrap();

    let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
    vm.check_transaction(&deployment, None, rng).unwrap();
    let next_block = sample_next_block(&vm, &private_key, &[deployment], rng).unwrap();
    vm.add_next_block(&next_block).unwrap();
    assert_eq!(next_block.transactions().num_accepted(), 1);

    // For each layer, deploy a program that calls the program from the previous layer.
    for i in 1..NUM_DEPLOYMENTS {
        let mut program_string = String::new();
        // Add the import statements.
        for j in 0..i {
            program_string.push_str(&format!("import program_layer_{j}.aleo;\n"));
        }
        // Add the program body.
        program_string.push_str(&format!(
            "program program_layer_{i}.aleo;

constructor:
    assert.eq true true;

mapping m:
    key as u8.public;
    value as u32.public;

function do:
    input r0 as u32.public;
    call program_layer_{prev}.aleo/do r0 into r1;
    async do r0 r1 into r2;
    output r2 as program_layer_{i}.aleo/do.future;

finalize do:
    input r0 as u32.public;
    input r1 as program_layer_{prev}.aleo/do.future;
    await r1;",
            prev = i - 1
        ));

        for k in 0..CurrentNetwork::LATEST_MAX_WRITES() {
            program_string.push_str(&format!("set r0 into m[{k}u8];\n"));
        }
        // Construct the program.
        let program = Program::from_str(&program_string).unwrap();

        // Deploy the program.
        let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

        // Create block with deployment.
        let next_block = sample_next_block(&vm, &private_key, &[deployment], rng).unwrap();

        // Add block to the VM.
        vm.add_next_block(&next_block).unwrap();
        assert_eq!(next_block.transactions().num_accepted(), 1);
    }

    // Prepare the inputs.
    let inputs = [Value::<CurrentNetwork>::from_str("1u32").unwrap()].into_iter();

    // Execute.
    let transaction = vm.execute(&private_key, ("program_layer_30.aleo", "do"), inputs, None, 0, None, rng).unwrap();

    // Verify.
    assert!(vm.check_transaction(&transaction, None, rng).is_err());
}

#[test]
fn test_rejected_reason_deployments() {
    let rng = &mut TestRng::default();

    // Initialize the VM.
    let vm = sample_vm();
    let genesis = sample_genesis_block(rng);
    vm.add_next_block(&genesis).unwrap();

    // Fetch the genesis private key.
    let private_key = sample_genesis_private_key(rng);

    // Determine the V14 activation height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();

    // Fund two accounts to pay for the two upgrades.
    // This has to be done because only one deployment can be made per fee-paying address per block.
    let private_key_2 = PrivateKey::new(rng).unwrap();
    let private_key_3 = PrivateKey::new(rng).unwrap();
    let address_2 = Address::try_from(&private_key_2).unwrap();
    let address_3 = Address::try_from(&private_key_3).unwrap();

    let tx_1 = vm
        .execute(
            &private_key,
            ("credits.aleo", "transfer_public"),
            [Value::from_str(&format!("{address_2}")).unwrap(), Value::from_str("100_000_000u64").unwrap()].iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let tx_2 = vm
        .execute(
            &private_key,
            ("credits.aleo", "transfer_public"),
            [Value::from_str(&format!("{address_3}")).unwrap(), Value::from_str("100_000_000u64").unwrap()].iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();

    let block = sample_next_block(&vm, &private_key, &[tx_1, tx_2], rng).unwrap();
    vm.add_next_block(&block).unwrap();

    // Define a program whose constructor always fails.
    let failing_constructor = Program::<CurrentNetwork>::from_str(
        r"
program failing_constructor.aleo;

function dummy:

constructor:
    assert.eq true false;",
    )
    .unwrap();

    // Define a program whose constructor always succeeds (used for the duplicate-ID tests).
    let duplicate_id_program = Program::<CurrentNetwork>::from_str(
        r"
program duplicate_id_program.aleo;

function dummy:

constructor:
    assert.eq true true;",
    )
    .unwrap();

    // Advance to two slots before V14, leaving room for one pre-V14 test block.
    while vm.block_store().current_block_height() < v14_height - 2 {
        let block = sample_next_block(&vm, &private_key, &[], rng).unwrap();
        vm.add_next_block(&block).unwrap();
    }

    // Place two transactions for `duplicate_id_program.aleo` and one for `failing_constructor.aleo`
    // in the same block.  Before V14:
    //   • The first `duplicate_id_program` deploy is accepted.
    //   • The second `duplicate_id_program` deploy is rejected (duplicate program ID) with no rejected reason.
    //   • The `failing_constructor` deploy is rejected (constructor fails) with no rejected reason.
    let deploy_dup_1 = vm.deploy(&private_key, &duplicate_id_program, None, 0, None, rng).unwrap();
    let deploy_dup_2 = vm.deploy(&private_key_2, &duplicate_id_program, None, 0, None, rng).unwrap();
    let deploy_failing_constructor = vm.deploy(&private_key_3, &failing_constructor, None, 0, None, rng).unwrap();

    let block =
        sample_next_block(&vm, &private_key, &[deploy_dup_1, deploy_dup_2, deploy_failing_constructor], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 2);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    // Before V14, neither rejected transaction carries a rejected reason.
    for confirmed_tx in block.transactions().iter().filter(|tx| tx.is_rejected()) {
        assert!(confirmed_tx.rejected_reason().is_none());
    }
    vm.add_next_block(&block).unwrap();

    // Advance to V14.
    while vm.block_store().current_block_height() < v14_height {
        let block = sample_next_block(&vm, &private_key, &[], rng).unwrap();
        vm.add_next_block(&block).unwrap();
    }

    // Deploy two transactions for a fresh program `dup_program_2.aleo`.  At V14:
    //   • The first deploy is accepted.
    //   • The second deploy is rejected with `RejectedReason::DuplicateProgramID`.
    let duplicate_id_program_2 = Program::<CurrentNetwork>::from_str(
        r"
program duplicate_id_program_2.aleo;

function dummy:

constructor:
    assert.eq true true;",
    )
    .unwrap();
    let deploy_dup2_1 = vm.deploy(&private_key, &duplicate_id_program_2, None, 0, None, rng).unwrap();
    let deploy_dup2_2 = vm.deploy(&private_key_2, &duplicate_id_program_2, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &private_key, &[deploy_dup2_1, deploy_dup2_2], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 1);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    // The rejected transaction must carry a `DuplicateProgramID` reason.
    let rejected_tx = block.transactions().iter().find(|tx| tx.is_rejected()).unwrap();
    assert_eq!(
        rejected_tx.rejected_reason(),
        Some(RejectedReason::DuplicateProgramID("duplicate_id_program_2.aleo".to_string()))
    );
    vm.add_next_block(&block).unwrap();

    // Re-deploy `failing_constructor.aleo`
    // At V14 the deployment is again rejected, but now carries a `Finalize` reason.
    let deploy_failing_constructor_2 = vm.deploy(&private_key, &failing_constructor, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &private_key, &[deploy_failing_constructor_2], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0);
    assert_eq!(block.transactions().num_rejected(), 1);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    let rejected_tx = block.transactions().iter().find(|tx| tx.is_rejected()).unwrap();
    // The locator points to the constructor of `failing_constructor.aleo`.
    let Some(RejectedReason::Finalize(locator, index, command)) = rejected_tx.rejected_reason() else {
        panic!("Expected RejectedReason::Finalize, got {:?}", rejected_tx.rejected_reason());
    };
    assert_eq!(locator, "failing_constructor.aleo/constructor");
    assert_eq!(index, 0);
    assert_eq!(command, "assert.eq true false;");
}

#[test]
fn test_rejected_reason_finalize() {
    let rng = &mut TestRng::default();

    // Initialize the VM.
    let vm = sample_vm();
    let genesis = sample_genesis_block(rng);
    vm.add_next_block(&genesis).unwrap();

    // Fetch the genesis private key.
    let private_key = sample_genesis_private_key(rng);

    // Determine the V14 activation height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();

    // Define a program whose `fail` function always fails in finalize.
    let fail_finalize_program = Program::<CurrentNetwork>::from_str(
        r"
program failing_program.aleo;

function fail:
    input r0 as boolean.public;
    async fail r0 into r1;
    output r1 as failing_program.aleo/fail.future;

finalize fail:
    input r0 as boolean.public;
    assert.eq true true;
    assert.eq false r0;

constructor:
    assert.eq true true;",
    )
    .unwrap();

    // Define a program whose `fail` function always fails in finalize.
    let call_fail_finalize_program = Program::<CurrentNetwork>::from_str(
        r"
import failing_program.aleo;

program call_failing_program.aleo;

function call_fail:
    input r0 as boolean.public;
    call failing_program.aleo/fail r0 into r1;
    async call_fail r1 into r2;
    output r2 as call_failing_program.aleo/call_fail.future;

finalize call_fail:
    input r0 as failing_program.aleo/fail.future;
    await r0;

constructor:
    assert.eq true true;",
    )
    .unwrap();

    // Advance to four blocks before V14, leaving room for one pre-V14 execution block.
    while vm.block_store().current_block_height() < v14_height - 4 {
        let block = sample_next_block(&vm, &private_key, &[], rng).unwrap();
        vm.add_next_block(&block).unwrap();
    }

    // Deploy `failing_program.aleo` early so it is available for execution.
    let deployment = vm.deploy(&private_key, &fail_finalize_program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block).unwrap();

    // Deploy `call_failing_program.aleo` early so it is available for execution.
    let deployment = vm.deploy(&private_key, &call_fail_finalize_program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block).unwrap();

    // Execute `failing_program.aleo/fail_here` and `call_failing_program/call_fail_here`.
    // The transaction is rejected in finalize, but before V14 no rejected reason is attached.
    let inputs = [Value::from_str("true").unwrap()].into_iter();
    let fail_transaction =
        vm.execute(&private_key, ("failing_program.aleo", "fail"), inputs.clone(), None, 0, None, rng).unwrap();
    let call_fail_transaction = vm
        .execute(&private_key, ("call_failing_program.aleo", "call_fail"), inputs.clone(), None, 0, None, rng)
        .unwrap();
    let block = sample_next_block(&vm, &private_key, &[fail_transaction, call_fail_transaction], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0);
    assert_eq!(block.transactions().num_rejected(), 2);
    assert_eq!(block.aborted_transaction_ids().len(), 0);

    // Before V14, neither rejected transaction carries a rejected reason.
    for confirmed_tx in block.transactions().iter().filter(|tx| tx.is_rejected()) {
        assert!(confirmed_tx.rejected_reason().is_none());
    }
    vm.add_next_block(&block).unwrap();

    // Advance to V14, leaving room for one pre-V14 execution block.
    while vm.block_store().current_block_height() < v14_height {
        let block = sample_next_block(&vm, &private_key, &[], rng).unwrap();
        vm.add_next_block(&block).unwrap();
    }

    // Execute `fail_finalize.aleo/fail` again.  Now at V14 the rejection carries a proper rejected reason.
    let inputs = [Value::from_str("true").unwrap()].into_iter();
    let fail_transaction =
        vm.execute(&private_key, ("failing_program.aleo", "fail"), inputs.clone(), None, 0, None, rng).unwrap();
    let call_fail_transaction = vm
        .execute(&private_key, ("call_failing_program.aleo", "call_fail"), inputs.clone(), None, 0, None, rng)
        .unwrap();
    let block = sample_next_block(&vm, &private_key, &[fail_transaction, call_fail_transaction], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0);
    assert_eq!(block.transactions().num_rejected(), 2);
    assert_eq!(block.aborted_transaction_ids().len(), 0);

    let transactions = block.transactions().iter().collect::<Vec<_>>();

    // Ensure the first call fails and has the proper reason.
    let first_confirmed_transaction = transactions[0];
    let Some(RejectedReason::Finalize(locator, index, command)) = first_confirmed_transaction.rejected_reason() else {
        panic!("Expected RejectedReason::Finalize, got {:?}", first_confirmed_transaction.rejected_reason());
    };
    assert_eq!(locator, "failing_program.aleo/fail");
    assert_eq!(index, 1);
    assert_eq!(command, "assert.eq false r0;");

    // Ensure the second call fails and has the proper reason.
    let second_confirmed_transaction = transactions[1];
    let Some(RejectedReason::Finalize(locator, index, command)) = second_confirmed_transaction.rejected_reason() else {
        panic!("Expected RejectedReason::Finalize, got {:?}", second_confirmed_transaction.rejected_reason());
    };
    assert_eq!(locator, "failing_program.aleo/fail");
    assert_eq!(index, 1);
    assert_eq!(command, "assert.eq false r0;");
}

#[test]
fn test_rejected_reason_non_finalize() {
    // Initialize an RNG.
    let rng = &mut TestRng::default();

    // Determine the V14 activation height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();

    // Initialize the VM.
    let vm = sample_vm();

    // Construct the validators, one less than the maximum committee size.
    let max_committee_size = consensus_config_value!(CurrentNetwork, MAX_CERTIFICATES, 0).unwrap();
    let validators = (0..max_committee_size - 1)
        .map(|_| {
            let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
            let amount = MIN_VALIDATOR_STAKE;
            let is_open = true;
            (private_key, (amount, is_open))
        })
        .collect::<IndexMap<_, _>>();

    // Track the allocated amount.
    let mut allocated_amount = 0;

    // Construct the committee.
    let mut committee_map = IndexMap::new();
    for (private_key, (amount, _)) in &validators {
        let address = Address::try_from(private_key).unwrap();
        committee_map.insert(address, (*amount, true, 0));
        allocated_amount += *amount;
    }

    // Initialize new validator keys.
    let mut new_validators = vec![];
    let v14_max_committee_size = consensus_config_value!(CurrentNetwork, MAX_CERTIFICATES, v14_height).unwrap();
    let num_new_validators = v14_max_committee_size as usize - validators.len();
    for _ in 0..=num_new_validators {
        let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
        let address = Address::try_from(&private_key).unwrap();
        let withdrawal_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
        let withdrawal_address = Address::try_from(&withdrawal_private_key).unwrap();

        new_validators.push((private_key, address, withdrawal_private_key, withdrawal_address));
    }

    // Construct the public balances, allocating the remaining supply to the first validator and the new validators.
    // The remaining validators will have a balance of 0.
    let mut remaining_supply = <CurrentNetwork as Network>::STARTING_SUPPLY - allocated_amount;
    let public_balance = remaining_supply / (new_validators.len() + 1) as u64;
    let mut public_balances = IndexMap::new();
    for (private_key, _) in validators.iter() {
        public_balances.insert(Address::try_from(private_key).unwrap(), 0);
    }
    for (_, address, _, _) in &new_validators {
        public_balances.insert(*address, public_balance);
        remaining_supply -= public_balance;
    }
    public_balances.insert(Address::try_from(validators.first().unwrap().0).unwrap(), remaining_supply);

    // Construct the bonded balances.
    let bonded_balances = validators
        .iter()
        .map(|(private_key, (amount, _))| {
            let address = Address::try_from(private_key).unwrap();
            (address, (address, address, *amount))
        })
        .collect();

    // Fetch the first private_key
    let private_key = validators.keys().next().unwrap();

    // Construct the genesis block, which should pass.
    let genesis_block = vm
        .genesis_quorum(
            private_key,
            Committee::new_genesis(committee_map).unwrap(),
            public_balances,
            bonded_balances,
            rng,
        )
        .unwrap();

    // Initialize a Ledger from the genesis block.
    vm.add_next_block(&genesis_block).unwrap();

    // Advance a few blocks before V14.
    while vm.block_store().current_block_height() < v14_height - 3 {
        let block = sample_next_block(&vm, private_key, &[], rng).unwrap();
        vm.add_next_block(&block).unwrap();
    }

    // Bond validators until the maximum.
    let current_max_committee_size =
        consensus_config_value!(CurrentNetwork, MAX_CERTIFICATES, vm.block_store().current_block_height()).unwrap();
    let mut new_bond_transactions = vec![];
    let num_new_validators_to_bond = current_max_committee_size as usize - validators.len();
    assert!(num_new_validators_to_bond <= new_validators.len(), "Not enough new validators to bond");
    for (new_validator_private_key, _, _, withdrawal_address) in new_validators.iter().take(num_new_validators_to_bond)
    {
        let bond_transaction = vm
            .execute(
                new_validator_private_key,
                ("credits.aleo", "bond_validator"),
                vec![
                    Value::<CurrentNetwork>::from_str(&withdrawal_address.to_string()).unwrap(),
                    Value::<CurrentNetwork>::from_str(&format!("{MIN_VALIDATOR_STAKE}u64")).unwrap(),
                    Value::<CurrentNetwork>::from_str("10u8").unwrap(),
                ]
                .iter(),
                None,
                0,
                None,
                rng,
            )
            .unwrap();

        new_bond_transactions.push(bond_transaction);
    }
    let block = sample_next_block(&vm, private_key, &new_bond_transactions, rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), new_bond_transactions.len());
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    // Attempt to bond a new validator past the maximum and ensure that the rejected reason does not exist.
    let bond_transaction = vm
        .execute(
            &new_validators.last().unwrap().0,
            ("credits.aleo", "bond_validator"),
            vec![
                Value::<CurrentNetwork>::from_str(&new_validators.last().unwrap().3.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{MIN_VALIDATOR_STAKE}u64")).unwrap(),
                Value::<CurrentNetwork>::from_str("10u8").unwrap(),
            ]
            .iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, private_key, &[bond_transaction.clone()], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0);
    assert_eq!(block.transactions().num_rejected(), 1);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    // Before V14, neither rejected transaction carries a rejected reason.
    for confirmed_tx in block.transactions().iter().filter(|tx| tx.is_rejected()) {
        assert!(confirmed_tx.rejected_reason().is_none());
    }

    // Advance to the V14 height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();
    while vm.block_store().current_block_height() < v14_height {
        let block = sample_next_block(&vm, private_key, &[], rng).unwrap();
        vm.add_next_block(&block).unwrap();
    }

    // After V14 the bond is again rejected, but now carries a `NonFinalize` reason.
    let block = sample_next_block(&vm, private_key, &[bond_transaction], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0);
    assert_eq!(block.transactions().num_rejected(), 1);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    let rejected_tx = block.transactions().iter().find(|tx| tx.is_rejected()).unwrap();
    // The locator points to the constructor of `failing_constructor.aleo`.
    let Some(RejectedReason::NonFinalize(locator)) = rejected_tx.rejected_reason() else {
        panic!("Expected RejectedReason::NonFinalize, got {:?}", rejected_tx.rejected_reason());
    };
    assert_eq!(locator, "credits.aleo/bond_validator");
    vm.add_next_block(&block).unwrap();
}
