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

use console::program::{DynamicRecord, InputID, compute_function_id};
use snarkvm_ledger_block::{Input, Transition};
use snarkvm_synthesizer_process::TranslationAssignment;

// Re-encodes the transition's root external-record input as ExternalRecordWithDynamicID.
fn replace_external_input(
    transition: &Transition<CurrentNetwork>,
    dynamic_id: Field<CurrentNetwork>,
) -> Transition<CurrentNetwork> {
    let mut inputs = transition.inputs().to_vec();
    let static_id = match inputs[0] {
        Input::ExternalRecord(id) | Input::ExternalRecordWithDynamicID(id, _) => id,
        ref other => panic!("expected external-record input, found {other}"),
    };
    inputs[0] = Input::ExternalRecordWithDynamicID(static_id, dynamic_id);
    Transition::new(
        *transition.program_id(),
        *transition.function_name(),
        inputs,
        transition.outputs().to_vec(),
        *transition.tpk(),
        *transition.tcm(),
        *transition.scm(),
    )
    .unwrap()
}

// Checks that a root call with an Input::ExternalRecordWithDynamicID is rejected.
#[test]
fn test_external_record_with_dynamic_id_input_to_root() {
    let consensus_version = ConsensusVersion::V18;

    let rng = &mut TestRng::default();
    let caller_private_key = sample_genesis_private_key(rng);
    let address = Address::try_from(&caller_private_key).unwrap();

    let program_a = Program::<CurrentNetwork>::from_str(
        r"
        program issuer.aleo;

        record ticket:
        owner as address.private;
        amount as u64.public;

        function foo:
            assert.eq true true;

        constructor:
        assert.eq true true;
        ",
    )
    .unwrap();

    let program_b = Program::<CurrentNetwork>::from_str(
        r"
        import issuer.aleo;

        program checker.aleo;

        function check_ticket:
            input r0 as issuer.aleo/ticket.record;
            input r1 as address.public;

            lt r0.amount 1000u64 into r2;
            assert.eq r2 true;

        constructor:
            assert.eq true true;
        ",
    )
    .unwrap();

    // Initialize the VM at V18 and deploy the two programs.
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(consensus_version).unwrap(), rng);
    let deploy_a = vm.deploy(&caller_private_key, &program_a, None, 0, None, rng).unwrap();
    add_and_test_with_costs(&vm, &caller_private_key, None, &[deploy_a], rng);
    let deploy_b = vm.deploy(&caller_private_key, &program_b, None, 0, None, rng).unwrap();
    add_and_test_with_costs(&vm, &caller_private_key, None, &[deploy_b], rng);

    // Build the forged execution with a standalone process. We cannot use `vm.execute` here because
    // the (fake) ticket record does not exist on the ledger, so the record-existence check would
    // reject the honest execution before we get a chance to forge it.
    let process = crate::Process::<CurrentNetwork>::load().unwrap();
    process.lock().add_program(&program_a).unwrap();
    process.lock().add_program(&program_b).unwrap();

    let amount = 42u64;
    let record = Record::<CurrentNetwork, Plaintext<CurrentNetwork>>::from_str(&format!(
        "{{ owner: {address}.private, amount: {amount}u64.public, _nonce: 0group.public, _version: 1u8.public }}"
    ))
    .unwrap();
    let record_value = Value::<CurrentNetwork>::Record(record.clone());
    let receiver_value = Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap();

    let function_name = Identifier::<CurrentNetwork>::from_str("check_ticket").unwrap();
    let authorization = process
        .authorize::<CurrentAleo, _>(
            &caller_private_key,
            program_b.id(),
            function_name,
            [record_value, receiver_value].iter(),
            rng,
        )
        .unwrap();

    let request = authorization.peek_next().unwrap();
    let tvk = *request.tvk();
    let function_id = compute_function_id(request.network_id(), request.program_id(), request.function_name()).unwrap();
    let id_static = match request.input_ids()[0] {
        InputID::ExternalRecord(id) => id,
        ref other => panic!("expected external-record request input id, found {other:?}"),
    };
    let dynamic_record = DynamicRecord::from_record(&record).unwrap();
    let dynamic_value = Value::DynamicRecord(dynamic_record.clone());
    let id_dynamic = match InputID::dynamic_record(function_id, &dynamic_value, tvk, 0).unwrap() {
        InputID::DynamicRecord(id) => id,
        _ => unreachable!(),
    };

    let (_response, trace) = process.execute::<CurrentAleo, _>(authorization, rng).unwrap();
    let mut transitions = trace.transitions().to_vec();
    let root_index = transitions.len() - 1;

    // Prove against the VM's current state root so the execution proof is otherwise well-formed.
    let global_state_root = vm.block_store().current_state_root();

    // Create the translation assignment.
    process
        .synthesize_translation_key::<CurrentAleo, _>(program_a.id(), &Identifier::from_str("ticket").unwrap(), rng)
        .unwrap();
    let translation_pk = process.get_proving_key(program_a.id(), Identifier::from_str("ticket").unwrap()).unwrap();
    let translation_assignment = TranslationAssignment::new(
        record,
        dynamic_record,
        *program_a.id(),
        function_id,
        Identifier::from_str("ticket").unwrap(),
        true,
        true,
        tvk,
        None,
        None,
        0,
        id_dynamic,
        id_static,
    );

    // Replace the Input::ExternalRecord by a malicious Input::ExternalRecordWithDynamicID.
    let forged_transition = replace_external_input(&transitions[root_index], id_dynamic);
    transitions[root_index] = forged_transition;

    let proving_tasks = trace.transition_tasks().values().cloned().collect::<Vec<_>>();
    let translation_assignments = vec![(translation_pk, vec![(translation_assignment, 0)])];
    let (_root, proof) = Trace::<CurrentNetwork>::prove_batch::<CurrentAleo, _>(
        "checker.aleo/check_ticket",
        VarunaVersion::V2,
        proving_tasks,
        &translation_assignments,
        &[],
        global_state_root,
        rng,
    )
    .unwrap();

    let forged_execution = Execution::from(transitions.iter().cloned(), global_state_root, Some(proof)).unwrap();

    // Compute the updated fee
    let execution_id = forged_execution.to_execution_id().unwrap();
    let (base_fee, _) = execution_cost(vm.process(), &forged_execution, consensus_version).unwrap();
    let fee_authorization = vm.authorize_fee_public(&caller_private_key, base_fee, 0, execution_id, rng).unwrap();
    let fee = vm.execute_fee_authorization(fee_authorization, None, rng).unwrap();
    let forged_transaction = Transaction::from_execution(forged_execution, Some(fee)).unwrap();

    // The VM must reject the forged transaction because a root (static) transition may not carry an
    // `ExternalRecordWithDynamicID` input.
    let error = vm.check_transaction(&forged_transaction, None, rng).unwrap_err();

    assert!(
        error.to_string().contains("Incorrect input variant")
            && error.to_string().contains("external_record_with_dynamic_id")
            && error.to_string().contains("issuer.aleo/ticket.record"),
    );
}
