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

// This test verifies that path traversal through external structs works correctly when the
// external struct contains a member that is a LOCAL struct reference (not an ExternalStruct).
#[test]
fn test_external_struct_with_local_nested_struct() {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at V13 height.
    let height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V13).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(height, rng);

    // Define the first program with nested structs where Inner is a LOCAL reference in Outer.
    // This is the key distinction: Outer.inner is declared as `inner as Inner` (local), not
    // `inner as parent.aleo/Inner` (external).
    let program_parent = Program::from_str(
        r"
program parent.aleo;

struct Inner:
    x as field;

struct Outer:
    inner as Inner;

function make_outer:
    cast 42field into r0 as Inner;
    cast r0 into r1 as Outer;
    output r1 as Outer.public;

constructor:
    assert.eq edition 0u16;
",
    )
    .unwrap();

    // Define the child program that accesses nested path r0.inner.x on an external struct.
    let program_child = Program::from_str(
        r"
import parent.aleo;

program child.aleo;

function access_nested:
    input r0 as parent.aleo/Outer.private;
    assert.eq r0.inner.x 42field;

constructor:
    assert.eq edition 0u16;
",
    )
    .unwrap();

    // Deploy the parent program.
    let deployment_parent = vm.deploy(&caller_private_key, &program_parent, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment_parent], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block).unwrap();

    // Deploy the child program.
    let deployment_child = vm.deploy(&caller_private_key, &program_child, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment_child], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Child program deployment should succeed");
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
}

// This test verifies path traversal through external records when the record entry is a
// local struct in the external program.
#[test]
fn test_external_record_with_local_struct_entry() {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at V13 height.
    let height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V13).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(height, rng);

    // Define the parent program with a record containing a local struct.
    let program_parent = Program::from_str(
        r"
program parent.aleo;

struct Data:
    amount as field;

record Token:
    owner as address.private;
    data as Data.private;

function mint:
    input r0 as address.private;
    cast 100field into r1 as Data;
    cast r0 r1 into r2 as Token.record;
    output r2 as Token.record;

constructor:
    assert.eq edition 0u16;
",
    )
    .unwrap();

    // Define the child program that accesses the external record's local struct field.
    let program_child = Program::from_str(
        r"
import parent.aleo;

program child.aleo;

function check_token:
    input r0 as parent.aleo/Token.record;
    assert.eq r0.data.amount 100field;

constructor:
    assert.eq edition 0u16;
",
    )
    .unwrap();

    // Deploy the parent program.
    let deployment_parent = vm.deploy(&caller_private_key, &program_parent, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment_parent], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block).unwrap();

    // Deploy the child program.
    let deployment_child = vm.deploy(&caller_private_key, &program_child, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment_child], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Child program deployment should succeed");
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
}

// This test verifies path traversal through external structs containing arrays of local structs.
#[test]
fn test_external_struct_with_array_of_local_structs() {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at V13 height.
    let height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V13).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(height, rng);

    // Define the parent program with a struct containing an array of local structs.
    let program_parent = Program::from_str(
        r"
program parent.aleo;

struct Item:
    x as field;

struct Container:
    items as [Item; 2u32];

function make_container:
    cast 1field into r0 as Item;
    cast 2field into r1 as Item;
    cast r0 r1 into r2 as [Item; 2u32];
    cast r2 into r3 as Container;
    output r3 as Container.public;

constructor:
    assert.eq edition 0u16;
",
    )
    .unwrap();

    // Define the child program that accesses the array element's field.
    let program_child = Program::from_str(
        r"
import parent.aleo;

program child.aleo;

function access_array_element:
    input r0 as parent.aleo/Container.private;
    assert.eq r0.items[0u32].x 1field;
    assert.eq r0.items[1u32].x 2field;

constructor:
    assert.eq edition 0u16;
",
    )
    .unwrap();

    // Deploy the parent program.
    let deployment_parent = vm.deploy(&caller_private_key, &program_parent, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment_parent], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block).unwrap();

    // Deploy the child program.
    let deployment_child = vm.deploy(&caller_private_key, &program_child, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment_child], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Child program deployment should succeed");
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
}

// This test verifies that future validation works correctly when an external function's finalize
// block takes a local struct as a parameter.
#[test]
fn test_external_future_with_local_struct_finalize_param() {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at V13 height.
    let height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V13).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(height, rng);

    // Define the parent program with a function that has a finalize block taking a local struct.
    let program_parent = Program::from_str(
        r"
program parent.aleo;

struct Data:
    amount as field;

mapping store:
    key as field.public;
    value as field.public;

function save:
    input r0 as Data.public;
    async save r0 into r1;
    output r1 as parent.aleo/save.future;

finalize save:
    input r0 as Data.public;
    set r0.amount into store[0field];

constructor:
    assert.eq edition 0u16;
",
    )
    .unwrap();

    // Define the child program that calls the parent function.
    let program_child = Program::from_str(
        r"
import parent.aleo;

program child.aleo;

function call_save:
    cast 42field into r0 as parent.aleo/Data;
    call parent.aleo/save r0 into r1;
    async call_save r1 into r2;
    output r2 as child.aleo/call_save.future;

finalize call_save:
    input r0 as parent.aleo/save.future;
    await r0;

constructor:
    assert.eq edition 0u16;
",
    )
    .unwrap();

    // Deploy the parent program.
    let deployment_parent = vm.deploy(&caller_private_key, &program_parent, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment_parent], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block).unwrap();

    // Deploy the child program.
    let deployment_child = vm.deploy(&caller_private_key, &program_child, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment_child], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Child program deployment should succeed");
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    // Execute the child function to verify runtime validation also works.
    use console::program::Value;
    let execution = vm
        .execute(
            &caller_private_key,
            ("child.aleo", "call_save"),
            Vec::<Value<_>>::new().into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Execution should succeed");
    assert_eq!(block.aborted_transaction_ids().len(), 0);
}
