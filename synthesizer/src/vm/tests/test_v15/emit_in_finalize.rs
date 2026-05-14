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

//! V15 prototype: `emit <operand>;` inside `finalize` bodies.
//!
//! Each successful finalize execution that runs `emit` should attach a
//! `FinalizeOperation::EmitEvent(plaintext)` to the confirmed transaction. These tests
//! deploy a program, drive an execute through speculation + block construction, then
//! inspect `block.transactions()` for the expected EmitEvent payload.

use super::*;

use snarkvm_ledger_block::ConfirmedTransaction;
use snarkvm_synthesizer_program::FinalizeOperation;

/// Collects every `EmitEvent` plaintext payload from a confirmed-transaction's
/// finalize operations.
fn collect_emit_events(confirmed: &ConfirmedTransaction<CurrentNetwork>) -> Vec<String> {
    confirmed
        .finalize_operations()
        .iter()
        .filter_map(|op| match op {
            FinalizeOperation::EmitEvent(plaintext) => Some(plaintext.to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn test_emit_in_finalize_literal() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program fin_emit_lit.aleo;

        function trigger:
            input r0 as u64.public;
            async trigger r0 into r1;
            output r1 as fin_emit_lit.aleo/trigger.future;

        finalize trigger:
            input r0 as u64.public;
            emit r0;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);

    // Deploy.
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    // Execute `trigger(123u64)` — this queues a finalize that emits the input.
    let inputs = [Value::<CurrentNetwork>::from_str("123u64")?];
    let tx = vm.execute(&caller_private_key, ("fin_emit_lit.aleo", "trigger"), inputs.iter(), None, 0, None, rng)?;
    let tx_id = tx.id();

    // Build the block ourselves so we can inspect confirmed-transaction finalize operations.
    let block = sample_next_block(&vm, &caller_private_key, &[tx], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);

    let confirmed = block.transactions().iter().find(|c| c.id() == tx_id).expect("confirmed tx for our execute");
    let events = collect_emit_events(confirmed);
    assert_eq!(events, vec!["123u64".to_string()], "expected exactly one `EmitEvent(123u64)`, got {events:?}");

    vm.add_next_block(&block)?;
    Ok(())
}

#[test]
fn test_emit_in_finalize_struct_plaintext() -> Result<()> {
    // A struct-shaped operand should be emitted by value — verify the resolved payload
    // matches the constructed struct.
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program fin_emit_struct.aleo;

        struct Event:
            kind as u8;
            amount as u64;

        function notify:
            input r0 as u8.public;
            input r1 as u64.public;
            async notify r0 r1 into r2;
            output r2 as fin_emit_struct.aleo/notify.future;

        finalize notify:
            input r0 as u8.public;
            input r1 as u64.public;
            cast r0 r1 into r2 as Event;
            emit r2;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    let inputs = [Value::<CurrentNetwork>::from_str("9u8")?, Value::<CurrentNetwork>::from_str("999u64")?];
    let tx = vm.execute(&caller_private_key, ("fin_emit_struct.aleo", "notify"), inputs.iter(), None, 0, None, rng)?;
    let tx_id = tx.id();
    let block = sample_next_block(&vm, &caller_private_key, &[tx], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);

    let confirmed = block.transactions().iter().find(|c| c.id() == tx_id).expect("confirmed tx");
    let events = collect_emit_events(confirmed);
    assert_eq!(events.len(), 1, "expected exactly one EmitEvent, got {events:?}");
    // Plaintext::Display for a struct prints `{ kind: 9u8, value: 999u64 }` (with leading newline).
    assert!(events[0].contains("kind: 9u8"), "event should include `kind: 9u8`, got {}", events[0]);
    assert!(events[0].contains("amount: 999u64"), "event should include `amount: 999u64`, got {}", events[0]);

    vm.add_next_block(&block)?;
    Ok(())
}

#[test]
fn test_emit_in_finalize_cross_program_ordering() -> Result<()> {
    // Two programs, A and B; A's finalize emits 1u8, then async-awaits B whose finalize
    // emits 2u8. After the run, the confirmed tx for A should carry exactly two
    // EmitEvent ops in the depth-first order A -> B.
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    // Deploy callee first.
    let callee = Program::from_str(
        r"
        program fin_emit_callee.aleo;

        function inner:
            async inner into r0;
            output r0 as fin_emit_callee.aleo/inner.future;

        finalize inner:
            emit 2u8;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &callee, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    // Deploy caller, which invokes callee.
    let caller_program = Program::from_str(
        r"
        import fin_emit_callee.aleo;
        program fin_emit_caller.aleo;

        function outer:
            call fin_emit_callee.aleo/inner into r0;
            async outer r0 into r1;
            output r1 as fin_emit_caller.aleo/outer.future;

        finalize outer:
            input r0 as fin_emit_callee.aleo/inner.future;
            emit 1u8;
            await r0;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;
    let tx = vm.deploy(&caller_private_key, &caller_program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    // Execute outer.
    let inputs: [Value<CurrentNetwork>; 0] = [];
    let tx = vm.execute(&caller_private_key, ("fin_emit_caller.aleo", "outer"), inputs.iter(), None, 0, None, rng)?;
    let tx_id = tx.id();
    let block = sample_next_block(&vm, &caller_private_key, &[tx], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);

    let confirmed = block.transactions().iter().find(|c| c.id() == tx_id).expect("confirmed tx");
    let events = collect_emit_events(confirmed);
    // Expected emit order matches finalize-execution traversal: A's `emit 1u8` runs
    // first, then `await` flushes B's finalize which runs `emit 2u8`.
    assert_eq!(
        events,
        vec!["1u8".to_string(), "2u8".to_string()],
        "cross-program emits must appear in caller-then-callee order; got {events:?}"
    );

    vm.add_next_block(&block)?;
    Ok(())
}
