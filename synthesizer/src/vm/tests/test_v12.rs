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
// consensus version 9.
#[test]
fn test_deploy_external_structs_v10() {
    // Use V10 rather than V11 to make sure we still won't be on V12
    // when deploying the second program.
    let block = deploy_programs(ConsensusVersion::V10);

    assert_eq!(block.transactions().num_accepted(), 0);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 1);
}

// This test verifies that a program with external structs can be deployed on
// consensus version 12.
#[test]
fn test_deploy_external_structs_v12() {
    let block = deploy_programs(ConsensusVersion::V12);

    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
}

fn deploy_programs(consensus_version: ConsensusVersion) -> Block<CurrentNetwork> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

    // Initialize the VM at the correct height.
    let v10_height = CurrentNetwork::CONSENSUS_HEIGHT(consensus_version).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(v10_height, rng);

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
