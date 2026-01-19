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

// Tests that `call.dynamic` to closures fails at deployment time since closures cannot be called dynamically.
#[test]
fn test_dynamic_call_closure_forbidden() {
    let rng = &mut TestRng::default();

    let caller_private_key = sample_genesis_private_key(rng);

    // First create a program with a closure
    let target_program = Program::<CurrentNetwork>::from_str(
        r"
        program has_closure.aleo;

        closure add_numbers:
            input r0 as u64;
            input r1 as u64;
            add r0 r1 into r2;
            output r2 as u64;

        function use_closure:
            input r0 as u64.public;
            input r1 as u64.public;
            call add_numbers r0 r1 into r2;
            output r2 as u64.public;

        constructor:
            assert.eq true true;
        ",
    )
    .unwrap();

    let target_field = Identifier::<CurrentNetwork>::from_str("has_closure").unwrap().to_field().unwrap();
    let aleo_field = Identifier::<CurrentNetwork>::from_str("aleo").unwrap().to_field().unwrap();
    let closure_field = Identifier::<CurrentNetwork>::from_str("add_numbers").unwrap().to_field().unwrap();

    // Attempt to call the closure dynamically
    let caller_program_str = format!(
        r"
        program call_closure.aleo;

        function attempt_closure_call:
            input r0 as u64.public;
            input r1 as u64.public;
            call.dynamic {target_field} {aleo_field} {closure_field}
                with r0 r1 (as u64.public u64.public)
                into r2 (as u64.public);
            output r2 as u64.public;

        constructor:
            assert.eq true true;
        "
    );

    let caller_program = Program::<CurrentNetwork>::from_str(&caller_program_str).unwrap();

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap(), rng);

    // Deploy the target program with the closure
    let deploy_target = vm.deploy(&caller_private_key, &target_program, None, 0, None, rng).unwrap();
    add_and_test(&vm, &caller_private_key, &[deploy_target], rng);

    // Deployment should fail because closures cannot be called dynamically
    let deploy_result = vm.deploy(&caller_private_key, &caller_program, None, 0, None, rng);

    assert!(deploy_result.is_err(), "Deployment should fail for program calling a closure dynamically");
    let error_msg = deploy_result.unwrap_err().to_string();
    assert!(
        error_msg.contains("closure") || error_msg.contains("dynamically"),
        "Error should mention closure restriction, got: {error_msg}"
    );
}

// Tests that a read-only `async` block in a closures can be deployed and executed from ConsensusVersion::V14 onwards.
#[test]
fn test_async_in_closure() {
    let rng = &mut TestRng::default();

    let caller_private_key = sample_genesis_private_key(rng);

    let program = Program::<CurrentNetwork>::from_str(
        r"
        program async_in_closure.aleo;

        mapping results:
            key as field.public;
            value as field.public;

        closure foo:
            input r0 as field;
            async foo.aleo/foo r0 into r1;
            output r1 as foo.aleo/foo.future;

        async foo:
            input r0 as field;
            get results[0] into r1;

        function set_result:
            input r0 as field;
            input r1 as field;
            set r1 into results[r0];

        constructor:
            assert.eq true true;
        "
    ).unwrap();

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V13).unwrap(), rng);

    // Deploy the program
    let deploy_tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();

    let block = sample_next_block(&vm, &caller_private_key, &[deploy_tx], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 0);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.transactions().num_aborted(), 1);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    // Deploy the program at V14
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap(), rng);
    let deploy_tx = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
    add_and_test(&vm, &caller_private_key, &[deploy_tx], rng);

    // Execute `set_result`
    let execute_tx = vm.execute(&caller_private_key, ("async_in_closure.aleo", "set_result"), vec![Value::from_str("1field").unwrap(), Value::from_str("2field").unwrap()].into_iter(), None, 0, None, rng).unwrap();
    add_and_test(&vm, &caller_private_key, &[execute_tx], rng);

    // Execute `foo`
    let execute_tx = vm.execute(&caller_private_key, ("async_in_closure.aleo", "foo"), vec![Value::from_str("1field").unwrap()].into_iter(), None, 0, None, rng).unwrap();
    // Test that the future contains the correct value
    let future = execute_tx.outputs().get(0).unwrap().as_future().unwrap();
    let result = future.await(&vm, &caller_private_key, rng).unwrap();
    assert_eq!(result, Value::from_str("2field").unwrap());
    add_and_test(&vm, &caller_private_key, &[execute_tx], rng);

    // Check the result
    let result = vm.finalize_store().get_value_confirmed(ProgramID::from_str("async_in_closure.aleo").unwrap(), Identifier::from_str("results").unwrap(), &Plaintext::from_str("1field").unwrap()).unwrap().unwrap();
    assert_eq!(result, Plaintext::from_str("3field").unwrap());

}