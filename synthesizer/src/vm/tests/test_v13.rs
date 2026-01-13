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

use crate::vm::test_helpers::*;

use snarkvm_synthesizer_program::Program;

use console::network::ConsensusVersion;
use snarkvm_utilities::TestRng;

// This test verifies that a program with external structs cannot be deployed on
// consensus version 12.
#[test]
fn test_deploy_external_structs_v11() {
    // Use V11 rather than V12 to make sure we still won't be on V13
    // when deploying the second program.
    let block = deploy_external_structs_programs(ConsensusVersion::V11);

    assert_eq!(block.transactions().num_accepted(), 0);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 1);
}

// This test verifies that a program with external structs can be deployed on
// consensus version 13.
#[test]
fn test_deploy_external_structs_v13() {
    let block = deploy_external_structs_programs(ConsensusVersion::V13);

    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
}

fn deploy_external_structs_programs(consensus_version: ConsensusVersion) -> Block<CurrentNetwork> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at the correct height.
    let height = CurrentNetwork::CONSENSUS_HEIGHT(consensus_version).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(height, rng);

    // Define the first program with a record.
    let program_one = Program::from_str(
        r"
program test_one.aleo;

constructor:
    assert.eq true true;

struct S:
    x as field;

function make_s:
    cast 0field into r0 as S;
    output r0 as S.public;
",
    )
    .unwrap();

    // Define the second program which refers to the external struct type.
    let program_two = Program::from_str(
        r"
import test_one.aleo;

program test_two.aleo;

constructor:
    assert.eq true true;

function second:
    call test_one.aleo/make_s into r0;
    output r0 as test_one.aleo/S.public;
",
    )
    .unwrap();

    // Deploy the first program.
    let deployment_one = vm.deploy(&caller_private_key, &program_one, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment_one], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    // Deploy the second program.
    let deployment_two = vm.deploy(&caller_private_key, &program_two, None, 0, None, rng).unwrap();
    sample_next_block(&vm, &caller_private_key, &[deployment_two], rng).unwrap()
}

// This test verifies that a program with a mapping containing a missing struct can be deployed on
// consensus version 12.
#[test]
fn test_deploy_mapping_with_missing_struct_programs_v12() {
    let block = deploy_mapping_with_missing_struct_program(ConsensusVersion::V12);

    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
}

// This test verifies that a program with a mapping containing a missing struct cannot be deployed on
// consensus version 13.
#[test]
fn test_deploy_mapping_with_missing_struct_v13() {
    let block = deploy_mapping_with_missing_struct_program(ConsensusVersion::V13);

    assert_eq!(block.transactions().num_accepted(), 0);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 1);
}

fn deploy_mapping_with_missing_struct_program(consensus_version: ConsensusVersion) -> Block<CurrentNetwork> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at the correct height.
    let height = CurrentNetwork::CONSENSUS_HEIGHT(consensus_version).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(height, rng);

    // Define the first program with a record.
    let program_one = Program::from_str(
        r"
program child.aleo;

mapping foo:
    key as field.public;
    value as S.public;

function dummy:

constructor:
    assert.eq edition 0u16;
",
    )
    .unwrap();

    // Deploy the program.
    let deployment = vm.deploy(&caller_private_key, &program_one, None, 0, None, rng).unwrap();
    sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap()
}

// This test verifies that a program with a mapping containing a missing struct cannot be deployed on
// consensus version 13.
#[test]
fn test_deploy_mapping_with_missing_external_struct_v13() {
    let block = deploy_mapping_with_missing_external_struct_programs(ConsensusVersion::V13);

    assert_eq!(block.transactions().num_accepted(), 0);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 1);
}

fn deploy_mapping_with_missing_external_struct_programs(consensus_version: ConsensusVersion) -> Block<CurrentNetwork> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at the correct height.
    let height = CurrentNetwork::CONSENSUS_HEIGHT(consensus_version).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(height, rng);

    // Define the first program with a record.
    let program_one = Program::from_str(
        r"
program child.aleo;

function dummy:

constructor:
    assert.eq edition 0u16;
",
    )
    .unwrap();

    // Define the second program which refers to the external struct type.
    let program_two = Program::from_str(
        r"
import child.aleo;

program parent.aleo;

mapping foo:
    key as field.public;
    value as child.aleo/S.public;

function dummy:

constructor:
    assert.eq edition 0u16;
",
    )
    .unwrap();

    // Deploy the first program.
    let deployment_one = vm.deploy(&caller_private_key, &program_one, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment_one], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    // Deploy the second program.
    let deployment_two = vm.deploy(&caller_private_key, &program_two, None, 0, None, rng).unwrap();
    sample_next_block(&vm, &caller_private_key, &[deployment_two], rng).unwrap()
}
