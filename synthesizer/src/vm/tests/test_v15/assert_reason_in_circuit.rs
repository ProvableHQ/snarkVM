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

//! V15 prototype: `assert.eq/neq <a> <b> with <reason>;` inside transition function
//! bodies (circuit context).
//!
//! Failures here surface at `vm.execute(...)` because the assert violates a satisfied
//! constraint — the error chain bubbles the resolved reason plaintext through
//! `AssertError::EqWithReason { reason, .. }`. These tests:
//!
//! - confirm a successful execute proceeds normally,
//! - assert the error message contains the resolved reason on a failing execute,
//! - cover both eq and neq variants, and
//! - cover non-literal reasons via a struct (strings are not deployable post-V12).

use super::*;

fn assert_execute_fails_with_reason<E: std::fmt::Debug>(
    err: &E,
    expected_phrase: &str,
    expected_reason_fragment: &str,
) {
    let chain = format!("{err:?}");
    assert!(chain.contains(expected_phrase), "expected error to mention `{expected_phrase}`; full chain = {chain}");
    assert!(
        chain.contains(expected_reason_fragment),
        "expected error to embed reason fragment `{expected_reason_fragment}`; full chain = {chain}"
    );
}

#[test]
fn test_assert_reason_circuit_eq_passes_then_fails() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program circ_assert_eq.aleo;

        function gate:
            input r0 as u64.public;
            assert.eq r0 0u64 with 42u64;
            output r0 as u64.public;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    // Happy path: input 0u64 satisfies the assert.
    let inputs = [Value::<CurrentNetwork>::from_str("0u64")?];
    let tx = vm.execute(&caller_private_key, ("circ_assert_eq.aleo", "gate"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    // Failing path: input 1u64 violates `r0 == 0u64`; execute should error with the reason.
    let inputs = [Value::<CurrentNetwork>::from_str("1u64")?];
    let err = vm
        .execute(&caller_private_key, ("circ_assert_eq.aleo", "gate"), inputs.iter(), None, 0, None, rng)
        .expect_err("execute should fail for non-zero input");
    assert_execute_fails_with_reason(&err, "assert.eq", "42u64");

    Ok(())
}

#[test]
fn test_assert_reason_circuit_neq() -> Result<()> {
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program circ_assert_neq.aleo;

        function check:
            input r0 as u64.public;
            input r1 as u64.public;
            assert.neq r0 r1 with 99u64;
            output r0 as u64.public;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    // Happy path: distinct inputs satisfy the neq.
    let inputs = [Value::<CurrentNetwork>::from_str("1u64")?, Value::<CurrentNetwork>::from_str("2u64")?];
    let tx = vm.execute(&caller_private_key, ("circ_assert_neq.aleo", "check"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    // Failing path: equal inputs trigger the neq with reason.
    let inputs = [Value::<CurrentNetwork>::from_str("3u64")?, Value::<CurrentNetwork>::from_str("3u64")?];
    let err = vm
        .execute(&caller_private_key, ("circ_assert_neq.aleo", "check"), inputs.iter(), None, 0, None, rng)
        .expect_err("execute should fail when operands are equal");
    assert_execute_fails_with_reason(&err, "assert.neq", "99u64");

    Ok(())
}

#[test]
fn test_assert_reason_circuit_struct_reason() -> Result<()> {
    // Verify a struct reason plaintext appears in the failing execute's error chain.
    // (String reasons cannot be deployed post-V12, so they aren't exercised.)
    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    let program = Program::from_str(
        r"
        program circ_assert_complex.aleo;

        struct Reason:
            code as u8;
            amount as u64;

        function fail_with_struct:
            input r0 as u64.public;
            cast 5u8 r0 into r1 as Reason;
            assert.eq r0 0u64 with r1;
            output r0 as u64.public;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15)?, rng);
    let tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &caller_private_key, &caller_address, None, &[tx], rng);

    let inputs = [Value::<CurrentNetwork>::from_str("7u64")?];
    let err = vm
        .execute(
            &caller_private_key,
            ("circ_assert_complex.aleo", "fail_with_struct"),
            inputs.iter(),
            None,
            0,
            None,
            rng,
        )
        .expect_err("struct-reason execute should fail");
    let chain = format!("{err:?}");
    assert!(chain.contains("code: 5u8"), "struct reason `code: 5u8` should appear in error chain: {chain}");
    assert!(chain.contains("amount: 7u64"), "struct reason `amount: 7u64` should appear in error chain: {chain}");

    Ok(())
}
