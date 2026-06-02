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

// A program that reads a contiguous sub-array via a range access `r0[1..4]`, and asserts it
// equals the expected slice passed in as a separate input.
const SLICE_PROGRAM: &str = r"
program slice_test.aleo;

function run:
    input r0 as [u8; 5u32].private;
    input r1 as [u8; 3u32].private;
    assert.eq r0[1u32..4u32] r1;
    output r1 as [u8; 3u32].private;

constructor:
    assert.eq true true;
";

// Tests that deploying a program using a range access is aborted before `ConsensusVersion::V16`
// and accepted at `V16`.
#[test]
fn test_deploy_slice_before_and_at_v16() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // Start one block before V16 so that after adding the (rejected) block we are exactly at V16.
    let v16_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V16).unwrap();
    let vm = sample_vm_at_height(v16_height - 1, rng);

    let program = Program::from_str(SLICE_PROGRAM).unwrap();

    // Deployment before V16 should be aborted.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    // Verify the rejection is specifically due to the V16 gate, not an unrelated check.
    let error = vm.check_transaction(&deployment, None, rng).unwrap_err().to_string();
    assert!(error.contains("ConsensusVersion::V16"), "Expected a V16 gate error, but got: {error}");
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0, "Deployment before V16 should not be accepted");
    assert_eq!(block.aborted_transaction_ids().len(), 1, "Deployment before V16 should be aborted");
    vm.add_next_block(&block).unwrap();

    // We should now be at V16.
    assert_eq!(vm.block_store().current_block_height(), v16_height);

    // Deployment at V16 should succeed.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Deployment at V16 should be accepted");
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
}

// Tests that a range access produces the correct contiguous sub-array by asserting it in-program.
#[test]
fn test_slice_execution() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // Initialize the VM at V16 so that range accesses can be deployed.
    let v16_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V16).unwrap();
    let vm = sample_vm_at_height(v16_height, rng);

    // Deploy the slice test program.
    let program = Program::from_str(SLICE_PROGRAM).unwrap();
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Program deployment should succeed at V16");
    vm.add_next_block(&block).unwrap();

    // Execute `run([0,1,2,3,4], [1,2,3])`. The in-program `assert.eq r0[1..4] r1` passes iff the
    // slice is correct.
    let execution = vm
        .execute(
            &caller_private_key,
            ("slice_test.aleo", "run"),
            [
                Value::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8, 3u8, 4u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[1u8, 2u8, 3u8]").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();

    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Slice assertion should pass for the correct sub-array");
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
}

// Tests that a range access with an incorrect expected slice fails the in-program assertion.
#[test]
fn test_slice_execution_wrong_slice_rejected() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    let v16_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V16).unwrap();
    let vm = sample_vm_at_height(v16_height, rng);

    let program = Program::from_str(SLICE_PROGRAM).unwrap();
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block).unwrap();

    // Execute with a wrong expected slice `[0,1,2]` (should be `[1,2,3]`), so the assertion fails.
    let result = vm.execute(
        &caller_private_key,
        ("slice_test.aleo", "run"),
        [
            Value::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8, 3u8, 4u8]").unwrap(),
            Value::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8]").unwrap(),
        ]
        .into_iter(),
        None,
        0,
        None,
        rng,
    );
    // The assertion failure surfaces as an execution error.
    assert!(result.is_err(), "Execution with an incorrect expected slice should fail the assertion");
}

// A program that slices inside a `finalize` block, exercising the console-only execution path
// (finalize has no circuit, so it is not cross-checked by proof verification).
const SLICE_FINALIZE_PROGRAM: &str = r"
program slice_finalize_test.aleo;

function run:
    input r0 as [u8; 5u32].public;
    input r1 as [u8; 3u32].public;
    async run r0 r1 into r2;
    output r2 as slice_finalize_test.aleo/run.future;

finalize run:
    input r0 as [u8; 5u32].public;
    input r1 as [u8; 3u32].public;
    assert.eq r0[1u32..4u32] r1;

constructor:
    assert.eq true true;
";

// Tests slicing in a finalize block: the finalize `assert.eq r0[1..4] r1` passes for the correct
// slice (accepted) and fails for a wrong one (rejected).
#[test]
fn test_slice_in_finalize() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let v16_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V16).unwrap();
    let vm = sample_vm_at_height(v16_height, rng);

    let program = Program::from_str(SLICE_FINALIZE_PROGRAM).unwrap();
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Deployment should succeed at V16");
    vm.add_next_block(&block).unwrap();

    // Correct slice: the finalize assertion passes, so the transaction is accepted.
    let execution = vm
        .execute(
            &caller_private_key,
            ("slice_finalize_test.aleo", "run"),
            [
                Value::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8, 3u8, 4u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[1u8, 2u8, 3u8]").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Finalize slice assertion should pass for the correct slice");
    assert_eq!(block.transactions().num_rejected(), 0);
    vm.add_next_block(&block).unwrap();

    // Wrong slice: the function body has no assertion, so execution succeeds, but the finalize
    // assertion fails during block production and the transaction is rejected.
    let execution = vm
        .execute(
            &caller_private_key,
            ("slice_finalize_test.aleo", "run"),
            [
                Value::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8, 3u8, 4u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8]").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0, "Finalize slice assertion should fail for a wrong slice");
    assert_eq!(block.transactions().num_rejected(), 1, "A failing finalize assertion should reject the transaction");
    vm.add_next_block(&block).unwrap();
}

// Type-check-only tests for range bounds. These use `Stack::new` (deploy-time type inference) and
// do not synthesize proving keys, so they are fast. Note the type-checker is permissive about the
// V16 feature itself (the version gate is separate); these assert the bound checks.
#[test]
fn test_slice_range_bounds_typecheck() {
    let process = Process::<CurrentNetwork>::load().unwrap();
    let typechecks = |program_str: &str| Stack::new(&process, &Program::from_str(program_str).unwrap()).is_ok();

    // Valid: ascending, in-bounds, non-empty -> [u8;3].
    assert!(
        typechecks(
            "program slice_ok.aleo;
function foo:
    input r0 as [u8; 5u32].private;
    input r1 as [u8; 3u32].private;
    assert.eq r0[1u32..4u32] r1;
    output r1 as [u8; 3u32].private;
"
        ),
        "An in-bounds ascending range should type-check"
    );
    // Reversed range (start > end) must be rejected.
    assert!(
        !typechecks(
            "program slice_rev.aleo;
function foo:
    input r0 as [u8; 5u32].private;
    input r1 as [u8; 3u32].private;
    assert.eq r0[4u32..1u32] r1;
    output r1 as [u8; 3u32].private;
"
        ),
        "A reversed range must be rejected"
    );
    // Out-of-bounds end must be rejected.
    assert!(
        !typechecks(
            "program slice_oob.aleo;
function foo:
    input r0 as [u8; 5u32].private;
    input r1 as [u8; 9u32].private;
    assert.eq r0[0u32..9u32] r1;
    output r1 as [u8; 9u32].private;
"
        ),
        "An out-of-bounds range must be rejected"
    );
    // Empty range (start == end) must be rejected, since arrays cannot be empty.
    assert!(
        !typechecks(
            "program slice_empty.aleo;
function foo:
    input r0 as [u8; 5u32].private;
    input r1 as [u8; 1u32].private;
    assert.eq r0[2u32..2u32] r1;
    output r1 as [u8; 1u32].private;
"
        ),
        "An empty range must be rejected"
    );
    // A range on a non-array (literal) operand must be rejected.
    assert!(
        !typechecks(
            "program slice_lit.aleo;
function foo:
    input r0 as u8.private;
    input r1 as [u8; 1u32].private;
    assert.eq r0[0u32..1u32] r1;
    output r1 as [u8; 1u32].private;
"
        ),
        "A range on a literal must be rejected"
    );
}

// A program that slices the OUTER dimension of a 2-D array: `[[u8;2];4][1..3] -> [[u8;2];2]`.
const NESTED_SLICE_PROGRAM: &str = r"
program nested_slice_test.aleo;

function run:
    input r0 as [[u8; 2u32]; 4u32].private;
    input r1 as [[u8; 2u32]; 2u32].private;
    assert.eq r0[1u32..3u32] r1;
    output r1 as [[u8; 2u32]; 2u32].private;

constructor:
    assert.eq true true;
";

// Tests that slicing a nested array slices the outer dimension and preserves the element type.
#[test]
fn test_slice_nested_execution() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let v16_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V16).unwrap();
    let vm = sample_vm_at_height(v16_height, rng);

    let program = Program::from_str(NESTED_SLICE_PROGRAM).unwrap();
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Deployment should succeed at V16");
    vm.add_next_block(&block).unwrap();

    // `[[0,1],[2,3],[4,5],[6,7]][1..3]` == `[[2,3],[4,5]]`.
    let execution = vm
        .execute(
            &caller_private_key,
            ("nested_slice_test.aleo", "run"),
            [
                Value::<CurrentNetwork>::from_str("[[0u8, 1u8], [2u8, 3u8], [4u8, 5u8], [6u8, 7u8]]").unwrap(),
                Value::<CurrentNetwork>::from_str("[[2u8, 3u8], [4u8, 5u8]]").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Nested-array slice should be correct");
    assert_eq!(block.transactions().num_rejected(), 0);
    vm.add_next_block(&block).unwrap();
}
