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

//! V15 prototype: `emit <operand>;` inside transition function bodies (circuit context).
//!
//! The circuit-side `emit` is a debug-print primitive — zero constraints, never reaches
//! the verifier. Under `--features test`, the resolved plaintext is captured to a
//! per-thread buffer via `drain_recent_emits` so tests can assert what was emitted.

use super::*;

use snarkvm_synthesizer_program::drain_recent_emits;

#[test]
fn test_emit_in_circuit_function() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program circuit_emit_test.aleo;

        function check_emit:
            input r0 as u64.public;
            emit r0;
            output r0 as u64.public;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    // Drain anything captured during deploy so the assertions below are scoped to the
    // executes under test.
    let _drain = drain_recent_emits();

    // First execute: proof succeeds, captured emits all match the input value. The exact
    // number depends on how many times the prover/verifier path invokes `evaluate`
    // (authorization, finalize-cost estimation, etc.) — assert content, not count.
    let inputs = [Value::<CurrentNetwork>::from_str("42u64")?];
    let tx =
        vm.execute(&caller_private_key, ("circuit_emit_test.aleo", "check_emit"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);
    let emits = drain_recent_emits();
    assert!(!emits.is_empty(), "expected at least one emit, got none");
    assert!(emits.iter().all(|e| e == "42u64"), "all captured emits must equal `42u64`, got {emits:?}");

    // Second execute with a different value: capture reflects new input, no stickiness.
    let inputs = [Value::<CurrentNetwork>::from_str("7u64")?];
    let tx =
        vm.execute(&caller_private_key, ("circuit_emit_test.aleo", "check_emit"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);
    let emits = drain_recent_emits();
    assert!(!emits.is_empty(), "second execute should emit at least once");
    assert!(emits.iter().all(|e| e == "7u64"), "second execute should emit only `7u64`, got {emits:?}");

    Ok(())
}

#[test]
fn test_emit_in_circuit_struct_plaintext() -> Result<()> {
    // Verify a struct operand is captured by `Plaintext::Display`. (String literals are
    // rejected by the deploy validator post-V12, so we exercise a struct instead.)
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program circuit_emit_struct.aleo;

        struct Event:
            tag as u8;
            payload as u64;

        function emit_struct:
            input r0 as u64.public;
            cast 3u8 r0 into r1 as Event;
            emit r1;
            output r0 as u64.public;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    let _drain = drain_recent_emits();
    let inputs = [Value::<CurrentNetwork>::from_str("21u64")?];
    let tx = vm.execute(
        &caller_private_key,
        ("circuit_emit_struct.aleo", "emit_struct"),
        inputs.iter(),
        None,
        0,
        None,
        rng,
    )?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    let emits = drain_recent_emits();
    assert!(!emits.is_empty(), "expected at least one struct emit, got none");
    for e in &emits {
        assert!(e.contains("tag: 3u8"), "expected `tag: 3u8` in each emit; got {e}");
        assert!(e.contains("payload: 21u64"), "expected `payload: 21u64` in each emit; got {e}");
    }

    Ok(())
}
