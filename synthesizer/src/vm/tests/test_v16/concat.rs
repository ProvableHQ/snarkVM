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

// A program that concatenates two arrays via a flattening `cast` (`[u8;2] ++ [u8;3] -> [u8;5]`),
// and asserts the result equals the expected array passed in as a separate input.
const CONCAT_PROGRAM: &str = r"
program concat_test.aleo;

function run:
    input r0 as [u8; 2u32].private;
    input r1 as [u8; 3u32].private;
    input r2 as [u8; 5u32].private;
    cast r0 r1 into r3 as [u8; 5u32];
    assert.eq r3 r2;
    output r3 as [u8; 5u32].private;

constructor:
    assert.eq true true;
";

// Tests that deploying a program using an array-flattening `cast` is aborted before
// `ConsensusVersion::V16` and accepted at `V16`. This exercises the type-aware flatten gate.
#[test]
fn test_deploy_concat_before_and_at_v16() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // Start one block before V16 so that after adding the (rejected) block we are exactly at V16.
    let v16_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V16).unwrap();
    let vm = sample_vm_at_height(v16_height - 1, rng);

    let program = Program::from_str(CONCAT_PROGRAM).unwrap();

    // Deployment before V16 should be aborted (the flattening cast is not yet allowed).
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    // Verify the rejection is specifically due to the V16 flatten gate, not an unrelated check.
    let error = vm.check_transaction(&deployment, None, rng).unwrap_err().to_string();
    assert!(
        error.contains("array-flattening cast") && error.contains("ConsensusVersion::V16"),
        "Expected a V16 flatten-cast gate error, but got: {error}"
    );
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

// Tests that a flattening `cast` concatenates two arrays correctly by asserting it in-program.
#[test]
fn test_concat_execution() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // Initialize the VM at V16 so that the flattening cast can be deployed.
    let v16_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V16).unwrap();
    let vm = sample_vm_at_height(v16_height, rng);

    // Deploy the concat test program.
    let program = Program::from_str(CONCAT_PROGRAM).unwrap();
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Program deployment should succeed at V16");
    vm.add_next_block(&block).unwrap();

    // Execute `run([0,1], [2,3,4], [0,1,2,3,4])`. The in-program `assert.eq` passes iff the
    // concatenation is correct.
    let execution = vm
        .execute(
            &caller_private_key,
            ("concat_test.aleo", "run"),
            [
                Value::<CurrentNetwork>::from_str("[0u8, 1u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[2u8, 3u8, 4u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8, 3u8, 4u8]").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();

    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Concatenation assertion should pass for the correct result");
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
}

// A program that flattens inside a `finalize` block, exercising the console-only execution path
// (finalize has no circuit, so it is not cross-checked by proof verification).
const CONCAT_FINALIZE_PROGRAM: &str = r"
program concat_finalize_test.aleo;

function run:
    input r0 as [u8; 2u32].public;
    input r1 as [u8; 3u32].public;
    input r2 as [u8; 5u32].public;
    async run r0 r1 r2 into r3;
    output r3 as concat_finalize_test.aleo/run.future;

finalize run:
    input r0 as [u8; 2u32].public;
    input r1 as [u8; 3u32].public;
    input r2 as [u8; 5u32].public;
    cast r0 r1 into r3 as [u8; 5u32];
    assert.eq r3 r2;

constructor:
    assert.eq true true;
";

// Tests a flattening `cast` in a finalize block: the finalize assertion passes for a correct
// concatenation (accepted) and fails for a wrong expected result (rejected).
#[test]
fn test_concat_in_finalize() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let v16_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V16).unwrap();
    let vm = sample_vm_at_height(v16_height, rng);

    let program = Program::from_str(CONCAT_FINALIZE_PROGRAM).unwrap();
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Deployment should succeed at V16");
    vm.add_next_block(&block).unwrap();

    // Correct concatenation: the finalize assertion passes, so the transaction is accepted.
    let execution = vm
        .execute(
            &caller_private_key,
            ("concat_finalize_test.aleo", "run"),
            [
                Value::<CurrentNetwork>::from_str("[0u8, 1u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[2u8, 3u8, 4u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8, 3u8, 4u8]").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Finalize concat assertion should pass for the correct result");
    assert_eq!(block.transactions().num_rejected(), 0);
    vm.add_next_block(&block).unwrap();

    // Wrong expected result: the finalize assertion fails during block production and the
    // transaction is rejected.
    let execution = vm
        .execute(
            &caller_private_key,
            ("concat_finalize_test.aleo", "run"),
            [
                Value::<CurrentNetwork>::from_str("[0u8, 1u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[2u8, 3u8, 4u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[9u8, 9u8, 9u8, 9u8, 9u8]").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0, "Finalize concat assertion should fail for a wrong result");
    assert_eq!(block.transactions().num_rejected(), 1, "A failing finalize assertion should reject the transaction");
    vm.add_next_block(&block).unwrap();
}

// A program that flattens arrays whose element type is a STRUCT, exercising the
// `matches_plaintext` (runtime) vs `types_equivalent` (type-check) whole-vs-flatten decision for
// struct element types.
const CONCAT_STRUCT_PROGRAM: &str = r"
program concat_struct_test.aleo;

struct point:
    x as u8;
    y as u8;

function run:
    input r0 as [point; 2u32].private;
    input r1 as [point; 1u32].private;
    input r2 as [point; 3u32].private;
    cast r0 r1 into r3 as [point; 3u32];
    assert.eq r3 r2;
    output r3 as [point; 3u32].private;

constructor:
    assert.eq true true;
";

// Tests that a flattening `cast` over arrays of structs concatenates correctly: `[point;2]` ++
// `[point;1]` -> `[point;3]`, verified by an in-program `assert.eq`.
#[test]
fn test_concat_struct_elements() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let v16_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V16).unwrap();
    let vm = sample_vm_at_height(v16_height, rng);

    let program = Program::from_str(CONCAT_STRUCT_PROGRAM).unwrap();
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Deployment should succeed at V16");
    vm.add_next_block(&block).unwrap();

    // Execute `run([{0,1},{2,3}], [{4,5}], [{0,1},{2,3},{4,5}])`.
    let execution = vm
        .execute(
            &caller_private_key,
            ("concat_struct_test.aleo", "run"),
            [
                Value::<CurrentNetwork>::from_str("[{ x: 0u8, y: 1u8 }, { x: 2u8, y: 3u8 }]").unwrap(),
                Value::<CurrentNetwork>::from_str("[{ x: 4u8, y: 5u8 }]").unwrap(),
                Value::<CurrentNetwork>::from_str("[{ x: 0u8, y: 1u8 }, { x: 2u8, y: 3u8 }, { x: 4u8, y: 5u8 }]")
                    .unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Struct-element concatenation should be correct");
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
}

// Type-check-only tests for flattening `cast` (fast: no proving-key synthesis). The type-checker is
// permissive about V16 itself (the version gate is separate); these assert the flatten rules.
#[test]
fn test_flatten_cast_typecheck() {
    let process = Process::<CurrentNetwork>::load().unwrap();
    let typechecks = |program_str: &str| Stack::new(&process, &Program::from_str(program_str).unwrap()).is_ok();

    // Valid flatten: [u8;2] ++ [u8;3] -> [u8;5].
    assert!(
        typechecks(
            "program flat_ok.aleo;
function foo:
    input r0 as [u8; 2u32].private;
    input r1 as [u8; 3u32].private;
    cast r0 r1 into r2 as [u8; 5u32];
    output r2 as [u8; 5u32].private;
"
        ),
        "Flattening two arrays into the summed length should type-check"
    );
    // Valid whole-match (multi-dim): three [u8;2] -> [[u8;2];3] (NOT flattened).
    assert!(
        typechecks(
            "program whole_ok.aleo;
function foo:
    input r0 as [u8; 2u32].private;
    input r1 as [u8; 2u32].private;
    input r2 as [u8; 2u32].private;
    cast r0 r1 r2 into r3 as [[u8; 2u32]; 3u32];
    output r3 as [[u8; 2u32]; 3u32].private;
"
        ),
        "Whole-match into a multi-dim array should type-check (no flattening)"
    );
    // Valid mixed element + array: u8 ++ [u8;3] -> [u8;4] (prepend).
    assert!(
        typechecks(
            "program mixed_ok.aleo;
function foo:
    input r0 as u8.private;
    input r1 as [u8; 3u32].private;
    cast r0 r1 into r2 as [u8; 4u32];
    output r2 as [u8; 4u32].private;
"
        ),
        "Mixing a scalar element and an array should type-check"
    );
    // Invalid: flattened operands do not sum to the target length (2 + 3 != 6).
    assert!(
        !typechecks(
            "program flat_sum.aleo;
function foo:
    input r0 as [u8; 2u32].private;
    input r1 as [u8; 3u32].private;
    cast r0 r1 into r2 as [u8; 6u32];
    output r2 as [u8; 6u32].private;
"
        ),
        "A flatten whose elements don't sum to the target length must be rejected"
    );
    // Invalid: two-level flatten is not allowed. [[u8;2];3] does not flatten into [u8;6].
    assert!(
        !typechecks(
            "program flat_2level.aleo;
function foo:
    input r0 as [[u8; 2u32]; 3u32].private;
    cast r0 into r1 as [u8; 6u32];
    output r1 as [u8; 6u32].private;
"
        ),
        "Two-level flattening must be rejected (flatten is one level only)"
    );
    // Invalid: element type mismatch (u16 operand into a u8 array).
    assert!(
        !typechecks(
            "program flat_mismatch.aleo;
function foo:
    input r0 as [u16; 2u32].private;
    input r1 as [u8; 3u32].private;
    cast r0 r1 into r2 as [u8; 5u32];
    output r2 as [u8; 5u32].private;
"
        ),
        "A flatten with a mismatched element type must be rejected"
    );
}

// A program with two functions that take the SAME operand shapes (three `[u8;2]`) but different
// target types: one whole-matches into `[[u8;2];3]`, the other flattens into `[u8;6]`. This proves
// the target type drives the whole-vs-flatten decision and that they produce distinct results.
const DISAMBIGUATION_PROGRAM: &str = r"
program disambig_test.aleo;

function whole:
    input r0 as [u8; 2u32].private;
    input r1 as [u8; 2u32].private;
    input r2 as [u8; 2u32].private;
    input r3 as [[u8; 2u32]; 3u32].private;
    cast r0 r1 r2 into r4 as [[u8; 2u32]; 3u32];
    assert.eq r4 r3;
    output r4 as [[u8; 2u32]; 3u32].private;

function flat:
    input r0 as [u8; 2u32].private;
    input r1 as [u8; 2u32].private;
    input r2 as [u8; 2u32].private;
    input r3 as [u8; 6u32].private;
    cast r0 r1 r2 into r4 as [u8; 6u32];
    assert.eq r4 r3;
    output r4 as [u8; 6u32].private;

constructor:
    assert.eq true true;
";

// Tests that the same operands cast to a multi-dim vs a flat target produce distinct, correct
// results (whole-match vs flatten, decided by the target type).
#[test]
fn test_flatten_disambiguation_execution() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let v16_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V16).unwrap();
    let vm = sample_vm_at_height(v16_height, rng);

    let program = Program::from_str(DISAMBIGUATION_PROGRAM).unwrap();
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Deployment should succeed at V16");
    vm.add_next_block(&block).unwrap();

    // Whole-match: three [u8;2] -> [[0,1],[2,3],[4,5]].
    let execution = vm
        .execute(
            &caller_private_key,
            ("disambig_test.aleo", "whole"),
            [
                Value::<CurrentNetwork>::from_str("[0u8, 1u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[2u8, 3u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[4u8, 5u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[[0u8, 1u8], [2u8, 3u8], [4u8, 5u8]]").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Whole-match into a multi-dim array should be correct");
    vm.add_next_block(&block).unwrap();

    // Flatten: same three [u8;2] -> [0,1,2,3,4,5].
    let execution = vm
        .execute(
            &caller_private_key,
            ("disambig_test.aleo", "flat"),
            [
                Value::<CurrentNetwork>::from_str("[0u8, 1u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[2u8, 3u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[4u8, 5u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8, 3u8, 4u8, 5u8]").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Flatten into a flat array should be correct");
    vm.add_next_block(&block).unwrap();
}

// A program exercising the composable building blocks: prepending a scalar to an array, and
// removing an element by concatenating two slices.
const COMPOSITION_PROGRAM: &str = r"
program compose_test.aleo;

function prepend:
    input r0 as u8.private;
    input r1 as [u8; 3u32].private;
    input r2 as [u8; 4u32].private;
    cast r0 r1 into r3 as [u8; 4u32];
    assert.eq r3 r2;
    output r3 as [u8; 4u32].private;

function remove_index_2:
    input r0 as [u8; 5u32].private;
    input r1 as [u8; 4u32].private;
    cast r0[0u32..2u32] r0[3u32..5u32] into r2 as [u8; 4u32];
    assert.eq r2 r1;
    output r2 as [u8; 4u32].private;

constructor:
    assert.eq true true;
";

// Tests prepend (mixed element+array flatten) and remove-element (slice + flatten composition).
#[test]
fn test_flatten_composition_execution() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let v16_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V16).unwrap();
    let vm = sample_vm_at_height(v16_height, rng);

    let program = Program::from_str(COMPOSITION_PROGRAM).unwrap();
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Deployment should succeed at V16");
    vm.add_next_block(&block).unwrap();

    // Prepend: 9 :: [0,1,2] == [9,0,1,2].
    let execution = vm
        .execute(
            &caller_private_key,
            ("compose_test.aleo", "prepend"),
            [
                Value::<CurrentNetwork>::from_str("9u8").unwrap(),
                Value::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[9u8, 0u8, 1u8, 2u8]").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Prepend (element ++ array) should be correct");
    vm.add_next_block(&block).unwrap();

    // Remove index 2 from [0,1,2,3,4] via concat of slices [0..2] ++ [3..5] == [0,1,3,4].
    let execution = vm
        .execute(
            &caller_private_key,
            ("compose_test.aleo", "remove_index_2"),
            [
                Value::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8, 3u8, 4u8]").unwrap(),
                Value::<CurrentNetwork>::from_str("[0u8, 1u8, 3u8, 4u8]").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Remove-element via slice+flatten should be correct");
    vm.add_next_block(&block).unwrap();
}
