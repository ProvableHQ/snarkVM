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

//! V15 prototype: `assert.eq/neq <a> <b> with <reason>;` inside `finalize` bodies.
//!
//! On finalize failure the VM records a transient `RejectionDiagnostics<N>` for the
//! rejected transaction id — accessible via `vm.pending_rejection_diagnostic(&tx_id)`.
//! These tests drive speculation, then assert the structured `resolved_reason`
//! captured for each rejected tx, exercising:
//!
//! - bare `assert.eq`/`assert.neq` (no reason → `resolved_reason == None`)
//! - eq + neq with a literal-plaintext reason
//! - struct plaintext reason (strings are not deployable post-V12)
//! - multi-tx batches in one speculation pass
//! - clearing between speculation passes
//! - cross-program calls where the rejection occurs in the callee.

use super::*;

use console::program::{Literal, Plaintext};
use snarkvm_synthesizer_program::FinalizeGlobalState;

/// Builds a `FinalizeGlobalState` for the next block, matching the consensus path.
fn next_finalize_state(vm: &VM<CurrentNetwork, LedgerType>) -> FinalizeGlobalState {
    let block_hash = vm.block_store().get_block_hash(vm.block_store().max_height().unwrap()).unwrap().unwrap();
    let latest_block = vm.block_store().get_block(&block_hash).unwrap().unwrap();
    let time_since_last_block = CurrentNetwork::BLOCK_TIME as i64;
    let next_block_height = latest_block.height() + 1;
    let next_block_timestamp = latest_block.timestamp().saturating_add(time_since_last_block);
    let next_timestamp = (next_block_height
        >= CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V12).unwrap_or_default())
    .then_some(next_block_timestamp);
    FinalizeGlobalState::from(next_block_height as u64, next_block_height, next_timestamp, [0u8; 32])
}

/// Speculates a set of transactions and returns the captured rejection diagnostics for
/// each tx id, in order. Resolves diagnostics directly through the VM — no block
/// construction needed.
fn speculate_and_collect_reasons(
    vm: &VM<CurrentNetwork, LedgerType>,
    transactions: &[Transaction<CurrentNetwork>],
    rng: &mut TestRng,
) -> Result<Vec<Option<Plaintext<CurrentNetwork>>>> {
    let finalize_state = next_finalize_state(vm);
    let time_since_last_block = CurrentNetwork::BLOCK_TIME as i64;
    let (_ratifications, _confirmed, aborted, _ratified) = vm.speculate(
        finalize_state,
        time_since_last_block,
        Some(0u64),
        vec![],
        &None.into(),
        transactions.iter(),
        rng,
    )?;
    assert!(aborted.is_empty(), "speculation should not abort any tx in these tests");

    Ok(transactions
        .iter()
        .map(|tx| vm.pending_rejection_diagnostic(&tx.id()).and_then(|d| d.resolved_reason))
        .collect())
}

#[test]
fn test_assert_reason_finalize_literal_reasons() -> Result<()> {
    // Three transitions:
    //   - `fail_eq`: assert.eq with reason → rejected, reason captured
    //   - `fail_neq`: assert.neq with reason → rejected, reason captured
    //   - `bare_fail`: bare assert.eq → rejected, no reason (resolved_reason = None)
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program fin_assert_lit.aleo;

        function fail_eq:
            input r0 as u64.public;
            async fail_eq r0 into r1;
            output r1 as fin_assert_lit.aleo/fail_eq.future;

        finalize fail_eq:
            input r0 as u64.public;
            assert.eq r0 0u64 with 7u64;

        function fail_neq:
            input r0 as u64.public;
            async fail_neq r0 into r1;
            output r1 as fin_assert_lit.aleo/fail_neq.future;

        finalize fail_neq:
            input r0 as u64.public;
            assert.neq r0 r0 with 13u64;

        function bare_fail:
            input r0 as u64.public;
            async bare_fail r0 into r1;
            output r1 as fin_assert_lit.aleo/bare_fail.future;

        finalize bare_fail:
            input r0 as u64.public;
            assert.eq r0 0u64;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    // Build all three failing tx's in a single speculation pass — verifies multi-tx
    // diagnostic capture (each entry keyed on its own tx id).
    let one = [Value::<CurrentNetwork>::from_str("1u64")?];
    let tx_eq = vm.execute(&caller_private_key, ("fin_assert_lit.aleo", "fail_eq"), one.iter(), None, 0, None, rng)?;
    let tx_neq =
        vm.execute(&caller_private_key, ("fin_assert_lit.aleo", "fail_neq"), one.iter(), None, 0, None, rng)?;
    let tx_bare =
        vm.execute(&caller_private_key, ("fin_assert_lit.aleo", "bare_fail"), one.iter(), None, 0, None, rng)?;

    let reasons = speculate_and_collect_reasons(&vm, &[tx_eq, tx_neq, tx_bare], rng)?;
    assert_eq!(reasons.len(), 3);
    // fail_eq → reason 7u64
    let Some(Plaintext::Literal(Literal::U64(v), _)) = reasons[0].as_ref() else {
        panic!("expected u64 plaintext reason for fail_eq, got {:?}", reasons[0]);
    };
    assert_eq!(**v, 7u64);
    // fail_neq → reason 13u64
    let Some(Plaintext::Literal(Literal::U64(v), _)) = reasons[1].as_ref() else {
        panic!("expected u64 plaintext reason for fail_neq, got {:?}", reasons[1]);
    };
    assert_eq!(**v, 13u64);
    // bare_fail → no reason
    assert!(reasons[2].is_none(), "bare assert.eq must produce no resolved_reason; got {:?}", reasons[2]);

    Ok(())
}

#[test]
fn test_assert_reason_finalize_struct_reason() -> Result<()> {
    // Verify that a struct reason plaintext is captured intact through the diagnostics
    // pipeline. (String reasons cannot be deployed post-V12, so they aren't exercised.)
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program fin_assert_complex.aleo;

        struct Reason:
            code as u8;
            amount as u64;

        function fail_with_struct:
            input r0 as u64.public;
            async fail_with_struct r0 into r1;
            output r1 as fin_assert_complex.aleo/fail_with_struct.future;

        finalize fail_with_struct:
            input r0 as u64.public;
            cast 9u8 r0 into r1 as Reason;
            assert.eq r0 0u64 with r1;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    let one = [Value::<CurrentNetwork>::from_str("1u64")?];
    let tx_struct = vm.execute(
        &caller_private_key,
        ("fin_assert_complex.aleo", "fail_with_struct"),
        one.iter(),
        None,
        0,
        None,
        rng,
    )?;

    let reasons = speculate_and_collect_reasons(&vm, &[tx_struct], rng)?;
    let struct_reason = reasons[0].as_ref().expect("struct reason should be captured");
    let struct_str = struct_reason.to_string();
    assert!(struct_str.contains("code: 9u8"), "struct reason should contain `code: 9u8`, got {struct_str}");
    assert!(struct_str.contains("amount: 1u64"), "struct reason should contain `amount: 1u64`, got {struct_str}");

    Ok(())
}

#[test]
fn test_assert_reason_finalize_clears_between_speculations() -> Result<()> {
    // A successful pass following a failing pass must clear stale diagnostics, so that
    // `pending_rejection_diagnostic` returns `None` for transactions that succeed.
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program fin_assert_clear.aleo;

        function go:
            input r0 as u64.public;
            async go r0 into r1;
            output r1 as fin_assert_clear.aleo/go.future;

        finalize go:
            input r0 as u64.public;
            assert.eq r0 0u64 with 42u64;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    // First pass: failing tx — reason 42 is captured.
    let one = [Value::<CurrentNetwork>::from_str("1u64")?];
    let tx_fail = vm.execute(&caller_private_key, ("fin_assert_clear.aleo", "go"), one.iter(), None, 0, None, rng)?;
    let tx_fail_id = tx_fail.id();
    let reasons = speculate_and_collect_reasons(&vm, &[tx_fail], rng)?;
    let Some(Plaintext::Literal(Literal::U64(v), _)) = reasons[0].as_ref() else {
        panic!("expected u64 reason in failing pass; got {:?}", reasons[0]);
    };
    assert_eq!(**v, 42u64);

    // Second pass: passing tx — speculation should clear the prior diagnostics map.
    let zero = [Value::<CurrentNetwork>::from_str("0u64")?];
    let tx_ok = vm.execute(&caller_private_key, ("fin_assert_clear.aleo", "go"), zero.iter(), None, 0, None, rng)?;
    let tx_ok_id = tx_ok.id();
    let reasons = speculate_and_collect_reasons(&vm, &[tx_ok], rng)?;
    assert!(reasons[0].is_none(), "successful tx must not surface a stale reason; got {:?}", reasons[0]);
    // The prior tx id must no longer resolve, either.
    assert!(
        vm.pending_rejection_diagnostic(&tx_fail_id).is_none(),
        "diagnostics for prior speculation must be cleared"
    );
    // And the successful tx id must not have a diagnostic entry of its own.
    assert!(vm.pending_rejection_diagnostic(&tx_ok_id).is_none());

    Ok(())
}

#[test]
fn test_assert_reason_finalize_cross_program() -> Result<()> {
    // Outer calls inner; the failing assert is in inner.finalize. The captured
    // diagnostics should still resolve the reason from the inner program back through
    // the chain, attributed to the outer tx id.
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let callee = Program::from_str(
        r"
        program fin_assert_callee.aleo;

        function gate:
            input r0 as u64.public;
            async gate r0 into r1;
            output r1 as fin_assert_callee.aleo/gate.future;

        finalize gate:
            input r0 as u64.public;
            assert.eq r0 0u64 with 99u64;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &callee, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    let caller_program = Program::from_str(
        r"
        import fin_assert_callee.aleo;
        program fin_assert_caller.aleo;

        function outer:
            input r0 as u64.public;
            call fin_assert_callee.aleo/gate r0 into r1;
            async outer r1 into r2;
            output r2 as fin_assert_caller.aleo/outer.future;

        finalize outer:
            input r0 as fin_assert_callee.aleo/gate.future;
            await r0;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;
    let tx = vm.deploy(&caller_private_key, &caller_program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    let inputs = [Value::<CurrentNetwork>::from_str("1u64")?];
    let tx = vm.execute(&caller_private_key, ("fin_assert_caller.aleo", "outer"), inputs.iter(), None, 0, None, rng)?;
    let reasons = speculate_and_collect_reasons(&vm, &[tx], rng)?;
    let Some(Plaintext::Literal(Literal::U64(v), _)) = reasons[0].as_ref() else {
        panic!("expected u64 reason flowed up from callee; got {:?}", reasons[0]);
    };
    assert_eq!(**v, 99u64);

    Ok(())
}
