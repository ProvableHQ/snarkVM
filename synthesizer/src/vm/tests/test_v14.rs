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

use console::{account::ViewKey, network::ConsensusVersion, program::Value};
use snarkvm_synthesizer_program::Program;
use snarkvm_utilities::TestRng;

// This test verifies that a large program that is over the previous size limit can be deployed after V14.
#[test]
fn test_deploy_large_program_v14() {
    // Initialize an RNG.
    let rng = &mut TestRng::default();

    let large_program = Program::from_str(include_str!("./resources/large_program.aleo")).unwrap();

    println!("Large program size (string size): {}", large_program.to_string().len());

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
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
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

    // Advance the ledger past ConsensusV14 where the new varuna version starts to take place.
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

    // Ensure that the valid transaction was accepted and the invalid one was rejected.
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
    input r0 as u64.public;
    async compute r0 into r1;
    output r1 as test_max_writes.aleo/compute.future;

finalize compute:
    input r0 as u64.public;
",
    );

    // Generate cast instructions to create large arrays.
    // Each cast with 32 elements is ~200+ bytes, so we need fewer instructions.
    for i in 0..CurrentNetwork::LATEST_MAX_WRITES() {
        program_string.push_str(&format!("    set 0field into foo[{i}u16];\n"));
    }

    let program = Program::<CurrentNetwork>::from_str(&program_string).unwrap();

    // Advance the ledger past ConsensusV9 where the new varuna version and deployment version starts to take place.
    let transactions: [Transaction<CurrentNetwork>; 0] = [];
    while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V9).unwrap() {
        // Call the function
        let next_block = sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
        vm.add_next_block(&next_block).unwrap();
    }

    // Construct the deployment transaction.
    let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

    // Advance the ledger past ConsensusV14 where the new varuna version starts to take place.
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

    // Ensure that the valid transaction was accepted and the invalid one was rejected.
    assert_eq!(next_block.transactions().num_accepted(), 1);
}
