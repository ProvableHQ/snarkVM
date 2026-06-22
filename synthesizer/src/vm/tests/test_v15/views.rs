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

//! End-to-end tests for the `view` function prototype.
//!
//! These tests build a real VM at V15 height, deploy programs containing view functions, run
//! transactions that mutate mappings via `finalize`, then call `vm.evaluate_view(...)` to
//! verify the typed return values reflect on-chain state.

use super::*;

#[cfg(feature = "history")]
use console::program::{Literal, Plaintext};

/// Convenience: extract a single `u64` from a single-output view result.
#[cfg(feature = "history")]
fn expect_u64(outputs: &[Value<CurrentNetwork>]) -> u64 {
    assert_eq!(outputs.len(), 1, "expected exactly one output, got {}", outputs.len());
    match &outputs[0] {
        Value::Plaintext(Plaintext::Literal(Literal::U64(v), _)) => **v,
        other => panic!("expected u64 plaintext, got: {other}"),
    }
}

/// Full lifecycle: deploy a program with a view function, run a transition that updates a
/// mapping via finalize, then evaluate the view and observe the new value.
#[cfg(feature = "history")]
#[test]
fn test_evaluate_view_reflects_finalize_state() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    // The program defines:
    //   - `balances` mapping
    //   - `increment(addr, amount)` transition + finalize that writes balances[addr] += amount
    //   - `total_balance(addr)` view that returns balances[addr] (default 0)
    let program = Program::from_str(
        r"
        program vw_lifecycle.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function increment:
            input r0 as address.public;
            input r1 as u64.public;
            async increment r0 r1 into r2;
            output r2 as vw_lifecycle.aleo/increment.future;

        finalize increment:
            input r0 as address.public;
            input r1 as u64.public;
            get.or_use balances[r0] 0u64 into r2;
            add r2 r1 into r3;
            set r3 into balances[r0];

        view total_balance:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    // Deploy.
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    // Read against an untouched mapping should return the default (0).
    let height = vm.block_store().current_block_height();
    let outputs = vm.evaluate_view_at_height(
        "vw_lifecycle.aleo",
        "total_balance",
        vec![Value::from_str(&caller_address.to_string())?],
        height,
    )?;
    assert_eq!(expect_u64(&outputs), 0);

    // Execute increment(addr, 10).
    let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("10u64")?];
    let tx = vm.execute(&caller_private_key, ("vw_lifecycle.aleo", "increment"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    // Read should now reflect the finalize-set value.
    let height = vm.block_store().current_block_height();
    let outputs = vm.evaluate_view_at_height(
        "vw_lifecycle.aleo",
        "total_balance",
        vec![Value::from_str(&caller_address.to_string())?],
        height,
    )?;
    assert_eq!(expect_u64(&outputs), 10);

    // Increment again by 32. New total: 42.
    let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("32u64")?];
    let tx = vm.execute(&caller_private_key, ("vw_lifecycle.aleo", "increment"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    let height = vm.block_store().current_block_height();
    let outputs = vm.evaluate_view_at_height(
        "vw_lifecycle.aleo",
        "total_balance",
        vec![Value::from_str(&caller_address.to_string())?],
        height,
    )?;
    assert_eq!(expect_u64(&outputs), 42);

    Ok(())
}

/// Read with multiple outputs returns each in declaration order.
#[cfg(feature = "history")]
#[test]
fn test_evaluate_view_multi_output() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program vw_multi_output.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        constructor:
            assert.eq true true;

        view summary:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            add r1 1u64 into r2;
            mul r1 2u64 into r3;
            output r1 as u64.public;
            output r2 as u64.public;
            output r3 as u64.public;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    let outputs = vm.evaluate_view_at_height(
        "vw_multi_output.aleo",
        "summary",
        vec![Value::from_str(&caller_address.to_string())?],
        vm.block_store().current_block_height(),
    )?;

    // Default 0u64 for the unknown key, then +1 and *2.
    assert_eq!(outputs.len(), 3);
    assert_eq!(expect_u64(&outputs[0..1]), 0);
    assert_eq!(expect_u64(&outputs[1..2]), 1);
    assert_eq!(expect_u64(&outputs[2..3]), 0);
    Ok(())
}

/// Read with multiple inputs computes a typed return.
#[cfg(feature = "history")]
#[test]
fn test_evaluate_view_multi_input() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // Pure-arithmetic view, no mappings needed.
    let program = Program::from_str(
        r"
        program vw_multi_in.aleo;

        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        constructor:
            assert.eq true true;

        view add3:
            input r0 as u64.public;
            input r1 as u64.public;
            input r2 as u64.public;
            add r0 r1 into r3;
            add r3 r2 into r4;
            output r4 as u64.public;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    let outputs = vm.evaluate_view_at_height(
        "vw_multi_in.aleo",
        "add3",
        vec![Value::from_str("10u64")?, Value::from_str("20u64")?, Value::from_str("12u64")?],
        vm.block_store().current_block_height(),
    )?;
    assert_eq!(expect_u64(&outputs), 42);
    Ok(())
}

/// Read that uses `branch.eq` to take a forward branch over a noop. The output is
/// invariant under the branch (computed before it), but the branch path itself is
/// exercised: when `r0 == 0`, the doubling step is skipped at runtime; when `r0 != 0`,
/// the doubling step runs and writes `r2`. Either way the view's declared output
/// references `r1`, which is always written. This proves the view evaluator handles
/// `branch.eq` without crashing under both taken and not-taken paths.
#[cfg(feature = "history")]
#[test]
fn test_evaluate_view_with_branch() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    let program = Program::from_str(
        r"
        program vw_branch.aleo;

        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        constructor:
            assert.eq true true;

        view maybe_extra:
            input r0 as u64.public;
            add r0 10u64 into r1;
            branch.eq r0 0u64 to skip;
            add r1 r1 into r2;
            position skip;
            output r1 as u64.public;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    // r0 = 0: branch is taken, doubling is skipped.
    let outputs = vm.evaluate_view_at_height(
        "vw_branch.aleo",
        "maybe_extra",
        vec![Value::from_str("0u64")?],
        vm.block_store().current_block_height(),
    )?;
    assert_eq!(expect_u64(&outputs), 10);

    // r0 = 5: branch is NOT taken, the unused r2 destination is written but we still output r1.
    let outputs = vm.evaluate_view_at_height(
        "vw_branch.aleo",
        "maybe_extra",
        vec![Value::from_str("5u64")?],
        vm.block_store().current_block_height(),
    )?;
    assert_eq!(expect_u64(&outputs), 15);
    Ok(())
}

/// Read with no inputs (views a fixed-key mapping or just returns a constant).
#[cfg(feature = "history")]
#[test]
fn test_evaluate_view_zero_inputs() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    let program = Program::from_str(
        r"
        program vw_zeroin.aleo;

        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        constructor:
            assert.eq true true;

        view fixed_value:
            add 0u64 1234u64 into r0;
            output r0 as u64.public;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    let outputs =
        vm.evaluate_view_at_height("vw_zeroin.aleo", "fixed_value", vec![], vm.block_store().current_block_height())?;
    assert_eq!(expect_u64(&outputs), 1234);
    Ok(())
}

/// Calling evaluate_view with the wrong input arity returns an error.
#[cfg(feature = "history")]
#[test]
fn test_evaluate_view_arity_mismatch() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    let program = Program::from_str(
        r"
        program vw_arity.aleo;

        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        constructor:
            assert.eq true true;

        view takes_two:
            input r0 as u64.public;
            input r1 as u64.public;
            add r0 r1 into r2;
            output r2 as u64.public;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    // Too few inputs.
    let result = vm.evaluate_view_at_height(
        "vw_arity.aleo",
        "takes_two",
        vec![Value::from_str("1u64")?],
        vm.block_store().current_block_height(),
    );
    assert!(result.is_err(), "expected error for too few inputs");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("expects 2"), "error should mention input count: {err}");

    // Too many inputs.
    let result = vm.evaluate_view_at_height(
        "vw_arity.aleo",
        "takes_two",
        vec![Value::from_str("1u64")?, Value::from_str("2u64")?, Value::from_str("3u64")?],
        vm.block_store().current_block_height(),
    );
    assert!(result.is_err(), "expected error for too many inputs");

    Ok(())
}

/// Calling a view that does not exist on a deployed program returns a "not defined" error.
#[cfg(feature = "history")]
#[test]
fn test_evaluate_view_unknown_view() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    let program = Program::from_str(
        r"
        program vw_unknown.aleo;

        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        constructor:
            assert.eq true true;

        view existing:
            add 0u64 1u64 into r0;
            output r0 as u64.public;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    let result =
        vm.evaluate_view_at_height("vw_unknown.aleo", "missing", vec![], vm.block_store().current_block_height());
    assert!(result.is_err(), "expected error for unknown view");
    Ok(())
}

/// Calling evaluate_view against a program that was never deployed returns a "no such program" error.
#[cfg(feature = "history")]
#[test]
fn test_evaluate_view_unknown_program() {
    let rng = &mut TestRng::default();
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);

    let result =
        vm.evaluate_view_at_height("never_deployed.aleo", "anything", vec![], vm.block_store().current_block_height());
    assert!(result.is_err(), "expected error for unknown program");
}

/// Construction-time rejection: declared output type must match the operand's actual type.
#[test]
fn test_view_output_type_mismatch_rejected() {
    // `r1` is a `u64`, but the view declares its output as `u32`.
    let result = Program::<CurrentNetwork>::from_str(
        r"
        program vw_typecheck.aleo;

        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        constructor:
            assert.eq true true;

        view bad_output:
            input r0 as u64.public;
            add r0 1u64 into r1;
            output r1 as u32.public;
        ",
    );
    // Parsing succeeds (the view is structurally valid); the mismatch is caught when the
    // stack is computed from the program at deploy time. We assert here that *either* parse
    // or the subsequent type-check rejects this — whichever fails first is acceptable.
    if let Ok(program) = result {
        let rng = &mut TestRng::default();
        let caller_private_key = sample_genesis_private_key(rng);
        let caller_address = Address::try_from(&caller_private_key).unwrap();
        let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);
        let deploy = vm.deploy(&caller_private_key, &program, None, 0, None, rng);
        assert!(deploy.is_err(), "deploy should reject type-mismatched view output");
        let _ = caller_address;
    }
}

/// Construction-time rejection: write-style commands inside a view are rejected at parse.
#[test]
fn test_view_rejects_write_commands_at_parse() {
    let cases = [
        // `set` into a mapping
        r"
        program vw_bad_set.aleo;
        mapping m:
            key as u64.public;
            value as u64.public;
        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        constructor:
            assert.eq true true;
        view bad:
            input r0 as u64.public;
            set r0 into m[r0];
            output r0 as u64.public;
        ",
        // `remove` from a mapping
        r"
        program vw_bad_rm.aleo;
        mapping m:
            key as u64.public;
            value as u64.public;
        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        constructor:
            assert.eq true true;
        view bad:
            input r0 as u64.public;
            remove m[r0];
            output r0 as u64.public;
        ",
        // `rand.chacha`
        r"
        program vw_bad_rand.aleo;
        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        constructor:
            assert.eq true true;
        view bad:
            rand.chacha into r0 as u64;
            output r0 as u64.public;
        ",
    ];

    for source in cases {
        let result = Program::<CurrentNetwork>::from_str(source);
        assert!(result.is_err(), "expected parse error for view with forbidden command:\n{source}");
    }
}

/// Tests that a program containing a `view` block is rejected at V14 (since `view` is V15
/// syntax) and accepted at V15. Without this gate, deploying such a program pre-V15 would
/// fork: new nodes accept the bytes, old nodes reject the unknown component variant.
#[test]
fn test_deploy_view_before_and_at_v15() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // Start one block before V15 so that after the (rejected) deployment block we are
    // exactly at V15 and the same program is accepted.
    let v15_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap();
    let vm = sample_vm_at_height(v15_height - 1, rng);

    let program = Program::from_str(
        r"
program vw_v15_gate.aleo;

function noop:
    input r0 as u64.private;
    output r0 as u64.private;

constructor:
    assert.eq true true;

view fixed_value:
    add 0u64 1234u64 into r0;
    output r0 as u64.public;
",
    )
    .unwrap();

    // Deployment before V15 should be aborted.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0, "Deployment with view before V15 should not be accepted");
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 1, "Deployment with view before V15 should be aborted");
    vm.add_next_block(&block).unwrap();

    // We should now be at V15.
    assert_eq!(vm.block_store().current_block_height(), v15_height);

    // Deployment at V15 should succeed.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Deployment with view at V15 should be accepted");
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
}

/// Tests that a view containing a `string` type is rejected at deploy via the strengthened
/// `Program::contains_string_type` (which now walks `self.views`). Without that fix, strings
/// could sneak in through view inputs/outputs even though they're banned post-V12.
#[test]
fn test_deploy_view_with_string_type_rejected() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);

    let program = Program::from_str(
        r"
program vw_string_input.aleo;

function noop:
    input r0 as u64.private;
    output r0 as u64.private;

constructor:
    assert.eq true true;

view echo:
    input r0 as string.public;
    add 0u64 0u64 into r1;
    output r0 as string.public;
",
    )
    .unwrap();

    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0, "Deployment with string-typed view input should be rejected");
    assert_eq!(block.aborted_transaction_ids().len(), 1);
}

/// Tests that a finalize body can call a view function in the SAME program and route its
/// return value through normal finalize logic. Deploys a program with a `lookup` view, then
/// executes a function whose finalize calls `lookup`, doubles the result, and writes it back.
#[test]
fn test_finalize_calls_same_program_view() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program vw_call_same.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        mapping doubled:
            key as address.public;
            value as u64.public;

        function seed:
            input r0 as address.public;
            input r1 as u64.public;
            async seed r0 r1 into r2;
            output r2 as vw_call_same.aleo/seed.future;

        finalize seed:
            input r0 as address.public;
            input r1 as u64.public;
            set r1 into balances[r0];

        function compute_doubled:
            input r0 as address.public;
            async compute_doubled r0 into r1;
            output r1 as vw_call_same.aleo/compute_doubled.future;

        // The finalize body calls the in-program `lookup` view, multiplies the result by 2,
        // and stores it in the `doubled` mapping.
        finalize compute_doubled:
            input r0 as address.public;
            call lookup r0 into r1;
            mul r1 2u64 into r2;
            set r2 into doubled[r0];

        view lookup:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    // Deploy.
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    // Seed `balances[caller] = 21`.
    let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("21u64")?];
    let tx = vm.execute(&caller_private_key, ("vw_call_same.aleo", "seed"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    // Run `compute_doubled(caller)`. Its finalize calls `lookup(caller)` (→ 21), doubles
    // (→ 42), and writes to `doubled[caller]`.
    let inputs = [Value::from_str(&caller_address.to_string())?];
    let tx =
        vm.execute(&caller_private_key, ("vw_call_same.aleo", "compute_doubled"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    // Confirm the new mapping value via an external read.
    #[cfg(feature = "history")]
    {
        let outputs = vm.evaluate_view_at_height(
            "vw_call_same.aleo",
            "lookup",
            vec![Value::from_str(&caller_address.to_string())?],
            vm.block_store().current_block_height(),
        )?;
        // The view reads `balances`, which still holds 21 (untouched by `compute_doubled`'s finalize).
        assert_eq!(expect_u64(&outputs), 21, "external view of `lookup` should still see balances=21");
    }

    Ok(())
}

/// Tests that a finalize body can call a view function in an IMPORTED (external) program.
#[test]
fn test_finalize_calls_cross_program_view() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    // The "data" program: holds the `balances` mapping and the `lookup` view.
    let data_program = Program::from_str(
        r"
        program vw_call_data.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function seed:
            input r0 as address.public;
            input r1 as u64.public;
            async seed r0 r1 into r2;
            output r2 as vw_call_data.aleo/seed.future;

        finalize seed:
            input r0 as address.public;
            input r1 as u64.public;
            set r1 into balances[r0];

        view lookup:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )?;

    // The "caller" program: imports `vw_call_data.aleo` and calls its `lookup` view from
    // within its own finalize body.
    let caller_program = Program::from_str(
        r"
        import vw_call_data.aleo;

        program vw_call_caller.aleo;

        mapping doubled:
            key as address.public;
            value as u64.public;

        function compute_doubled:
            input r0 as address.public;
            async compute_doubled r0 into r1;
            output r1 as vw_call_caller.aleo/compute_doubled.future;

        finalize compute_doubled:
            input r0 as address.public;
            call vw_call_data.aleo/lookup r0 into r1;
            mul r1 3u64 into r2;
            set r2 into doubled[r0];

        constructor:
            assert.eq true true;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    // Deploy the data program, then the caller program (which imports it).
    let tx_data = vm.deploy(&caller_private_key, &data_program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx_data], rng);
    let tx_caller = vm.deploy(&caller_private_key, &caller_program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx_caller], rng);

    // Seed `balances[caller] = 14` in the data program.
    let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("14u64")?];
    let tx = vm.execute(&caller_private_key, ("vw_call_data.aleo", "seed"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    // Run `compute_doubled(caller)`. Its finalize calls `vw_call_data.aleo/lookup(caller)`
    // (→ 14), multiplies by 3 (→ 42), and writes to `doubled[caller]`.
    let inputs = [Value::from_str(&caller_address.to_string())?];
    let tx =
        vm.execute(&caller_private_key, ("vw_call_caller.aleo", "compute_doubled"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    Ok(())
}

/// Tests that a finalize body calling a NON-view target (i.e. a regular function) is rejected
/// at deploy time. The type-check resolves the target and bails because it is not a view.
#[test]
fn test_finalize_calls_non_view_rejected_at_deploy() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let _caller_address = Address::try_from(&caller_private_key).unwrap();

    let program = Program::from_str(
        r"
        program vw_bad_call.aleo;

        function helper:
            input r0 as u64.private;
            output r0 as u64.private;

        function caller:
            input r0 as u64.public;
            async caller r0 into r1;
            output r1 as vw_bad_call.aleo/caller.future;

        finalize caller:
            input r0 as u64.public;
            call helper r0 into r1;
            assert.eq r1 r1;

        constructor:
            assert.eq true true;
        ",
    );
    // The program may either fail to parse or fail to deploy depending on which layer catches
    // it first; either way it must NOT successfully deploy.
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);
    if let Ok(program) = program {
        let deploy = vm.deploy(&caller_private_key, &program, None, 0, None, rng);
        assert!(deploy.is_err(), "deploy should reject a finalize that calls a non-view target");
    }
}

/// Tests that pre-V15 deployments of a program containing a `call` in finalize are rejected.
/// `contains_v15_syntax` flags any `call` in a finalize body as V15 syntax.
#[test]
fn test_deploy_finalize_call_before_and_at_v15() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    let v15_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap();
    let vm = sample_vm_at_height(v15_height - 1, rng);

    let program = Program::from_str(
        r"
        program vw_v15_finalize_call.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        function caller:
            input r0 as address.public;
            async caller r0 into r1;
            output r1 as vw_v15_finalize_call.aleo/caller.future;

        finalize caller:
            input r0 as address.public;
            call lookup r0 into r1;
            set r1 into balances[r0];

        view lookup:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )
    .unwrap();

    // Pre-V15: rejected.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0, "Deployment with finalize-call before V15 should be rejected");
    assert_eq!(block.aborted_transaction_ids().len(), 1);
    vm.add_next_block(&block).unwrap();
    assert_eq!(vm.block_store().current_block_height(), v15_height);

    // V15: accepted.
    let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Deployment with finalize-call at V15 should be accepted");
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
}

/// Tests two semantics together:
///   1. A single `finalize` body may call the same view multiple times.
///   2. The second call sees the caller's intervening `set` to the same mapping — i.e. each
///      view call observes the live finalize-batch state at the time of the call, including
///      the caller's own pending writes.
///
/// Program shape: `balances` is seeded to 11 via `seed`, then `compute`'s finalize calls
/// `lookup` twice with a `set balances[r0] = 55` in between. The two view outputs are packed
/// as `v_old * 1000 + v_new` and stored in `before_after[r0]`. We assert it equals 11_055,
/// which uniquely encodes (11, 55) — proving the first call saw 11 and the second saw 55.
#[test]
fn test_finalize_multiple_calls_and_interleaved_writes() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program vw_call_seq.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        mapping before_after:
            key as address.public;
            value as u64.public;

        function seed:
            input r0 as address.public;
            input r1 as u64.public;
            async seed r0 r1 into r2;
            output r2 as vw_call_seq.aleo/seed.future;

        finalize seed:
            input r0 as address.public;
            input r1 as u64.public;
            set r1 into balances[r0];

        function compute:
            input r0 as address.public;
            async compute r0 into r1;
            output r1 as vw_call_seq.aleo/compute.future;

        finalize compute:
            input r0 as address.public;
            call lookup r0 into r1;
            set 55u64 into balances[r0];
            call lookup r0 into r2;
            mul r1 1000u64 into r3;
            add r3 r2 into r4;
            set r4 into before_after[r0];

        view lookup:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    // Deploy.
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    // Seed `balances[caller] = 11`.
    let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("11u64")?];
    let tx = vm.execute(&caller_private_key, ("vw_call_seq.aleo", "seed"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    // Run `compute(caller)`. The finalize:
    //   - first call → v_old = 11
    //   - set balances[caller] = 55
    //   - second call → v_new = 55
    //   - store 11 * 1000 + 55 = 11055 in `before_after[caller]`.
    let inputs = [Value::from_str(&caller_address.to_string())?];
    let tx = vm.execute(&caller_private_key, ("vw_call_seq.aleo", "compute"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    // Confirm both calls observed the expected (old, new) pair via the encoded result.
    #[cfg(feature = "history")]
    {
        // We expose the encoded value via `lookup` on `before_after` — but `lookup` reads
        // `balances`, not `before_after`. Use a direct historic mapping read instead.
        let height = vm.block_store().current_block_height();
        let key = console::program::Plaintext::from(console::program::Literal::Address(caller_address));
        let value = vm
            .finalize_store()
            .get_historical_mapping_value(
                *program.id(),
                console::program::Identifier::from_str("before_after")?,
                key,
                height,
            )?
            .expect("before_after should have a value at the current height");
        match &*value {
            Value::Plaintext(console::program::Plaintext::Literal(console::program::Literal::U64(encoded), _)) => {
                assert_eq!(
                    **encoded, 11_055u64,
                    "expected v_old*1000 + v_new = 11055, got {encoded}; the two view calls must \
                     observe (11, 55) respectively, with the intervening `set` visible to the second call",
                );
            }
            other => panic!("expected u64 plaintext, got {other:?}"),
        }
    }

    // Also confirm the final committed balance is 55 (the intervening set landed).
    #[cfg(feature = "history")]
    {
        let outputs = vm.evaluate_view_at_height(
            "vw_call_seq.aleo",
            "lookup",
            vec![Value::from_str(&caller_address.to_string())?],
            vm.block_store().current_block_height(),
        )?;
        assert_eq!(expect_u64(&outputs), 55, "final balance after `compute` must be 55");
    }

    Ok(())
}

/// Cross-program negative: an importing program's finalize body calls a regular `function` in an
/// imported program (not a view). Same shape as the same-program negative test, but goes through
/// the `CallOperator::Locator` resolution path. Must be rejected at deploy time.
#[test]
fn test_finalize_calls_cross_program_non_view_rejected_at_deploy() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // Imported program: exposes a regular `function` (no view).
    let data_program = Program::from_str(
        r"
        program vw_bad_cross_data.aleo;

        function helper:
            input r0 as u64.public;
            output r0 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )
    .unwrap();

    // Caller: tries to `call vw_bad_cross_data.aleo/helper` from within its finalize body.
    let caller_program = Program::from_str(
        r"
        import vw_bad_cross_data.aleo;

        program vw_bad_cross_caller.aleo;

        function caller:
            input r0 as u64.public;
            async caller r0 into r1;
            output r1 as vw_bad_cross_caller.aleo/caller.future;

        finalize caller:
            input r0 as u64.public;
            call vw_bad_cross_data.aleo/helper r0 into r1;
            assert.eq r1 r1;

        constructor:
            assert.eq true true;
        ",
    );

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);
    let tx_data = vm.deploy(&caller_private_key, &data_program, None, 0, None, rng).unwrap();
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx_data], rng);

    // Either parse or deploy must reject — same pattern as the same-program negative test.
    if let Ok(caller_program) = caller_program {
        let deploy = vm.deploy(&caller_private_key, &caller_program, None, 0, None, rng);
        assert!(deploy.is_err(), "deploy should reject a finalize that calls a non-view target in an imported program");
    }
}

/// Negative: deploy is rejected when the call's operand count does not match the view's input
/// arity. Exercises the arity check in `Call::output_types`.
#[test]
fn test_finalize_call_arity_mismatch_rejected_at_deploy() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // `lookup` declares one input, but the caller passes two.
    let program = Program::from_str(
        r"
        program vw_call_arity.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function caller:
            input r0 as address.public;
            async caller r0 into r1;
            output r1 as vw_call_arity.aleo/caller.future;

        finalize caller:
            input r0 as address.public;
            call lookup r0 r0 into r1;
            assert.eq r1 r1;

        view lookup:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    );

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);
    if let Ok(program) = program {
        let deploy = vm.deploy(&caller_private_key, &program, None, 0, None, rng);
        assert!(deploy.is_err(), "deploy should reject a finalize-call with wrong arity");
    }
}

/// Negative: deploy is rejected when a destination of a finalize-call is consumed downstream as
/// a type that does not match the view's declared output type. Proves that `Call::output_types`
/// propagates the view's output types into the surrounding finalize type-check.
#[test]
fn test_finalize_call_destination_type_mismatch_rejected_at_deploy() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // `lookup` outputs `u64`, but the caller treats `r1` as `u32` in the following `add`.
    let program = Program::from_str(
        r"
        program vw_call_type_mismatch.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function caller:
            input r0 as address.public;
            async caller r0 into r1;
            output r1 as vw_call_type_mismatch.aleo/caller.future;

        finalize caller:
            input r0 as address.public;
            call lookup r0 into r1;
            add r1 1u32 into r2;
            assert.eq r2 r2;

        view lookup:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    );

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);
    if let Ok(program) = program {
        let deploy = vm.deploy(&caller_private_key, &program, None, 0, None, rng);
        assert!(deploy.is_err(), "deploy should reject when call destination type conflicts with downstream use");
    }
}

/// Construction-time rejection: a `call` inside a view body is rejected. Views are leaves in
/// the call graph — `ViewCore::add_command` rejects `is_call()` so a view cannot invoke another
/// view (or any function), preventing recursion at the structural level.
#[test]
fn test_view_rejects_call_command_at_parse() {
    let result = Program::<CurrentNetwork>::from_str(
        r"
        program vw_bad_call_in_view.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        constructor:
            assert.eq true true;

        view inner:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        view outer:
            input r0 as address.public;
            call inner r0 into r1;
            output r1 as u64.public;
        ",
    );
    assert!(result.is_err(), "expected parse error for a view body containing `call`");
}

/// Negative: a finalize body that uses a `Locator` form to call its own program (instead of
/// the `Resource` form) is rejected at deploy. Same-program calls must use the bare resource
/// name; a self-locator is treated as an error since the locator form is reserved for external
/// programs.
#[test]
fn test_finalize_call_self_locator_rejected_at_deploy() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    let program = Program::from_str(
        r"
        program vw_self_locator.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function caller:
            input r0 as address.public;
            async caller r0 into r1;
            output r1 as vw_self_locator.aleo/caller.future;

        finalize caller:
            input r0 as address.public;
            call vw_self_locator.aleo/lookup r0 into r1;
            assert.eq r1 r1;

        view lookup:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    );

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);
    if let Ok(program) = program {
        let deploy = vm.deploy(&caller_private_key, &program, None, 0, None, rng);
        assert!(deploy.is_err(), "deploy should reject a finalize-call using a self-locator");
    }
}

/// Negative: a finalize body that calls into an external program which is not declared in
/// `import` is rejected at deploy. The target program is deployed independently, but the caller
/// never imports it — the explicit import check in the finalize type-check fires before the
/// external stack lookup.
#[test]
fn test_finalize_call_missing_import_rejected_at_deploy() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // Target program with a view (deployed first). Includes a noop function so the program
    // has at least one deployable function alongside the view.
    let data_program = Program::from_str(
        r"
        program vw_no_import_data.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        view lookup:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )
    .unwrap();

    // Caller that references `vw_no_import_data.aleo/lookup` from finalize but does NOT include
    // `import vw_no_import_data.aleo;`.
    let caller_program = Program::from_str(
        r"
        program vw_no_import_caller.aleo;

        function caller:
            input r0 as address.public;
            async caller r0 into r1;
            output r1 as vw_no_import_caller.aleo/caller.future;

        finalize caller:
            input r0 as address.public;
            call vw_no_import_data.aleo/lookup r0 into r1;
            assert.eq r1 r1;

        constructor:
            assert.eq true true;
        ",
    );

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);
    let tx_data = vm.deploy(&caller_private_key, &data_program, None, 0, None, rng).unwrap();
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx_data], rng);

    if let Ok(caller_program) = caller_program {
        let deploy = vm.deploy(&caller_private_key, &caller_program, None, 0, None, rng);
        assert!(deploy.is_err(), "deploy should reject a finalize-call into a program that is not imported");
    }
}

/// Construction-time rejection: `call.dynamic` is forbidden inside a finalize body. The
/// `Finalize::add_command` guard rejects `is_dynamic_call`, so parsing the program must fail.
#[test]
fn test_finalize_rejects_call_dynamic_at_parse() {
    let result = Program::<CurrentNetwork>::from_str(
        r"
        program vw_bad_call_dynamic.aleo;

        function caller:
            input r0 as field.public;
            input r1 as field.public;
            input r2 as field.public;
            async caller r0 r1 r2 into r3;
            output r3 as vw_bad_call_dynamic.aleo/caller.future;

        finalize caller:
            input r0 as field.public;
            input r1 as field.public;
            input r2 as field.public;
            call.dynamic r0 r1 r2 into r3 (as u64.public);
            assert.eq r3 r3;

        constructor:
            assert.eq true true;
        ",
    );
    assert!(result.is_err(), "expected parse error for `call.dynamic` inside a finalize body");
}

/// Behavioral: a single finalize-to-view call returns multiple primitive values of different
/// types, each routed into its own destination register and written to a distinct mapping. This
/// exercises:
///   - the destination-zip path in `Call::output_types` for N>1 outputs,
///   - per-destination type propagation when output types differ (u64, boolean, address),
///   - end-to-end storage of each typed value via `set`.
#[test]
fn test_finalize_call_multi_output_multi_type() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program vw_multi_type.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        mapping flags:
            key as address.public;
            value as boolean.public;

        mapping owners:
            key as address.public;
            value as address.public;

        function compute:
            input r0 as address.public;
            async compute r0 into r1;
            output r1 as vw_multi_type.aleo/compute.future;

        finalize compute:
            input r0 as address.public;
            call summary r0 into r1 r2 r3;
            set r1 into balances[r0];
            set r2 into flags[r0];
            set r3 into owners[r0];

        view summary:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            add r1 7u64 into r2;
            gt r2 5u64 into r3;
            output r2 as u64.public;
            output r3 as boolean.public;
            output r0 as address.public;

        constructor:
            assert.eq true true;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    // Deploy.
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    // Run `compute(caller)`. The view observes balances[caller]=0, returns (7u64, true, caller),
    // and the finalize routes each output into its respective mapping.
    let inputs = [Value::from_str(&caller_address.to_string())?];
    let tx = vm.execute(&caller_private_key, ("vw_multi_type.aleo", "compute"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    // Verify each destination received the expected typed value.
    #[cfg(feature = "history")]
    {
        let height = vm.block_store().current_block_height();
        let key = console::program::Plaintext::from(console::program::Literal::Address(caller_address));
        let program_id = *program.id();

        let balances_value = vm
            .finalize_store()
            .get_historical_mapping_value(
                program_id,
                console::program::Identifier::from_str("balances")?,
                key.clone(),
                height,
            )?
            .expect("balances should be set");
        match &*balances_value {
            Value::Plaintext(console::program::Plaintext::Literal(console::program::Literal::U64(v), _)) => {
                assert_eq!(**v, 7u64, "balances[caller] must equal the view's u64 output (0 + 7)");
            }
            other => panic!("expected u64 plaintext for balances, got {other:?}"),
        }

        let flags_value = vm
            .finalize_store()
            .get_historical_mapping_value(
                program_id,
                console::program::Identifier::from_str("flags")?,
                key.clone(),
                height,
            )?
            .expect("flags should be set");
        match &*flags_value {
            Value::Plaintext(console::program::Plaintext::Literal(console::program::Literal::Boolean(v), _)) => {
                assert!(**v, "flags[caller] must equal the view's boolean output (7 > 5)");
            }
            other => panic!("expected boolean plaintext for flags, got {other:?}"),
        }

        let owners_value = vm
            .finalize_store()
            .get_historical_mapping_value(program_id, console::program::Identifier::from_str("owners")?, key, height)?
            .expect("owners should be set");
        match &*owners_value {
            Value::Plaintext(console::program::Plaintext::Literal(console::program::Literal::Address(v), _)) => {
                assert_eq!(*v, caller_address, "owners[caller] must equal the view's address output");
            }
            other => panic!("expected address plaintext for owners, got {other:?}"),
        }
    }

    Ok(())
}

/// Behavioral: a view in one program returns a struct value, and an importing program's
/// finalize body calls into it, extracts a struct field from the destination, and stores it.
/// Exercises `RegisterType::qualify` on a struct type crossing the program boundary: the
/// destination is typed as `vw_struct_data.aleo/Summary` in the caller, and downstream field
/// access (`r1.total`) must resolve against the external struct definition.
#[test]
fn test_finalize_call_struct_return_cross_program() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let data_program = Program::from_str(
        r"
        program vw_struct_data.aleo;

        struct Summary:
            total as u64;
            flag as boolean;

        mapping balances:
            key as address.public;
            value as u64.public;

        function seed:
            input r0 as address.public;
            input r1 as u64.public;
            async seed r0 r1 into r2;
            output r2 as vw_struct_data.aleo/seed.future;

        finalize seed:
            input r0 as address.public;
            input r1 as u64.public;
            set r1 into balances[r0];

        view summarize:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            gt r1 50u64 into r2;
            cast r1 r2 into r3 as Summary;
            output r3 as Summary.public;

        constructor:
            assert.eq true true;
        ",
    )?;

    let caller_program = Program::from_str(
        r"
        import vw_struct_data.aleo;

        program vw_struct_caller.aleo;

        mapping totals:
            key as address.public;
            value as u64.public;

        function compute:
            input r0 as address.public;
            async compute r0 into r1;
            output r1 as vw_struct_caller.aleo/compute.future;

        finalize compute:
            input r0 as address.public;
            call vw_struct_data.aleo/summarize r0 into r1;
            add r1.total 0u64 into r2;
            set r2 into totals[r0];

        constructor:
            assert.eq true true;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    // Deploy data then caller (caller imports data).
    let tx_data = vm.deploy(&caller_private_key, &data_program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx_data], rng);
    let tx_caller = vm.deploy(&caller_private_key, &caller_program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx_caller], rng);

    // Seed balances[caller] = 77 in the data program.
    let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("77u64")?];
    let tx = vm.execute(&caller_private_key, ("vw_struct_data.aleo", "seed"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    // Run `compute(caller)`. The finalize calls `summarize` which returns Summary{total: 77,
    // flag: true}, extracts the `total` field, and stores it in `totals`.
    let inputs = [Value::from_str(&caller_address.to_string())?];
    let tx =
        vm.execute(&caller_private_key, ("vw_struct_caller.aleo", "compute"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    // Verify totals[caller] = 77 (the extracted .total field of the returned struct).
    #[cfg(feature = "history")]
    {
        let height = vm.block_store().current_block_height();
        let key = console::program::Plaintext::from(console::program::Literal::Address(caller_address));
        let totals_value = vm
            .finalize_store()
            .get_historical_mapping_value(
                *caller_program.id(),
                console::program::Identifier::from_str("totals")?,
                key,
                height,
            )?
            .expect("totals should be set");
        match &*totals_value {
            Value::Plaintext(console::program::Plaintext::Literal(console::program::Literal::U64(v), _)) => {
                assert_eq!(**v, 77u64, "totals[caller] must equal the .total field of the cross-program struct return");
            }
            other => panic!("expected u64 plaintext, got {other:?}"),
        }
    }

    Ok(())
}

/// Negative: deploy is rejected when the call lists more destinations than the view declares
/// outputs. Mirrors the operand-arity test but exercises the destination-count check in
/// `Call::output_types_for_view`.
#[test]
fn test_finalize_call_destination_count_mismatch_rejected_at_deploy() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // `lookup` declares one output, but the caller binds two destinations.
    let program = Program::from_str(
        r"
        program vw_call_dest_count.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function caller:
            input r0 as address.public;
            async caller r0 into r1;
            output r1 as vw_call_dest_count.aleo/caller.future;

        finalize caller:
            input r0 as address.public;
            call lookup r0 into r1 r2;
            assert.eq r1 r1;

        view lookup:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    );

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);
    if let Ok(program) = program {
        let deploy = vm.deploy(&caller_private_key, &program, None, 0, None, rng);
        assert!(
            deploy.is_err(),
            "deploy should reject a finalize-call binding more destinations than the view returns"
        );
    }
}

/// Negative: deploy is rejected when an operand's register type does not match the view's
/// declared input type. The caller passes a `u32` register where the view expects `u64`. The
/// per-operand type check in `Call::output_types_for_view` rejects this at deploy rather than
/// deferring to runtime.
#[test]
fn test_finalize_call_input_type_mismatch_rejected_at_deploy() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // The finalize body propagates a `u32` register as the operand to `lookup`, which expects `u64`.
    let program = Program::from_str(
        r"
        program vw_call_in_type.aleo;

        mapping totals:
            key as address.public;
            value as u64.public;

        function caller:
            input r0 as address.public;
            input r1 as u32.public;
            async caller r0 r1 into r2;
            output r2 as vw_call_in_type.aleo/caller.future;

        finalize caller:
            input r0 as address.public;
            input r1 as u32.public;
            call double r1 into r2;
            set r2 into totals[r0];

        view double:
            input r0 as u64.public;
            add r0 r0 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    );

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);
    if let Ok(program) = program {
        let deploy = vm.deploy(&caller_private_key, &program, None, 0, None, rng);
        assert!(deploy.is_err(), "deploy should reject a finalize-call with a wrong-typed operand");
    }
}

/// Runtime: a view body that errors at runtime (e.g. `get` on a missing key) propagates the
/// failure through the in-finalize call, and the surrounding transaction is finalize-rejected.
/// The deploy itself succeeds — the failure is exclusively a runtime path through
/// `evaluate_call_to_view` -> `evaluate_view_inner` -> per-command bail.
#[test]
fn test_finalize_call_view_runtime_failure_rejects_tx() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    // `strict_lookup` uses a non-`or_use` `get`, so a missing key surfaces an error.
    let program = Program::from_str(
        r"
        program vw_call_runtime_fail.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        mapping out:
            key as address.public;
            value as u64.public;

        function caller:
            input r0 as address.public;
            async caller r0 into r1;
            output r1 as vw_call_runtime_fail.aleo/caller.future;

        finalize caller:
            input r0 as address.public;
            call strict_lookup r0 into r1;
            set r1 into out[r0];

        view strict_lookup:
            input r0 as address.public;
            get balances[r0] into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    // Deploy succeeds.
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    // Execute `caller(caller_address)` — `balances[caller_address]` is unset, so the view's
    // strict `get` fails. The block must finalize-reject the transaction.
    let inputs = [Value::from_str(&caller_address.to_string())?];
    let tx =
        vm.execute(&caller_private_key, ("vw_call_runtime_fail.aleo", "caller"), inputs.iter(), None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[tx], rng)?;
    assert_eq!(block.transactions().num_accepted(), 0, "tx should not be accepted when the called view fails");
    assert_eq!(block.transactions().num_rejected(), 1, "the failing finalize must surface as a rejected tx");
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block)?;

    // The `out` mapping must be unchanged — finalize rejection rolls the batch back.
    #[cfg(feature = "history")]
    {
        let height = vm.block_store().current_block_height();
        let key = console::program::Plaintext::from(console::program::Literal::Address(caller_address));
        let out_value = vm.finalize_store().get_historical_mapping_value(
            console::program::ProgramID::from_str("vw_call_runtime_fail.aleo")?,
            console::program::Identifier::from_str("out")?,
            key,
            height,
        )?;
        assert!(out_value.is_none(), "out[caller_address] must remain unset after the rejected tx");
    }

    Ok(())
}

/// Behavioral: a finalize body whose only command is a `call` to a view. Verifies that the
/// minimal-body shape deploys and executes successfully — the cost-rollup path must accept a
/// single-call finalize body, and the view's output destinations may be bound without being
/// consumed further.
#[test]
fn test_finalize_call_as_only_finalize_instruction() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program vw_call_only.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function caller:
            input r0 as address.public;
            async caller r0 into r1;
            output r1 as vw_call_only.aleo/caller.future;

        finalize caller:
            input r0 as address.public;
            call lookup r0 into r1;

        view lookup:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    let inputs = [Value::from_str(&caller_address.to_string())?];
    let tx = vm.execute(&caller_private_key, ("vw_call_only.aleo", "caller"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    Ok(())
}

/// Behavioral: a finalize body calling a view that takes zero inputs. Confirms the empty-operand
/// path through `Call::output_types_for_view` (arity match against the zero-input view) and the
/// runtime operand-loading loop (which becomes a no-op).
#[test]
fn test_finalize_calls_zero_input_view() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program vw_call_zero.aleo;

        mapping out:
            key as address.public;
            value as u64.public;

        function caller:
            input r0 as address.public;
            async caller r0 into r1;
            output r1 as vw_call_zero.aleo/caller.future;

        finalize caller:
            input r0 as address.public;
            call answer into r1;
            set r1 into out[r0];

        view answer:
            add 40u64 2u64 into r0;
            output r0 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    let inputs = [Value::from_str(&caller_address.to_string())?];
    let tx = vm.execute(&caller_private_key, ("vw_call_zero.aleo", "caller"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    // Confirm out[caller] = 42 via the historic store.
    #[cfg(feature = "history")]
    {
        let height = vm.block_store().current_block_height();
        let key = console::program::Plaintext::from(console::program::Literal::Address(caller_address));
        let value = vm
            .finalize_store()
            .get_historical_mapping_value(
                console::program::ProgramID::from_str("vw_call_zero.aleo")?,
                console::program::Identifier::from_str("out")?,
                key,
                height,
            )?
            .expect("out should be set");
        match &*value {
            Value::Plaintext(console::program::Plaintext::Literal(console::program::Literal::U64(v), _)) => {
                assert_eq!(**v, 42u64, "zero-input view should return the constant 42");
            }
            other => panic!("expected u64 plaintext, got {other:?}"),
        }
    }

    Ok(())
}

/// Behavioral: a `call` to a view sitting inside a `branch.eq` skip range is skipped at runtime
/// when the branch is taken. Exercises the new finalize-Call dispatch from within the branching
/// control-flow already used by other finalize bodies.
#[test]
fn test_finalize_call_inside_branch() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    // When `r1 == 0u8`, the finalize takes the branch and skips both the call AND the `set`,
    // so `out[r0]` stays unset. When `r1 == 1u8`, the branch is NOT taken: the call runs and the
    // result is stored in `out[r0]`.
    let program = Program::from_str(
        r"
        program vw_call_branch.aleo;

        mapping seeds:
            key as address.public;
            value as u64.public;

        mapping out:
            key as address.public;
            value as u64.public;

        function seed:
            input r0 as address.public;
            input r1 as u64.public;
            async seed r0 r1 into r2;
            output r2 as vw_call_branch.aleo/seed.future;

        finalize seed:
            input r0 as address.public;
            input r1 as u64.public;
            set r1 into seeds[r0];

        function caller:
            input r0 as address.public;
            input r1 as u8.public;
            async caller r0 r1 into r2;
            output r2 as vw_call_branch.aleo/caller.future;

        finalize caller:
            input r0 as address.public;
            input r1 as u8.public;
            branch.eq r1 0u8 to skip;
            call lookup r0 into r2;
            set r2 into out[r0];
            position skip;

        view lookup:
            input r0 as address.public;
            get.or_use seeds[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    // Seed `seeds[caller] = 7`.
    let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("7u64")?];
    let tx = vm.execute(&caller_private_key, ("vw_call_branch.aleo", "seed"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    // Branch taken (r1 == 0u8): call is skipped, `out[r0]` stays unset.
    let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("0u8")?];
    let tx = vm.execute(&caller_private_key, ("vw_call_branch.aleo", "caller"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    #[cfg(feature = "history")]
    {
        let height = vm.block_store().current_block_height();
        let key = console::program::Plaintext::from(console::program::Literal::Address(caller_address));
        let value = vm.finalize_store().get_historical_mapping_value(
            console::program::ProgramID::from_str("vw_call_branch.aleo")?,
            console::program::Identifier::from_str("out")?,
            key.clone(),
            height,
        )?;
        assert!(value.is_none(), "out must remain unset when the call is skipped by branch.eq");

        // Branch NOT taken (r1 == 1u8): call runs and stores 7.
        let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("1u8")?];
        let tx =
            vm.execute(&caller_private_key, ("vw_call_branch.aleo", "caller"), inputs.iter(), None, 0, None, rng)?;
        add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

        let height = vm.block_store().current_block_height();
        let value = vm
            .finalize_store()
            .get_historical_mapping_value(
                console::program::ProgramID::from_str("vw_call_branch.aleo")?,
                console::program::Identifier::from_str("out")?,
                key,
                height,
            )?
            .expect("out should be set when the branch is not taken");
        match &*value {
            Value::Plaintext(console::program::Plaintext::Literal(console::program::Literal::U64(v), _)) => {
                assert_eq!(**v, 7u64, "out[caller] must equal the view's result when the call runs");
            }
            other => panic!("expected u64 plaintext, got {other:?}"),
        }
    }

    Ok(())
}

/// Behavioral: a single finalize body calls the same IMPORTED view twice (with different keys)
/// and combines the results. Counterpart to the same-program multi-call test
/// (`test_finalize_multiple_calls_and_interleaved_writes`) — exercises that locator-resolution
/// and cost rollup walk correctly on repeated cross-program calls within one finalize.
#[test]
fn test_finalize_cross_program_multiple_calls() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    // Data program: holds the `balances` mapping and the `lookup` view.
    let data_program = Program::from_str(
        r"
        program vw_cross_multi_data.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function seed:
            input r0 as address.public;
            input r1 as u64.public;
            async seed r0 r1 into r2;
            output r2 as vw_cross_multi_data.aleo/seed.future;

        finalize seed:
            input r0 as address.public;
            input r1 as u64.public;
            set r1 into balances[r0];

        view lookup:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            output r1 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )?;

    // Caller: imports the data program and calls `lookup` twice in one finalize body — once
    // for each key — and stores their sum.
    let caller_program = Program::from_str(
        r"
        import vw_cross_multi_data.aleo;

        program vw_cross_multi_caller.aleo;

        mapping totals:
            key as address.public;
            value as u64.public;

        function combine:
            input r0 as address.public;
            input r1 as address.public;
            async combine r0 r1 into r2;
            output r2 as vw_cross_multi_caller.aleo/combine.future;

        finalize combine:
            input r0 as address.public;
            input r1 as address.public;
            call vw_cross_multi_data.aleo/lookup r0 into r2;
            call vw_cross_multi_data.aleo/lookup r1 into r3;
            add r2 r3 into r4;
            set r4 into totals[r0];

        constructor:
            assert.eq true true;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    let tx_data = vm.deploy(&caller_private_key, &data_program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx_data], rng);
    let tx_caller = vm.deploy(&caller_private_key, &caller_program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx_caller], rng);

    // Sample a second address to use as the other key.
    let other_private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
    let other_address = Address::try_from(&other_private_key)?;

    // Seed `balances[caller] = 17` and `balances[other] = 25`.
    let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("17u64")?];
    let tx =
        vm.execute(&caller_private_key, ("vw_cross_multi_data.aleo", "seed"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);
    let inputs = [Value::from_str(&other_address.to_string())?, Value::from_str("25u64")?];
    let tx =
        vm.execute(&caller_private_key, ("vw_cross_multi_data.aleo", "seed"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    // Run `combine(caller, other)` — finalize calls `lookup` twice, sums (17 + 25 = 42), stores
    // into `totals[caller]`.
    let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str(&other_address.to_string())?];
    let tx =
        vm.execute(&caller_private_key, ("vw_cross_multi_caller.aleo", "combine"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    #[cfg(feature = "history")]
    {
        let height = vm.block_store().current_block_height();
        let key = console::program::Plaintext::from(console::program::Literal::Address(caller_address));
        let value = vm
            .finalize_store()
            .get_historical_mapping_value(
                console::program::ProgramID::from_str("vw_cross_multi_caller.aleo")?,
                console::program::Identifier::from_str("totals")?,
                key,
                height,
            )?
            .expect("totals should be set");
        match &*value {
            Value::Plaintext(console::program::Plaintext::Literal(console::program::Literal::U64(v), _)) => {
                assert_eq!(**v, 42u64, "totals[caller] should equal balances[caller] + balances[other]");
            }
            other => panic!("expected u64 plaintext, got {other:?}"),
        }
    }

    Ok(())
}

/// Guard-view lifecycle (zero-output view): a view that has no outputs serves as a precondition
/// check. Calling it from a finalize body with `call vw/require_zero r0;` (no `into`) is valid;
/// when the assertion in the view body holds, the caller's finalize continues; when it fails,
/// the entire transaction is finalize-rejected. This is the Aleo analogue of Solidity's
/// `function require_member(address) external view { require(...); }` pattern.
#[test]
fn test_finalize_call_zero_output_guard_view() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    // `require_zero` is a guard view: it asserts that its input is `0u64` and returns nothing.
    // The caller's finalize calls it, then writes a marker to `out[r0]` so that we can
    // distinguish the success path (write happens) from the failure path (tx rejected, no write).
    let program = Program::from_str(
        r"
        program vw_guard.aleo;

        mapping out:
            key as address.public;
            value as u64.public;

        function caller:
            input r0 as address.public;
            input r1 as u64.public;
            async caller r0 r1 into r2;
            output r2 as vw_guard.aleo/caller.future;

        finalize caller:
            input r0 as address.public;
            input r1 as u64.public;
            call require_zero r1;
            set 1u64 into out[r0];

        view require_zero:
            input r0 as u64.public;
            assert.eq r0 0u64;

        constructor:
            assert.eq true true;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    // Deploy succeeds — a zero-output view is now a valid program element.
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    // Happy path: pass `0u64`. The guard's `assert.eq` holds, the finalize body continues and
    // writes `out[caller] = 1`.
    let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("0u64")?];
    let tx = vm.execute(&caller_private_key, ("vw_guard.aleo", "caller"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);

    #[cfg(feature = "history")]
    {
        let height = vm.block_store().current_block_height();
        let key = console::program::Plaintext::from(console::program::Literal::Address(caller_address));
        let value = vm
            .finalize_store()
            .get_historical_mapping_value(
                console::program::ProgramID::from_str("vw_guard.aleo")?,
                console::program::Identifier::from_str("out")?,
                key.clone(),
                height,
            )?
            .expect("out should be set after the guard passes");
        match &*value {
            Value::Plaintext(console::program::Plaintext::Literal(console::program::Literal::U64(v), _)) => {
                assert_eq!(**v, 1u64, "guard pass: caller's finalize should have written 1");
            }
            other => panic!("expected u64 plaintext, got {other:?}"),
        }

        // Failure path: pass `1u64`. The guard's `assert.eq` fails, the finalize is rejected,
        // and `out[caller]` retains the value from the previous run (still `1u64`, not a new write).
        let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("1u64")?];
        let tx = vm.execute(&caller_private_key, ("vw_guard.aleo", "caller"), inputs.iter(), None, 0, None, rng)?;
        let block = sample_next_block(&vm, &caller_private_key, &[tx], rng)?;
        assert_eq!(block.transactions().num_accepted(), 0, "guard fail: tx must not be accepted");
        assert_eq!(block.transactions().num_rejected(), 1, "guard fail: tx must be finalize-rejected");
        assert_eq!(block.aborted_transaction_ids().len(), 0);
        vm.add_next_block(&block)?;

        // out[caller] is still 1 (the rejected tx didn't write 1 again — but, more importantly,
        // didn't apply any state change). We re-read to confirm the value is unchanged from the
        // happy-path write above.
        let height = vm.block_store().current_block_height();
        let value = vm
            .finalize_store()
            .get_historical_mapping_value(
                console::program::ProgramID::from_str("vw_guard.aleo")?,
                console::program::Identifier::from_str("out")?,
                key,
                height,
            )?
            .expect("out should still be set from the earlier successful run");
        match &*value {
            Value::Plaintext(console::program::Plaintext::Literal(console::program::Literal::U64(v), _)) => {
                assert_eq!(**v, 1u64, "rejected tx must not have mutated state");
            }
            other => panic!("expected u64 plaintext, got {other:?}"),
        }
    }

    Ok(())
}

/// Negative: a finalize-call that binds destinations to a zero-output guard view must be
/// rejected at deploy. The `view.outputs().len() != self.destinations.len()` check in
/// `Call::output_types_for_view` should bail (0 outputs vs. 1 destination).
#[test]
fn test_finalize_call_zero_output_view_with_destinations_rejected_at_deploy() {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    // The caller mistakenly binds `r1` for the guard view's (nonexistent) return value.
    let program = Program::from_str(
        r"
        program vw_guard_bad.aleo;

        function caller:
            input r0 as u64.public;
            async caller r0 into r1;
            output r1 as vw_guard_bad.aleo/caller.future;

        finalize caller:
            input r0 as u64.public;
            call require_zero r0 into r1;
            assert.eq r1 r1;

        view require_zero:
            input r0 as u64.public;
            assert.eq r0 0u64;

        constructor:
            assert.eq true true;
        ",
    );

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);
    if let Ok(program) = program {
        let deploy = vm.deploy(&caller_private_key, &program, None, 0, None, rng);
        assert!(deploy.is_err(), "deploy should reject binding destinations to a zero-output view");
    }
}

/// Three upgrades change a view body. The mapping value is held constant, so each height must resolve to the
/// edition live then — the middle case (height_v1 -> edition 1) checks that the scan picks the intermediate
/// edition, not just the newest or oldest.
#[cfg(feature = "history")]
#[test]
fn test_evaluate_view_uses_historic_program_edition() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    // `total_balance` returns `balances[addr] + bump`; only `bump` changes across editions.
    let program = |bump: u64| -> Result<Program<CurrentNetwork>> {
        Program::from_str(&format!(
            r"
        program vw_upgrade.aleo;

        mapping balances:
            key as address.public;
            value as u64.public;

        function increment:
            input r0 as address.public;
            input r1 as u64.public;
            async increment r0 r1 into r2;
            output r2 as vw_upgrade.aleo/increment.future;

        finalize increment:
            input r0 as address.public;
            input r1 as u64.public;
            get.or_use balances[r0] 0u64 into r2;
            add r2 r1 into r3;
            set r3 into balances[r0];

        view total_balance:
            input r0 as address.public;
            get.or_use balances[r0] 0u64 into r1;
            add r1 {bump}u64 into r2;
            output r2 as u64.public;

        constructor:
            assert.eq true true;
        "
        ))
    };

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    // Deploy edition 0, then set `balances[addr] = 5`.
    let tx = vm.deploy(&caller_private_key, &program(0)?, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);
    let inputs = [Value::from_str(&caller_address.to_string())?, Value::from_str("5u64")?];
    let tx = vm.execute(&caller_private_key, ("vw_upgrade.aleo", "increment"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, Some(&[&inputs]), &[tx], rng);
    let height_v0 = vm.block_store().current_block_height();

    // Upgrade to edition 1 (+100), then edition 2 (+1000).
    let tx = vm.deploy(&caller_private_key, &program(100)?, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);
    let height_v1 = vm.block_store().current_block_height();
    let tx = vm.deploy(&caller_private_key, &program(1000)?, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);
    let height_v2 = vm.block_store().current_block_height();

    let total = |height: u32| {
        vm.evaluate_view_at_height(
            "vw_upgrade.aleo",
            "total_balance",
            vec![Value::from_str(&caller_address.to_string()).unwrap()],
            height,
        )
    };
    assert_eq!(expect_u64(&total(height_v0)?), 5, "edition 0 body at its height");
    assert_eq!(expect_u64(&total(height_v1)?), 105, "edition 1 body at its height (intermediate)");
    assert_eq!(expect_u64(&total(height_v2)?), 1005, "edition 2 body at its height");

    Ok(())
}

/// Querying a view at a height before the program was deployed returns an error.
#[cfg(feature = "history")]
#[test]
fn test_evaluate_view_before_deployment_height_errors() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);

    let program = Program::from_str(
        r"
        program vw_predeploy.aleo;

        function noop:
            input r0 as u64.private;
            output r0 as u64.private;

        constructor:
            assert.eq true true;

        view fixed:
            add 0u64 7u64 into r0;
            output r0 as u64.public;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    // A valid block height that predates the program's deployment.
    let before = vm.block_store().current_block_height();
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, None, &[tx], rng);

    let err = vm.evaluate_view_at_height("vw_predeploy.aleo", "fixed", vec![], before).unwrap_err().to_string();
    assert!(err.contains("was not deployed at or before height"), "unexpected error: {err}");
    Ok(())
}
