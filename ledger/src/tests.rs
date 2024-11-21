// Copyright 2024 Aleo Network Foundation
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

use crate::{
    Ledger,
    RecordsFilter,
    advance::split_candidate_solutions,
    test_helpers::{CurrentAleo, CurrentLedger, CurrentNetwork},
};
use aleo_std::StorageMode;
use console::{
    account::{Address, PrivateKey},
    network::{MainnetV0, prelude::*},
    program::{Entry, Identifier, Literal, Plaintext, ProgramID, Value},
    types::U16,
};
use ledger_authority::Authority;
use ledger_block::{Block, ConfirmedTransaction, Execution, Ratify, Rejected, Transaction};
use ledger_committee::{Committee, MIN_VALIDATOR_STAKE};
use ledger_narwhal::{BatchCertificate, BatchHeader, Data, Subdag, Transmission, TransmissionID};
use ledger_store::{ConsensusStore, helpers::memory::ConsensusMemory};
use snarkvm_utilities::try_vm_runtime;
use synthesizer::{Stack, program::Program, vm::VM};

use indexmap::{IndexMap, IndexSet};
use rand::seq::SliceRandom;
use std::collections::{BTreeMap, HashMap};
use time::OffsetDateTime;

/// Initializes a sample VM.
fn sample_vm() -> VM<CurrentNetwork, ConsensusMemory<CurrentNetwork>> {
    VM::from(ConsensusStore::<CurrentNetwork, ConsensusMemory<CurrentNetwork>>::open(None).unwrap()).unwrap()
}

/// Extract the transmissions from a block.
fn extract_transmissions(
    block: &Block<CurrentNetwork>,
) -> IndexMap<TransmissionID<CurrentNetwork>, Transmission<CurrentNetwork>> {
    let mut transmissions = IndexMap::new();
    for tx in block.transactions().iter() {
        let checksum = Data::Object(tx.transaction().clone()).to_checksum::<CurrentNetwork>().unwrap();
        transmissions.insert(TransmissionID::from((&tx.id(), &checksum)), tx.transaction().clone().into());
    }
    if let Some(coinbase_solution) = block.solutions().as_ref() {
        for (_, solution) in coinbase_solution.iter() {
            let checksum = Data::Object(*solution).to_checksum::<CurrentNetwork>().unwrap();
            transmissions.insert(TransmissionID::from((solution.id(), checksum)), (*solution).into());
        }
    }
    transmissions
}

/// Construct `num_blocks` quorum blocks given a set of validator private keys and the genesis block.
fn construct_quorum_blocks(
    private_keys: Vec<PrivateKey<CurrentNetwork>>,
    genesis: Block<CurrentNetwork>,
    num_blocks: u64,
    rng: &mut TestRng,
) -> Vec<Block<CurrentNetwork>> {
    // Initialize the ledger with the genesis block.
    let ledger =
        Ledger::<CurrentNetwork, ConsensusMemory<CurrentNetwork>>::load(genesis.clone(), StorageMode::Production)
            .unwrap();

    // Initialize the round parameters.
    assert!(num_blocks > 0);
    assert!(num_blocks < 25);
    let rounds_per_commit = 2;
    let final_round = num_blocks.saturating_mul(rounds_per_commit);

    // Sample rounds of batch certificates starting at the genesis round from a static set of 4 authors.
    let (round_to_certificates_map, committee) = {
        let committee = ledger.latest_committee().unwrap();
        let mut round_to_certificates_map: HashMap<u64, IndexSet<BatchCertificate<CurrentNetwork>>> = HashMap::new();
        let mut previous_certificates: IndexSet<BatchCertificate<CurrentNetwork>> = IndexSet::with_capacity(4);

        // Create certificates for each round.
        for round in 1..=final_round {
            let mut current_certificates = IndexSet::new();
            let previous_certificate_ids =
                if round <= 1 { IndexSet::new() } else { previous_certificates.iter().map(|c| c.id()).collect() };

            for (i, private_key_1) in private_keys.iter().enumerate() {
                let batch_header = BatchHeader::new(
                    private_key_1,
                    round,
                    OffsetDateTime::now_utc().unix_timestamp(),
                    committee.id(),
                    Default::default(),
                    previous_certificate_ids.clone(),
                    rng,
                )
                .unwrap();
                // Add signatures for the batch headers. This creates a fully connected DAG.
                let signatures = private_keys
                    .iter()
                    .enumerate()
                    .filter(|&(j, _)| i != j)
                    .map(|(_, private_key_2)| private_key_2.sign(&[batch_header.batch_id()], rng).unwrap())
                    .collect();
                current_certificates.insert(BatchCertificate::from(batch_header, signatures).unwrap());
            }

            round_to_certificates_map.insert(round, current_certificates.clone());
            previous_certificates = current_certificates;
        }
        (round_to_certificates_map, committee)
    };

    // Helper function to create a quorum block.
    fn create_next_quorum_block(
        ledger: &Ledger<CurrentNetwork, ConsensusMemory<CurrentNetwork>>,
        round: u64,
        leader_certificate: &BatchCertificate<CurrentNetwork>,
        previous_leader_certificate: Option<&BatchCertificate<CurrentNetwork>>,
        round_to_certificates_map: &HashMap<u64, IndexSet<BatchCertificate<CurrentNetwork>>>,
        rng: &mut TestRng,
    ) -> Block<CurrentNetwork> {
        // Construct the subdag for the block.
        let mut subdag_map = BTreeMap::new();
        // Add the leader certificate.
        subdag_map.insert(round, [leader_certificate.clone()].into());
        // Add the certificates of the previous round.
        subdag_map.insert(round - 1, round_to_certificates_map.get(&(round - 1)).unwrap().clone());
        // Add the certificates from the previous leader round, excluding the previous leader certificate.
        // This assumes the number of rounds per commit is 2.
        if let Some(prev_leader_cert) = previous_leader_certificate {
            let mut previous_leader_round_certificates =
                round_to_certificates_map.get(&(round - 2)).cloned().unwrap_or_default();
            previous_leader_round_certificates.shift_remove(prev_leader_cert);
            subdag_map.insert(round - 2, previous_leader_round_certificates);
        }
        // Construct the block.
        let subdag = Subdag::from(subdag_map).unwrap();
        let block = ledger.prepare_advance_to_next_quorum_block(subdag, Default::default(), rng).unwrap();
        ledger.check_next_block(&block, rng).unwrap();
        block
    }

    // Track the blocks that are created.
    let mut blocks = Vec::new();
    let mut previous_leader_certificate: Option<&BatchCertificate<CurrentNetwork>> = None;

    // Construct the blocks.
    for block_height in 1..=num_blocks {
        let round = block_height.saturating_mul(rounds_per_commit);
        let leader = committee.get_leader(round).unwrap();
        let leader_certificate =
            round_to_certificates_map.get(&round).unwrap().iter().find(|c| c.author() == leader).unwrap();
        let block = create_next_quorum_block(
            &ledger,
            round,
            leader_certificate,
            previous_leader_certificate,
            &round_to_certificates_map,
            rng,
        );
        ledger.advance_to_next_block(&block).unwrap();
        previous_leader_certificate = Some(leader_certificate);
        blocks.push(block);
    }

    blocks
}

#[test]
fn test_load() {
    let rng = &mut TestRng::default();

    // Sample the genesis private key.
    let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    // Initialize the store.
    let store = ConsensusStore::<_, ConsensusMemory<_>>::open(None).unwrap();
    // Create a genesis block.
    let genesis = VM::from(store).unwrap().genesis_beacon(&private_key, rng).unwrap();

    // Initialize the ledger with the genesis block.
    let ledger = CurrentLedger::load(genesis.clone(), StorageMode::Production).unwrap();
    assert_eq!(ledger.latest_hash(), genesis.hash());
    assert_eq!(ledger.latest_height(), genesis.height());
    assert_eq!(ledger.latest_round(), genesis.round());
    assert_eq!(ledger.latest_block(), genesis);
}

#[test]
fn test_load_unchecked() {
    // Load the genesis block.
    let genesis = crate::test_helpers::sample_genesis_block();

    // Initialize the ledger without checks.
    let ledger = CurrentLedger::load_unchecked(genesis.clone(), StorageMode::Production).unwrap();
    assert_eq!(ledger.latest_hash(), genesis.hash());
    assert_eq!(ledger.latest_height(), genesis.height());
    assert_eq!(ledger.latest_round(), genesis.round());
    assert_eq!(ledger.latest_block(), genesis);

    // Initialize the ledger with the genesis block.
    let ledger = CurrentLedger::load(genesis.clone(), StorageMode::Production).unwrap();
    assert_eq!(ledger.latest_hash(), genesis.hash());
    assert_eq!(ledger.latest_height(), genesis.height());
    assert_eq!(ledger.latest_round(), genesis.round());
    assert_eq!(ledger.latest_block(), genesis);
}

#[test]
fn test_state_path() {
    let rng = &mut TestRng::default();

    // Initialize the ledger.
    let ledger = crate::test_helpers::sample_ledger(PrivateKey::<CurrentNetwork>::new(rng).unwrap(), rng);
    // Retrieve the genesis block.
    let block = ledger.get_block(0).unwrap();

    // Construct the state path.
    let commitments = block.transactions().commitments().collect::<Vec<_>>();
    let commitment = commitments[0];

    let _state_path = ledger.get_state_path_for_commitment(commitment).unwrap();
}

#[test]
fn test_insufficient_private_fees() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, view_key, address, .. } =
        crate::test_helpers::sample_test_env(rng);

    // A helper function to find records.
    let find_records = || {
        let microcredits = Identifier::from_str("microcredits").unwrap();
        ledger
            .find_records(&view_key, RecordsFilter::SlowUnspent(private_key))
            .unwrap()
            .filter(|(_, record)| match record.data().get(&microcredits) {
                Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => !amount.is_zero(),
                _ => false,
            })
            .collect::<indexmap::IndexMap<_, _>>()
    };

    // Fetch the unspent records.
    let records = find_records();
    let record_1 = records[0].clone();
    let record_2 = records[1].clone();

    // Check fee amount requirements for `split` calls.
    {
        // Prepare a `split` execution without a fee.
        let inputs = [Value::Record(record_1.clone()), Value::from_str("100u64").unwrap()];
        let authorization = ledger.vm.authorize(&private_key, "credits.aleo", "split", inputs, rng).unwrap();
        let split_transaction_without_fee = ledger.vm.execute_authorization(authorization, None, None, rng).unwrap();
        assert!(ledger.check_transaction_basic(&split_transaction_without_fee, None, rng).is_ok());
    }

    // Check fee amount requirements for executions.
    {
        // Prepare an execution without a fee.
        let inputs = [
            Value::Record(record_1),
            Value::from_str(&format!("{address}")).unwrap(),
            Value::from_str("100u64").unwrap(),
        ];
        let authorization = ledger.vm.authorize(&private_key, "credits.aleo", "transfer_private", inputs, rng).unwrap();
        let transaction_without_fee = ledger.vm.execute_authorization(authorization, None, None, rng).unwrap();
        let execution = transaction_without_fee.execution().unwrap();

        // Check that a transaction with sufficient fee will succeed.
        let fee_authorization = ledger
            .vm
            .authorize_fee_private(
                &private_key,
                record_2.clone(),
                10_000_000,
                1_000,
                execution.to_execution_id().unwrap(),
                rng,
            )
            .unwrap();
        let fee = ledger.vm.execute_fee_authorization(fee_authorization, None, rng).unwrap();
        let sufficient_fee_transaction = Transaction::from_execution(execution.clone(), Some(fee)).unwrap();
        assert!(ledger.check_transaction_basic(&sufficient_fee_transaction, None, rng).is_ok());

        // Check that a transaction with insufficient fee will fail.
        let insufficient_fee_authorization = ledger
            .vm
            .authorize_fee_private(&private_key, record_2.clone(), 1, 0, execution.to_execution_id().unwrap(), rng)
            .unwrap();
        let insufficient_fee = ledger.vm.execute_fee_authorization(insufficient_fee_authorization, None, rng).unwrap();
        let insufficient_fee_transaction =
            Transaction::from_execution(execution.clone(), Some(insufficient_fee)).unwrap();
        assert!(ledger.check_transaction_basic(&insufficient_fee_transaction, None, rng).is_err());
    }

    // Check fee amount requirements for deployment.

    {
        // Deploy a test program to the ledger.
        let program = Program::<CurrentNetwork>::from_str(
            r"
program dummy.aleo;
function foo:
    input r0 as u8.private;
    async foo r0 into r1;
    output r1 as dummy.aleo/foo.future;
finalize foo:
    input r0 as u8.public;
    add r0 r0 into r1;",
        )
        .unwrap();

        // Check that a deployment transaction with sufficient fee will succeed.
        let transaction = ledger.vm.deploy(&private_key, &program, Some(record_2.clone()), 0, None, rng).unwrap();
        assert!(ledger.check_transaction_basic(&transaction, None, rng).is_ok());

        // Check that a deployment transaction with insufficient fee will fail.
        let deployment = transaction.deployment().unwrap();
        let insufficient_fee_authorization = ledger
            .vm
            .authorize_fee_private(&private_key, record_2, 1, 0, deployment.to_deployment_id().unwrap(), rng)
            .unwrap();
        let insufficient_fee = ledger.vm.execute_fee_authorization(insufficient_fee_authorization, None, rng).unwrap();
        let insufficient_fee_transaction =
            Transaction::from_deployment(*transaction.owner().unwrap(), deployment.clone(), insufficient_fee).unwrap();
        assert!(ledger.check_transaction_basic(&insufficient_fee_transaction, None, rng).is_err());
    }
}

#[test]
fn test_insufficient_public_fees() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, .. } = crate::test_helpers::sample_test_env(rng);

    // Sample recipient.
    let recipient_private_key = PrivateKey::new(rng).unwrap();
    let recipient_address = Address::try_from(&recipient_private_key).unwrap();
    let withdrawal_private_key = PrivateKey::<MainnetV0>::new(rng).unwrap();
    let withdrawal_address = Address::try_from(&withdrawal_private_key).unwrap();

    // Fund the recipient with 1 million credits.
    {
        let inputs =
            [Value::from_str(&format!("{recipient_address}")).unwrap(), Value::from_str("1000000000000u64").unwrap()];
        let transaction = ledger
            .vm
            .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.into_iter(), None, 0, None, rng)
            .unwrap();

        let block =
            ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![transaction], rng).unwrap();

        // Check that the next block is valid.
        ledger.check_next_block(&block, rng).unwrap();
        // Add the deployment block to the ledger.
        ledger.advance_to_next_block(&block).unwrap();
    }

    // Attempt to bond the node with insufficient public fees.
    {
        let inputs = [
            Value::from_str(&format!("{withdrawal_address}")).unwrap(),
            Value::from_str("1000000000000u64").unwrap(),
            Value::from_str("10u8").unwrap(),
        ];
        let transaction = ledger
            .vm
            .execute(&recipient_private_key, ("credits.aleo", "bond_validator"), inputs.into_iter(), None, 0, None, rng)
            .unwrap();

        let block =
            ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![transaction], rng).unwrap();

        // Check that the next block is valid.
        ledger.check_next_block(&block, rng).unwrap();
        // Add the deployment block to the ledger.
        ledger.advance_to_next_block(&block).unwrap();
    }
}

#[test]
fn test_insufficient_finalize_fees() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, view_key, address, .. } =
        crate::test_helpers::sample_test_env(rng);

    // Deploy a test program to the ledger.
    let program = Program::<CurrentNetwork>::from_str(
        r"
program dummy.aleo;
function foo:
    input r0 as u8.private;
    async foo r0 into r1;
    output r1 as dummy.aleo/foo.future;
finalize foo:
    input r0 as u8.public;
    add r0 r0 into r1;",
    )
    .unwrap();

    // A helper function to find records.
    let find_records = || {
        let microcredits = Identifier::from_str("microcredits").unwrap();
        ledger
            .find_records(&view_key, RecordsFilter::SlowUnspent(private_key))
            .unwrap()
            .filter(|(_, record)| match record.data().get(&microcredits) {
                Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => !amount.is_zero(),
                _ => false,
            })
            .collect::<indexmap::IndexMap<_, _>>()
    };

    // Fetch the unspent records.
    let records = find_records();
    // Prepare the additional fee.
    let credits = Some(records.values().next().unwrap().clone());

    // Deploy.
    let transaction = ledger.vm.deploy(&private_key, &program, credits, 0, None, rng).unwrap();
    // Verify.
    ledger.vm().check_transaction(&transaction, None, rng).unwrap();

    // Construct the next block.
    let block =
        ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![transaction], rng).unwrap();
    // Advance to the next block.
    ledger.advance_to_next_block(&block).unwrap();
    assert_eq!(ledger.latest_height(), 1);
    assert_eq!(ledger.latest_hash(), block.hash());

    // Create a transfer transaction to produce a record with insufficient balance to pay for fees.
    let transfer_transaction = ledger.create_transfer(&private_key, address, 100, 0, None, rng).unwrap();

    // Construct the next block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![transfer_transaction.clone()], rng)
        .unwrap();
    // Advance to the next block.
    ledger.advance_to_next_block(&block).unwrap();
    assert_eq!(ledger.latest_height(), 2);
    assert_eq!(ledger.latest_hash(), block.hash());

    // Execute the test program, without providing enough fees for finalize, and ensure that the ledger deems the transaction invalid.

    // Find records from the transfer transaction.
    let records = transfer_transaction
        .records()
        .map(|(_, record)| record.decrypt(&view_key))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Prepare the inputs.
    let inputs = [Value::<CurrentNetwork>::from_str("1u8").unwrap()].into_iter();

    // Check that the record has the correct balance.
    let insufficient_record = records[0].clone();
    if let Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) =
        &insufficient_record.data().get(&Identifier::from_str("microcredits").unwrap())
    {
        assert_eq!(**amount, 100)
    }
    // Ensure that we can't produce a transaction with a record that has insufficient balance to pay for fees.
    assert!(
        ledger
            .vm
            .execute(&private_key, ("dummy.aleo", "foo"), inputs.clone(), Some(insufficient_record), 0, None, rng)
            .is_err()
    );

    let sufficient_record = records[1].clone();
    // Execute with enough fees.
    let transaction =
        ledger.vm.execute(&private_key, ("dummy.aleo", "foo"), inputs, Some(sufficient_record), 0, None, rng).unwrap();
    // Verify.
    ledger.vm.check_transaction(&transaction, None, rng).unwrap();
    // Ensure that the ledger deems the transaction valid.
    assert!(ledger.check_transaction_basic(&transaction, None, rng).is_ok());
}

#[test]
fn test_rejected_execution() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, view_key, .. } = crate::test_helpers::sample_test_env(rng);

    // Deploy a test program to the ledger.
    let program_id = "test_rejected_execute.aleo";
    let program = Program::<CurrentNetwork>::from_str(&format!(
        "
program {program_id};

function failed_assert:
    async failed_assert into r0;
    output r0 as {program_id}/failed_assert.future;

finalize failed_assert:
    assert.eq false true;"
    ))
    .unwrap();

    // A helper function to find records.
    let find_records = || {
        let microcredits = Identifier::from_str("microcredits").unwrap();
        ledger
            .find_records(&view_key, RecordsFilter::SlowUnspent(private_key))
            .unwrap()
            .filter(|(_, record)| match record.data().get(&microcredits) {
                Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => !amount.is_zero(),
                _ => false,
            })
            .collect::<indexmap::IndexMap<_, _>>()
    };

    // Fetch the unspent records.
    let records = find_records();
    let record_1 = records[0].clone();
    let record_2 = records[1].clone();

    // Deploy the program.
    let deployment_transaction = ledger.vm().deploy(&private_key, &program, Some(record_1), 0, None, rng).unwrap();

    // Construct the deployment block.
    let deployment_block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![deployment_transaction], rng)
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&deployment_block, rng).unwrap();

    // Add the deployment block to the ledger.
    ledger.advance_to_next_block(&deployment_block).unwrap();

    // Construct a transaction that will cause error from an assert call in `finalize`.
    let failed_assert_transaction = ledger
        .vm()
        .execute(
            &private_key,
            (program_id, "failed_assert"),
            Vec::<Value<_>>::new().into_iter(),
            Some(record_2),
            0,
            None,
            rng,
        )
        .unwrap();
    let failed_assert_transaction_id = failed_assert_transaction.id();

    // Construct the next block containing the new transaction.
    let next_block = ledger
        .prepare_advance_to_next_beacon_block(
            &private_key,
            vec![],
            vec![],
            vec![failed_assert_transaction.clone()],
            rng,
        )
        .unwrap();

    // Check that the block contains 1 rejected execution.
    assert_eq!(next_block.transactions().len(), 1);
    let confirmed_transaction = next_block.transactions().iter().next().unwrap();
    assert!(confirmed_transaction.is_rejected());
    if let Transaction::Execute(_, execution, fee) = failed_assert_transaction {
        let fee_transaction = Transaction::from_fee(fee.unwrap()).unwrap();
        let expected_confirmed_transaction =
            ConfirmedTransaction::RejectedExecute(0, fee_transaction, Rejected::new_execution(execution), vec![]);

        assert_eq!(confirmed_transaction, &expected_confirmed_transaction);
    }

    // Check that the unconfirmed transaction ID of the rejected execution is correct.
    assert_eq!(confirmed_transaction.to_unconfirmed_transaction_id().unwrap(), failed_assert_transaction_id);

    // Check that the next block is valid.
    ledger.check_next_block(&next_block, rng).unwrap();

    // Add the block with the rejected transaction to the ledger.
    ledger.advance_to_next_block(&next_block).unwrap();
}

#[test]
fn test_deploy_with_public_fees() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, .. } = crate::test_helpers::sample_test_env(rng);

    // Deploy a test program to the ledger.
    let program_id = ProgramID::<CurrentNetwork>::from_str("dummy_program.aleo").unwrap();
    let program = Program::<CurrentNetwork>::from_str(&format!(
        "
program {program_id};
function foo:
    input r0 as u8.private;
    async foo r0 into r1;
    output r1 as {program_id}/foo.future;
finalize foo:
    input r0 as u8.public;
    add r0 r0 into r1;",
    ))
    .unwrap();

    // Deploy.
    let transaction = ledger.vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
    // Verify.
    ledger.vm().check_transaction(&transaction, None, rng).unwrap();

    // Construct the next block.
    let block =
        ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![transaction], rng).unwrap();
    // Advance to the next block.
    ledger.advance_to_next_block(&block).unwrap();
    assert_eq!(ledger.latest_height(), 1);
    assert_eq!(ledger.latest_hash(), block.hash());

    assert_eq!(program, ledger.get_program(program_id).unwrap())
}

#[test]
fn test_bond_and_unbond_validator() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, .. } = crate::test_helpers::sample_test_env(rng);

    // Sample new account for the new committee member.
    let new_member_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let new_member_address = Address::try_from(&new_member_private_key).unwrap();
    let new_member_withdrawal_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let new_member_withdrawal_address = Address::try_from(&new_member_withdrawal_private_key).unwrap();

    // Fund the new committee member and their withdrawwal address.
    let inputs = [
        Value::from_str(&format!("{new_member_address}")).unwrap(),
        Value::from_str("20000000000000u64").unwrap(), // 20 million credits.
    ];
    let transfer_transaction = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
        .unwrap();
    let inputs = [
        Value::from_str(&format!("{new_member_withdrawal_address}")).unwrap(),
        Value::from_str("20000000u64").unwrap(), // 20 credits.
    ];
    let transfer_to_withdrawal_transaction = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
        .unwrap();

    // Construct the next block.
    let transfer_block = ledger
        .prepare_advance_to_next_beacon_block(
            &private_key,
            vec![],
            vec![],
            vec![transfer_transaction, transfer_to_withdrawal_transaction],
            rng,
        )
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&transfer_block, rng).unwrap();

    // Add the deployment block to the ledger.
    ledger.advance_to_next_block(&transfer_block).unwrap();

    // Construct the bond public
    let bond_amount = MIN_VALIDATOR_STAKE;
    let commission = 10u8;
    let inputs = [
        Value::from_str(&format!("{new_member_withdrawal_address}")).unwrap(),
        Value::from_str(&format!("{bond_amount}u64")).unwrap(),
        Value::from_str(&format!("{commission}u8")).unwrap(),
    ];
    let bond_validator_transaction = ledger
        .vm
        .execute(&new_member_private_key, ("credits.aleo", "bond_validator"), inputs.iter(), None, 0, None, rng)
        .unwrap();

    // Construct the next block.
    let bond_validator_block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![bond_validator_transaction], rng)
        .unwrap();

    // Check that the committee does not include the new member.
    let committee = ledger.latest_committee().unwrap();
    assert!(!committee.is_committee_member(new_member_address));

    // Check that the next block is valid.
    ledger.check_next_block(&bond_validator_block, rng).unwrap();

    // Add the bond public block to the ledger.
    ledger.advance_to_next_block(&bond_validator_block).unwrap();

    // Check that the committee is updated with the new member.
    let committee = ledger.latest_committee().unwrap();
    assert!(committee.is_committee_member(new_member_address));

    // Check that number of validators in the `metadata` mapping in `credtis.aleo` is updated.
    let program_id = ProgramID::<CurrentNetwork>::from_str("credits.aleo").unwrap();
    let metadata_mapping_name = Identifier::from_str("metadata").unwrap();
    let key = Plaintext::<CurrentNetwork>::from_str("aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc")
        .unwrap();
    let num_validators = match ledger
        .vm()
        .finalize_store()
        .get_value_confirmed(program_id, metadata_mapping_name, &key)
        .unwrap()
        .unwrap()
    {
        Value::Plaintext(Plaintext::Literal(Literal::U32(num_validators), _)) => *num_validators as usize,
        _ => panic!("Unexpected value type"),
    };
    assert_eq!(num_validators, committee.num_members());

    // Construct the unbond public
    let unbond_amount = committee.get_stake(new_member_address);
    let inputs = [
        Value::from_str(&format!("{new_member_address}")).unwrap(),
        Value::from_str(&format!("{unbond_amount}u64")).unwrap(),
    ];
    let unbond_public_transaction = ledger
        .vm
        .execute(
            &new_member_withdrawal_private_key,
            ("credits.aleo", "unbond_public"),
            inputs.iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();

    // Construct the next block.
    let unbond_public_block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![unbond_public_transaction], rng)
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&unbond_public_block, rng).unwrap();

    // Add the bond public block to the ledger.
    ledger.advance_to_next_block(&unbond_public_block).unwrap();

    // Check that the committee does not include the new member.
    let committee = ledger.latest_committee().unwrap();
    assert!(!committee.is_committee_member(new_member_address));

    // Check that number of validators in the `metadata` mapping in `credtis.aleo` is updated.
    let program_id = ProgramID::<CurrentNetwork>::from_str("credits.aleo").unwrap();
    let metadata_mapping_name = Identifier::from_str("metadata").unwrap();
    let key = Plaintext::<CurrentNetwork>::from_str("aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc")
        .unwrap();
    let num_validators = match ledger
        .vm()
        .finalize_store()
        .get_value_confirmed(program_id, metadata_mapping_name, &key)
        .unwrap()
        .unwrap()
    {
        Value::Plaintext(Plaintext::Literal(Literal::U32(num_validators), _)) => *num_validators as usize,
        _ => panic!("Unexpected value type"),
    };
    assert_eq!(num_validators, committee.num_members());
}

#[test]
fn test_aborted_transaction_indexing() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, .. } = crate::test_helpers::sample_test_env(rng);

    // Sample a recipient account.
    let recipient_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let recipient_address = Address::try_from(&recipient_private_key).unwrap();

    // Sample another recipient account.
    let recipient_private_key_2 = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let recipient_address_2 = Address::try_from(&recipient_private_key_2).unwrap();

    // Fund a new address.
    let inputs = [Value::from_str(&format!("{recipient_address}")).unwrap(), Value::from_str("185000u64").unwrap()];
    let transfer_transaction = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
        .unwrap();

    // Construct the next block.
    let transfer_block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![transfer_transaction], rng)
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&transfer_block, rng).unwrap();

    // Add the deployment block to the ledger.
    ledger.advance_to_next_block(&transfer_block).unwrap();

    // Send a transaction that will be aborted due to insufficient fee.
    let inputs = [Value::from_str(&format!("{recipient_address_2}")).unwrap(), Value::from_str("1u64").unwrap()];
    let transfer_transaction = ledger
        .vm
        .execute(&recipient_private_key_2, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
        .unwrap();
    let aborted_transaction_id = transfer_transaction.id();

    // Create another arbitrary transaction.
    let inputs = [Value::from_str(&format!("{recipient_address_2}")).unwrap(), Value::from_str("1u64").unwrap()];
    let transfer_transaction_2 = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
        .unwrap();

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(
            &private_key,
            vec![],
            vec![],
            vec![transfer_transaction, transfer_transaction_2],
            rng,
        )
        .unwrap();

    // Check that the block contains the aborted transaction.
    assert_eq!(block.aborted_transaction_ids(), &[aborted_transaction_id]);

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the deployment block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();
}

#[test]
fn test_aborted_solution_ids() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, address, .. } = crate::test_helpers::sample_test_env(rng);

    // Retrieve the puzzle parameters.
    let puzzle = ledger.puzzle();
    let latest_epoch_hash = ledger.latest_epoch_hash().unwrap();
    let minimum_proof_target = ledger.latest_proof_target();

    // Create a solution that is less than the minimum proof target.
    let mut invalid_solution = puzzle.prove(latest_epoch_hash, address, rng.gen(), None).unwrap();
    while puzzle.get_proof_target(&invalid_solution).unwrap() >= minimum_proof_target {
        invalid_solution = puzzle.prove(latest_epoch_hash, address, rng.gen(), None).unwrap();
    }

    // Create a valid transaction for the block.
    let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("10u64").unwrap()];
    let transfer_transaction = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
        .unwrap();

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(
            &private_key,
            vec![],
            vec![invalid_solution],
            vec![transfer_transaction],
            rng,
        )
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the deployment block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Enforce that the block solution was aborted properly.
    assert!(block.solutions().is_empty());
    assert_eq!(block.aborted_solution_ids(), &vec![invalid_solution.id()]);
}

#[test]
fn test_execute_duplicate_input_ids() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, view_key, address, .. } =
        crate::test_helpers::sample_test_env(rng);

    // A helper function to find records.
    let find_records = || {
        let microcredits = Identifier::from_str("microcredits").unwrap();
        ledger
            .find_records(&view_key, RecordsFilter::SlowUnspent(private_key))
            .unwrap()
            .filter(|(_, record)| match record.data().get(&microcredits) {
                Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => !amount.is_zero(),
                _ => false,
            })
            .collect::<indexmap::IndexMap<_, _>>()
    };

    // Fetch the unspent records.
    let records = find_records();
    let record_execution = records[0].clone();
    let record_deployment = records[1].clone();

    // Prepare a transfer that spends a record.
    let inputs = [
        Value::Record(record_execution.clone()),
        Value::from_str(&format!("{address}")).unwrap(),
        Value::from_str("100u64").unwrap(),
    ];

    let num_duplicate_deployments = 3;
    let mut executions = Vec::with_capacity(num_duplicate_deployments + 1);
    let mut execution_ids = Vec::with_capacity(num_duplicate_deployments + 1);
    let mut deployments = Vec::with_capacity(num_duplicate_deployments);
    let mut deployment_ids = Vec::with_capacity(num_duplicate_deployments);

    // Create Executions and Deployments, spending the same record.
    for i in 0..num_duplicate_deployments {
        // Execute.
        let execution = ledger
            .vm
            .execute(&private_key, ("credits.aleo", "transfer_private"), inputs.clone().iter(), None, 0, None, rng)
            .unwrap();
        execution_ids.push(execution.id());
        executions.push(execution);
        // Deploy.
        let program_id = ProgramID::<CurrentNetwork>::from_str(&format!("dummy_program_{i}.aleo")).unwrap();
        let program = Program::<CurrentNetwork>::from_str(&format!(
            "
program {program_id};
function foo:
    input r0 as u8.private;
    async foo r0 into r1;
    output r1 as {program_id}/foo.future;
finalize foo:
    input r0 as u8.public;
    add r0 r0 into r1;",
        ))
        .unwrap();
        let deployment =
            ledger.vm.deploy(&private_key, &program, Some(record_deployment.clone()), 0, None, rng).unwrap();
        deployment_ids.push(deployment.id());
        deployments.push(deployment);
    }

    // Create one more execution which spends the record as a fee.
    let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("100u64").unwrap()];
    let execution = ledger
        .vm
        .execute(
            &private_key,
            ("credits.aleo", "transfer_public"),
            inputs.clone().iter(),
            Some(record_execution.clone()),
            0,
            None,
            rng,
        )
        .unwrap();
    execution_ids.push(execution.id());
    executions.push(execution);

    // Select a transaction to mutate by a malicious validator.
    let transaction_to_mutate = executions.last().unwrap().clone();

    // Create a mutated execution which adds one transition, resulting in a different transaction id.
    // This simulates a malicious validator re-using execution content.
    let execution_to_mutate = transaction_to_mutate.execution().unwrap();
    // Sample a transition.
    let sample = ledger_test_helpers::sample_transition(rng);
    // Extend the transitions.
    let mutated_transitions = std::iter::once(sample).chain(execution_to_mutate.transitions().cloned());
    // Create a mutated execution.
    let mutated_execution = Execution::from(
        mutated_transitions,
        execution_to_mutate.global_state_root(),
        execution_to_mutate.proof().cloned(),
    )
    .unwrap();
    // Create a new fee for the execution.
    let fee_authorization = ledger
        .vm
        .authorize_fee_public(
            &private_key,
            *executions.last().unwrap().fee_amount().unwrap(),
            0,
            mutated_execution.to_execution_id().unwrap(),
            rng,
        )
        .unwrap();
    let fee = ledger.vm.execute_fee_authorization(fee_authorization, None, rng).unwrap();
    // Create a mutated transaction.
    let mutated_transaction = Transaction::from_execution(mutated_execution, Some(fee)).unwrap();
    execution_ids.push(mutated_transaction.id());
    executions.push(mutated_transaction);

    // Create a mutated execution which just takes the fee transition, resulting in a different transaction id.
    // This simulates a malicious validator transforming a transaction to a fee transaction.
    let mutated_transaction = Transaction::from_fee(transaction_to_mutate.fee_transition().unwrap()).unwrap();
    execution_ids.push(mutated_transaction.id());
    executions.push(mutated_transaction);

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(
            &private_key,
            vec![],
            vec![],
            vec![
                executions.pop().unwrap(),
                executions.pop().unwrap(),
                executions.pop().unwrap(),
                executions.pop().unwrap(),
                executions.pop().unwrap(),
                deployments.pop().unwrap(),
                deployments.pop().unwrap(),
            ],
            rng,
        )
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Enforce that the block transactions were correct.
    assert_eq!(block.transactions().num_accepted(), 2);
    println!("execution_ids: {:?}", execution_ids);
    assert_eq!(block.transactions().transaction_ids().collect::<Vec<_>>(), vec![&execution_ids[2], &deployment_ids[2]]);
    assert_eq!(block.aborted_transaction_ids(), &vec![
        execution_ids[5],
        execution_ids[4],
        execution_ids[3],
        execution_ids[1],
        deployment_ids[1]
    ]);

    // Ensure that verification was not run on aborted deployments.
    let partially_verified_transaction = ledger.vm().partially_verified_transactions().read().clone();

    assert!(partially_verified_transaction.contains(&execution_ids[2]));
    assert!(partially_verified_transaction.contains(&deployment_ids[2]));
    assert!(!partially_verified_transaction.contains(&execution_ids[1]));
    assert!(!partially_verified_transaction.contains(&deployment_ids[1]));
    assert!(!partially_verified_transaction.contains(&execution_ids[3]));
    assert!(!partially_verified_transaction.contains(&execution_ids[4])); // Verification was run, but the execution was invalid.
    assert!(!partially_verified_transaction.contains(&execution_ids[5]));

    // Prepare a transfer that will succeed for the subsequent block.
    let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("1000u64").unwrap()];
    let transfer = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.into_iter(), None, 0, None, rng)
        .unwrap();
    let transfer_id = transfer.id();

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(
            &private_key,
            vec![],
            vec![],
            vec![executions.pop().unwrap(), deployments.pop().unwrap(), transfer],
            rng,
        )
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Enforce that the block transactions were correct.
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().transaction_ids().collect::<Vec<_>>(), vec![&transfer_id]);
    assert_eq!(block.aborted_transaction_ids(), &vec![execution_ids[0], deployment_ids[0]]);

    // Ensure that verification was not run on transactions aborted in a previous block.
    let partially_verified_transaction = ledger.vm().partially_verified_transactions().read().clone();
    assert!(partially_verified_transaction.contains(&transfer_id));
    assert!(!partially_verified_transaction.contains(&execution_ids[0]));
    assert!(!partially_verified_transaction.contains(&deployment_ids[0]));
}

#[test]
fn test_execute_duplicate_output_ids() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, view_key, address, .. } =
        crate::test_helpers::sample_test_env(rng);

    // Deploy a test program to the ledger.
    let program = Program::<CurrentNetwork>::from_str(
        "
program dummy_program.aleo;

record dummy_program:
    owner as address.private;
    rand_var as u64.private;

function create_duplicate_record:
    input r0 as u64.private;
    cast self.caller 1u64 into r1 as dummy_program.record;
    output r1 as dummy_program.record;",
    )
    .unwrap();

    // Deploy.
    let deployment_transaction = ledger.vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
    // Verify.
    ledger.vm().check_transaction(&deployment_transaction, None, rng).unwrap();

    // Construct the next block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![deployment_transaction], rng)
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();
    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // A helper function to find records.
    let find_records = || {
        let microcredits = Identifier::from_str("microcredits").unwrap();
        ledger
            .find_records(&view_key, RecordsFilter::SlowUnspent(private_key))
            .unwrap()
            .filter(|(_, record)| match record.data().get(&microcredits) {
                Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => !amount.is_zero(),
                _ => false,
            })
            .collect::<indexmap::IndexMap<_, _>>()
    };

    // Fetch the unspent records.
    let records = find_records();
    let record_1 = records[0].clone();

    // Create an execution with different transition ids, but with a fixed output record (output ID).
    let mut create_execution_with_duplicate_output_id = |x: u64| -> Transaction<CurrentNetwork> {
        // Use a fixed seed RNG.
        let fixed_rng = &mut TestRng::from_seed(1);

        // Create a transaction with a fixed rng.
        let inputs = [Value::from_str(&format!("{x}u64")).unwrap()];
        let transaction = ledger
            .vm
            .execute(
                &private_key,
                ("dummy_program.aleo", "create_duplicate_record"),
                inputs.into_iter(),
                None,
                0,
                None,
                fixed_rng,
            )
            .unwrap();
        // Extract the execution.
        let execution = transaction.execution().unwrap().clone();

        // Create a new fee for the execution.
        let fee_authorization = ledger
            .vm
            .authorize_fee_public(
                &private_key,
                *transaction.fee_amount().unwrap(),
                0,
                execution.to_execution_id().unwrap(),
                rng,
            )
            .unwrap();
        let fee = ledger.vm.execute_fee_authorization(fee_authorization, None, rng).unwrap();

        Transaction::from_execution(execution, Some(fee)).unwrap()
    };

    // Create an deployment with different transition ids, but with a fixed output record (output ID).
    let create_deployment_with_duplicate_output_id = |x: u64| -> Transaction<CurrentNetwork> {
        // Use a fixed seed RNG.
        let fixed_rng = &mut TestRng::from_seed(1);

        // Deploy a test program to the ledger.
        let program = Program::<CurrentNetwork>::from_str(&format!(
            "
program dummy_program_{x}.aleo;

record dummy_program:
    owner as address.private;
    rand_var as u64.private;

function create_duplicate_record:
    input r0 as u64.private;
    cast self.caller 1u64 into r1 as dummy_program.record;
    output r1 as dummy_program.record;"
        ))
        .unwrap();

        // Create a transaction with a fixed rng.
        let transaction = ledger.vm.deploy(&private_key, &program, None, 0, None, fixed_rng).unwrap();

        // Extract the deployment and owner.
        let deployment = transaction.deployment().unwrap().clone();
        let owner = *transaction.owner().unwrap();

        // Create a new fee for the execution.
        let fee_authorization = ledger
            .vm
            .authorize_fee_private(
                &private_key,
                record_1.clone(),
                *transaction.fee_amount().unwrap(),
                0,
                deployment.to_deployment_id().unwrap(),
                fixed_rng,
            )
            .unwrap();
        let fee = ledger.vm.execute_fee_authorization(fee_authorization, None, fixed_rng).unwrap();

        Transaction::from_deployment(owner, deployment, fee).unwrap()
    };

    // Create the first transfer.
    let transfer_1 = create_execution_with_duplicate_output_id(1);
    let transfer_1_id = transfer_1.id();

    // Create a second transfer with the same output id.
    let transfer_2 = create_execution_with_duplicate_output_id(2);
    let transfer_2_id = transfer_2.id();

    // Create a third transfer with the same output id.
    let transfer_3 = create_execution_with_duplicate_output_id(3);
    let transfer_3_id = transfer_3.id();

    // Ensure that each transaction has a duplicate output id.
    let tx_1_output_id = transfer_1.output_ids().next().unwrap();
    let tx_2_output_id = transfer_2.output_ids().next().unwrap();
    let tx_3_output_id = transfer_3.output_ids().next().unwrap();
    assert_eq!(tx_1_output_id, tx_2_output_id);
    assert_eq!(tx_1_output_id, tx_3_output_id);

    // Create the first deployment.
    let deployment_1 = create_deployment_with_duplicate_output_id(1);
    let deployment_1_id = deployment_1.id();

    // Create a second deployment with the same output id.
    let deployment_2 = create_deployment_with_duplicate_output_id(2);
    let deployment_2_id = deployment_2.id();

    // Create a third deployment with the same output id.
    let deployment_3 = create_deployment_with_duplicate_output_id(3);
    let deployment_3_id = deployment_3.id();

    // Ensure that each transaction has a duplicate output id.
    let deployment_1_output_id = deployment_1.output_ids().next().unwrap();
    let deployment_2_output_id = deployment_2.output_ids().next().unwrap();
    let deployment_3_output_id = deployment_3.output_ids().next().unwrap();
    assert_eq!(deployment_1_output_id, deployment_2_output_id);
    assert_eq!(deployment_1_output_id, deployment_3_output_id);

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(
            &private_key,
            vec![],
            vec![],
            vec![transfer_1, transfer_2, deployment_1, deployment_2],
            rng,
        )
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Enforce that the block transactions were correct.
    assert_eq!(block.transactions().num_accepted(), 2);
    assert_eq!(block.transactions().transaction_ids().collect::<Vec<_>>(), vec![&transfer_1_id, &deployment_1_id]);
    assert_eq!(block.aborted_transaction_ids(), &vec![transfer_2_id, deployment_2_id]);

    // Ensure that verification was not run on aborted deployments.
    let partially_verified_transaction = ledger.vm().partially_verified_transactions().read().clone();
    assert!(partially_verified_transaction.contains(&transfer_1_id));
    assert!(partially_verified_transaction.contains(&deployment_1_id));
    assert!(!partially_verified_transaction.contains(&transfer_2_id));
    assert!(!partially_verified_transaction.contains(&deployment_2_id));

    // Prepare a transfer that will succeed for the subsequent block.
    let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("1000u64").unwrap()];
    let transfer_4 = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.into_iter(), None, 0, None, rng)
        .unwrap();
    let transfer_4_id = transfer_4.id();

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(
            &private_key,
            vec![],
            vec![],
            vec![transfer_3, transfer_4, deployment_3],
            rng,
        )
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Enforce that the block transactions were correct.
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().transaction_ids().collect::<Vec<_>>(), vec![&transfer_4_id]);
    assert_eq!(block.aborted_transaction_ids(), &vec![transfer_3_id, deployment_3_id]);

    // Ensure that verification was not run on transactions aborted in a previous block.
    let partially_verified_transaction = ledger.vm().partially_verified_transactions().read().clone();
    assert!(partially_verified_transaction.contains(&transfer_4_id));
    assert!(!partially_verified_transaction.contains(&transfer_3_id));
    assert!(!partially_verified_transaction.contains(&deployment_3_id));
}

#[test]
fn test_execute_duplicate_transition_ids() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, address, .. } = crate::test_helpers::sample_test_env(rng);

    // Deploy a test program to the ledger.
    let program = Program::<CurrentNetwork>::from_str(
        "
program dummy_program.aleo;

function empty_function:
    ",
    )
    .unwrap();

    // Deploy.
    let deployment_transaction = ledger.vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
    // Verify.
    ledger.vm().check_transaction(&deployment_transaction, None, rng).unwrap();

    // Construct the next block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![deployment_transaction], rng)
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();
    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Create a transaction with different transaction IDs, but with a fixed transition ID.
    // NOTE: there's no use creating deployments with duplicate (fee) transition ids,
    //       as this is only possible if they have duplicate programs, duplicate transaction_ids,
    //       which will not abort but fail on check_next_block.
    let mut create_transaction_with_duplicate_transition_id = || -> Transaction<CurrentNetwork> {
        // Use a fixed seed RNG.
        let fixed_rng = &mut TestRng::from_seed(1);

        // Create a transaction with a fixed rng.
        let inputs: [Value<_>; 0] = [];
        let transaction = ledger
            .vm
            .execute(
                &private_key,
                ("dummy_program.aleo", "empty_function"),
                inputs.into_iter(),
                None,
                0,
                None,
                fixed_rng,
            )
            .unwrap();
        // Extract the execution.
        let execution = transaction.execution().unwrap().clone();

        // Create a new fee for the execution.
        let fee_authorization = ledger
            .vm
            .authorize_fee_public(
                &private_key,
                *transaction.fee_amount().unwrap(),
                0,
                execution.to_execution_id().unwrap(),
                rng,
            )
            .unwrap();
        let fee = ledger.vm.execute_fee_authorization(fee_authorization, None, rng).unwrap();

        Transaction::from_execution(execution, Some(fee)).unwrap()
    };

    // Create the first transaction.
    let transaction_1 = create_transaction_with_duplicate_transition_id();
    let transaction_1_id = transaction_1.id();

    // Create a second transaction with the same transition id.
    let transaction_2 = create_transaction_with_duplicate_transition_id();
    let transaction_2_id = transaction_2.id();

    // Create a third transaction with the same transition_id
    let transaction_3 = create_transaction_with_duplicate_transition_id();
    let transaction_3_id = transaction_3.id();

    // Ensure that each transaction has a duplicate transition id.
    let tx_1_transition_id = transaction_1.transition_ids().next().unwrap();
    let tx_2_transition_id = transaction_2.transition_ids().next().unwrap();
    let tx_3_transition_id = transaction_3.transition_ids().next().unwrap();
    assert_eq!(tx_1_transition_id, tx_2_transition_id);
    assert_eq!(tx_1_transition_id, tx_3_transition_id);

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![transaction_1, transaction_2], rng)
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Enforce that the block transactions were correct.
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().transaction_ids().collect::<Vec<_>>(), vec![&transaction_1_id]);
    assert_eq!(block.aborted_transaction_ids(), &vec![transaction_2_id]);

    // Ensure that verification was not run on aborted transactions.
    let partially_verified_transaction = ledger.vm().partially_verified_transactions().read().clone();
    assert!(partially_verified_transaction.contains(&transaction_1_id));
    assert!(!partially_verified_transaction.contains(&transaction_2_id));

    // Prepare a transfer that will succeed for the subsequent block.
    let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("1000u64").unwrap()];
    let transfer_transaction = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.into_iter(), None, 0, None, rng)
        .unwrap();
    let transfer_transaction_id = transfer_transaction.id();

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(
            &private_key,
            vec![],
            vec![],
            vec![transaction_3, transfer_transaction],
            rng,
        )
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Enforce that the block transactions were correct.
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().transaction_ids().collect::<Vec<_>>(), vec![&transfer_transaction_id]);
    assert_eq!(block.aborted_transaction_ids(), &vec![transaction_3_id]);

    // Ensure that verification was not run on transactions aborted in a previous block.
    let partially_verified_transaction = ledger.vm().partially_verified_transactions().read().clone();
    assert!(partially_verified_transaction.contains(&transfer_transaction_id));
    assert!(!partially_verified_transaction.contains(&transaction_3_id));
}

#[test]
fn test_execute_duplicate_transition_public_keys() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, address, .. } = crate::test_helpers::sample_test_env(rng);

    // Deploy a test program to the ledger.
    let program = Program::<CurrentNetwork>::from_str(
        "
program dummy_program.aleo;

function empty_function:

function simple_output:
    output 1u64 as u64.public;
    ",
    )
    .unwrap();

    // Deploy.
    let deployment_transaction = ledger.vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
    // Verify.
    ledger.vm().check_transaction(&deployment_transaction, None, rng).unwrap();

    // Construct the next block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![deployment_transaction], rng)
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();
    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Create a transaction with different transaction ids, but with a duplicate TPK.
    // NOTE: there's no use creating deployments with duplicate (fee) TPKs,
    //       as this is only possible if they have duplicate programs, duplicate transaction_ids,
    //       which will not abort but fail on check_next_block.
    let mut create_transaction_with_duplicate_tpk = |function: &str| -> Transaction<CurrentNetwork> {
        // Use a fixed seed RNG.
        let fixed_rng = &mut TestRng::from_seed(1);

        // Create a transaction with a fixed rng.
        let inputs: [Value<_>; 0] = [];
        let transaction = ledger
            .vm
            .execute(&private_key, ("dummy_program.aleo", function), inputs.into_iter(), None, 0, None, fixed_rng)
            .unwrap();
        // Extract the execution.
        let execution = transaction.execution().unwrap().clone();

        // Create a new fee for the execution.
        let fee_authorization = ledger
            .vm
            .authorize_fee_public(
                &private_key,
                *transaction.fee_amount().unwrap(),
                0,
                execution.to_execution_id().unwrap(),
                rng,
            )
            .unwrap();
        let fee = ledger.vm.execute_fee_authorization(fee_authorization, None, rng).unwrap();

        Transaction::from_execution(execution, Some(fee)).unwrap()
    };

    // Create the first transaction.
    let transaction_1 = create_transaction_with_duplicate_tpk("empty_function");
    let transaction_1_id = transaction_1.id();

    // Create a second transaction with the same tpk and tcm.
    let transaction_2 = create_transaction_with_duplicate_tpk("simple_output");
    let transaction_2_id = transaction_2.id();

    // Create a third transaction with the same tpk and tcm.
    let transaction_3 = create_transaction_with_duplicate_tpk("simple_output");
    let transaction_3_id = transaction_3.id();

    // Ensure that each transaction has a duplicate tcm and tpk.
    let tx_1_tpk = transaction_1.transitions().next().unwrap().tpk();
    let tx_2_tpk = transaction_2.transitions().next().unwrap().tpk();
    let tx_3_tpk = transaction_3.transitions().next().unwrap().tpk();
    assert_eq!(tx_1_tpk, tx_2_tpk);
    assert_eq!(tx_1_tpk, tx_3_tpk);

    let tx_1_tcm = transaction_1.transitions().next().unwrap().tcm();
    let tx_2_tcm = transaction_2.transitions().next().unwrap().tcm();
    let tx_3_tcm = transaction_3.transitions().next().unwrap().tcm();
    assert_eq!(tx_1_tcm, tx_2_tcm);
    assert_eq!(tx_1_tcm, tx_3_tcm);

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![transaction_1, transaction_2], rng)
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Enforce that the block transactions were correct.
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().transaction_ids().collect::<Vec<_>>(), vec![&transaction_1_id]);
    assert_eq!(block.aborted_transaction_ids(), &vec![transaction_2_id]);

    // Ensure that verification was not run on aborted transactions.
    let partially_verified_transaction = ledger.vm().partially_verified_transactions().read().clone();
    assert!(partially_verified_transaction.contains(&transaction_1_id));
    assert!(!partially_verified_transaction.contains(&transaction_2_id));

    // Prepare a transfer that will succeed for the subsequent block.
    let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("1000u64").unwrap()];
    let transfer_transaction = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.into_iter(), None, 0, None, rng)
        .unwrap();
    let transfer_transaction_id = transfer_transaction.id();

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(
            &private_key,
            vec![],
            vec![],
            vec![transaction_3, transfer_transaction],
            rng,
        )
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Enforce that the block transactions were correct.
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().transaction_ids().collect::<Vec<_>>(), vec![&transfer_transaction_id]);
    assert_eq!(block.aborted_transaction_ids(), &vec![transaction_3_id]);

    // Ensure that verification was not run on transactions aborted in a previous block.
    let partially_verified_transaction = ledger.vm().partially_verified_transactions().read().clone();
    assert!(partially_verified_transaction.contains(&transfer_transaction_id));
    assert!(!partially_verified_transaction.contains(&transaction_3_id));
}

#[test]
fn test_abort_multiple_deployments_with_same_payer() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, .. } = crate::test_helpers::sample_test_env(rng);

    // Create two distinct programs
    let program_1 = Program::<CurrentNetwork>::from_str(
        "
program dummy_program_1.aleo;

function empty_function:
    ",
    )
    .unwrap();

    let program_2 = Program::<CurrentNetwork>::from_str(
        "
program dummy_program_2.aleo;

function empty_function:
    ",
    )
    .unwrap();

    // Create a deployment transaction for the first program with the same public payer.
    let deployment_1 = ledger.vm.deploy(&private_key, &program_1, None, 0, None, rng).unwrap();
    let deployment_1_id = deployment_1.id();

    // Create a deployment transaction for the second program with the same public payer.
    let deployment_2 = ledger.vm.deploy(&private_key, &program_2, None, 0, None, rng).unwrap();
    let deployment_2_id = deployment_2.id();

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![deployment_1, deployment_2], rng)
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Enforce that the block transactions were correct.
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.aborted_transaction_ids(), &vec![deployment_2_id]);

    // Enforce that the first program was deployed and the second was aborted.
    assert_eq!(ledger.get_program(*program_1.id()).unwrap(), program_1);
    assert!(ledger.vm.transaction_store().contains_transaction_id(&deployment_1_id).unwrap());
    assert!(ledger.vm.block_store().contains_rejected_or_aborted_transaction_id(&deployment_2_id).unwrap());

    // Ensure that verification was not run on aborted transactions.
    let partially_verified_transaction = ledger.vm().partially_verified_transactions().read().clone();
    assert!(partially_verified_transaction.contains(&deployment_1_id));
    assert!(!partially_verified_transaction.contains(&deployment_2_id));
}

#[test]
fn test_abort_fee_transaction() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, address, .. } = crate::test_helpers::sample_test_env(rng);

    // Construct valid transaction for the ledger.
    let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("1000u64").unwrap()];
    let transaction = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.clone().into_iter(), None, 0, None, rng)
        .unwrap();
    let transaction_id = transaction.id();

    // Convert a fee transaction.
    let transaction_to_convert_to_fee = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.into_iter(), None, 0, None, rng)
        .unwrap();
    let fee_transaction = Transaction::from_fee(transaction_to_convert_to_fee.fee_transition().unwrap()).unwrap();
    let fee_transaction_id = fee_transaction.id();

    // Create a block using a fee transaction.
    let block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![fee_transaction, transaction], rng)
        .unwrap();

    // Check that the block aborts the invalid transaction.
    assert_eq!(block.aborted_transaction_ids(), &vec![fee_transaction_id]);
    assert_eq!(block.transaction_ids().collect::<Vec<_>>(), vec![&transaction_id]);

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();
}

#[test]
fn test_abort_invalid_transaction() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, address, .. } = crate::test_helpers::sample_test_env(rng);

    // Initialize a new VM.
    let vm = sample_vm();

    // Construct a custom genesis block.
    let custom_genesis = vm.genesis_beacon(&private_key, rng).unwrap();

    // Update the VM.
    vm.add_next_block(&custom_genesis).unwrap();

    // Generate a transaction that will be invalid on another network.
    let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("1000u64").unwrap()];
    let invalid_transaction = vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.clone().into_iter(), None, 0, None, rng)
        .unwrap();
    let invalid_transaction_id = invalid_transaction.id();

    // Check that the ledger deems this transaction invalid.
    assert!(ledger.check_transaction_basic(&invalid_transaction, None, rng).is_err());

    // Construct valid transactions for the ledger.
    let valid_transaction_1 = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.clone().into_iter(), None, 0, None, rng)
        .unwrap();
    let valid_transaction_2 = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.into_iter(), None, 0, None, rng)
        .unwrap();
    let valid_transaction_id_1 = valid_transaction_1.id();
    let valid_transaction_id_2 = valid_transaction_2.id();

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(
            &private_key,
            vec![],
            vec![],
            vec![valid_transaction_1, invalid_transaction, valid_transaction_2],
            rng,
        )
        .unwrap();

    // Check that the block aborts the invalid transaction.
    assert_eq!(block.aborted_transaction_ids(), &vec![invalid_transaction_id]);
    assert_eq!(block.transaction_ids().collect::<Vec<_>>(), vec![&valid_transaction_id_1, &valid_transaction_id_2]);

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();
}

#[test]
fn test_deploy_difference() {
    println!("CONSENSUS_V2_HEIGHT: {:?}", CurrentNetwork::CONSENSUS_V2_HEIGHT);
    let rng = &mut TestRng::default();

    let prelim_program_1 = Program::<CurrentNetwork>::from_str(
        r"
        import credits.aleo;

program staking_v1.aleo;

record PrivateToken:
    owner as address.private;
    amount as u64.private;

struct Metadata:
    name as u128;
    symbol as u128;
    decimals as u8;

struct State:
    total_supply as u64;
    total_reserve as u64;
    total_unstaking as u64;
    total_reward as u64;

struct Settings:
    unstake_wait as u32;
    stake_paused as boolean;
    global_paused as boolean;
    max_reward_per_notify as u64;
    protocol_fee as u16;
    fee_account as address;

struct Unstaking:
    microcredits as u64;
    height as u32;

struct ApprovalKey:
    approver as address;
    spender as address;

mapping account:
    key as address.public;
    value as u64.public;

mapping approvals:
    key as field.public;
    value as u64.public;

mapping metadata:
    key as boolean.public;
    value as Metadata.public;

mapping state:
    key as boolean.public;
    value as State.public;

mapping settings:
    key as boolean.public;
    value as Settings.public;

mapping unstakings:
    key as address.public;
    value as Unstaking.public;

mapping admins:
    key as address.public;
    value as boolean.public;

mapping stakers:
    key as address.public;
    value as boolean.public;

function transfer_public:
    input r0 as address.public;
    input r1 as u64.public;
    async transfer_public self.caller r0 r1 into r2;
    output r2 as staking_v1.aleo/transfer_public.future;
finalize transfer_public:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as u64.public;
    get account[r0] into r3;
    sub r3 r2 into r4;
    set r4 into account[r0];
    get.or_use account[r1] 0u64 into r5;
    add r5 r2 into r6;
    set r6 into account[r1];

function transfer_private:
    input r0 as PrivateToken.record;
    input r1 as address.private;
    input r2 as u64.private;
    cast r1 r2 into r3 as PrivateToken.record;
    sub r0.amount r2 into r4;
    cast r0.owner r4 into r5 as PrivateToken.record;
    output r3 as PrivateToken.record;
    output r5 as PrivateToken.record;

function transfer_private_to_public:
    input r0 as PrivateToken.record;
    input r1 as address.public;
    input r2 as u64.public;
    sub r0.amount r2 into r3;
    cast r0.owner r3 into r4 as PrivateToken.record;
    async transfer_private_to_public r1 r2 into r5;
    output r4 as PrivateToken.record;
    output r5 as staking_v1.aleo/transfer_private_to_public.future;
finalize transfer_private_to_public:
    input r0 as address.public;
    input r1 as u64.public;
    get.or_use account[r0] 0u64 into r2;
    add r2 r1 into r3;
    set r3 into account[r0];

function transfer_public_to_private:
    input r0 as address.private;
    input r1 as u64.public;
    cast r0 r1 into r2 as PrivateToken.record;
    async transfer_public_to_private self.caller r1 into r3;
    output r2 as PrivateToken.record;
    output r3 as staking_v1.aleo/transfer_public_to_private.future;
finalize transfer_public_to_private:
    input r0 as address.public;
    input r1 as u64.public;
    get account[r0] into r2;
    sub r2 r1 into r3;
    set r3 into account[r0];

function join:
    input r0 as PrivateToken.record;
    input r1 as PrivateToken.record;
    add r0.amount r1.amount into r2;
    cast r0.owner r2 into r3 as PrivateToken.record;
    output r3 as PrivateToken.record;

function stake_public:
    input r0 as u64.public;
    input r1 as address.public;
    assert.neq r0 0u64;
    call credits.aleo/transfer_public_as_signer staking_v1.aleo r0 into r2;
    async stake_public r0 r1 r2 into r3;
    output r3 as staking_v1.aleo/stake_public.future;
finalize stake_public:
    input r0 as u64.public;
    input r1 as address.public;
    input r2 as credits.aleo/transfer_public_as_signer.future;
    await r2;
    get settings[true] into r3;
    assert.eq r3.stake_paused false;
    assert.eq r3.global_paused false;
    get state[true] into r4;
    is.eq r4.total_reserve 0u64 into r5;
    cast r4.total_reserve into r6 as u128;
    ternary r5 1u128 r6 into r7;
    is.eq r4.total_reserve 0u64 into r8;
    cast r0 into r9 as u128;
    cast r4.total_supply into r10 as u128;
    mul r9 r10 into r11;
    div r11 r7 into r12;
    cast r12 into r13 as u64;
    ternary r8 r0 r13 into r14;
    assert.neq r14 0u64;
    add r4.total_supply r14 into r15;
    add r4.total_reserve r0 into r16;
    cast r15 r16 r4.total_unstaking r4.total_reward into r17 as State;
    set r17 into state[true];
    get.or_use account[r1] 0u64 into r18;
    add r18 r14 into r19;
    set r19 into account[r1];

function stake_private:
    input r0 as credits.aleo/credits.record;
    input r1 as u64.public;
    input r2 as address.public;
    assert.neq r1 0u64;
    call credits.aleo/transfer_private_to_public r0 staking_v1.aleo r1 into r3 r4;
    async stake_private r1 r2 r4 into r5;
    output r3 as credits.aleo/credits.record;
    output r5 as staking_v1.aleo/stake_private.future;
finalize stake_private:
    input r0 as u64.public;
    input r1 as address.public;
    input r2 as credits.aleo/transfer_private_to_public.future;
    await r2;
    get settings[true] into r3;
    assert.eq r3.stake_paused false;
    assert.eq r3.global_paused false;
    get state[true] into r4;
    is.eq r4.total_reserve 0u64 into r5;
    cast r4.total_reserve into r6 as u128;
    ternary r5 1u128 r6 into r7;
    is.eq r4.total_reserve 0u64 into r8;
    cast r0 into r9 as u128;
    cast r4.total_supply into r10 as u128;
    mul r9 r10 into r11;
    div r11 r7 into r12;
    cast r12 into r13 as u64;
    ternary r8 r0 r13 into r14;
    assert.neq r14 0u64;
    add r4.total_supply r14 into r15;
    add r4.total_reserve r0 into r16;
    cast r15 r16 r4.total_unstaking r4.total_reward into r17 as State;
    set r17 into state[true];
    get.or_use account[r1] 0u64 into r18;
    add r18 r14 into r19;
    set r19 into account[r1];

function unstake_token:
    input r0 as u64.public;
    assert.neq r0 0u64;
    async unstake_token self.caller r0 into r1;
    output r1 as staking_v1.aleo/unstake_token.future;
finalize unstake_token:
    input r0 as address.public;
    input r1 as u64.public;
    get settings[true] into r2;
    assert.eq r2.global_paused false;
    get.or_use account[r0] 0u64 into r3;
    sub r3 r1 into r4;
    set r4 into account[r0];
    get state[true] into r5;
    cast r1 into r6 as u128;
    cast r5.total_reserve into r7 as u128;
    mul r6 r7 into r8;
    cast r5.total_supply into r9 as u128;
    div r8 r9 into r10;
    cast r10 into r11 as u64;
    assert.neq r11 0u64;
    sub r5.total_supply r1 into r12;
    sub r5.total_reserve r11 into r13;
    add r5.total_unstaking r11 into r14;
    cast r12 r13 r14 r5.total_reward into r15 as State;
    set r15 into state[true];
    cast 0u64 0u32 into r16 as Unstaking;
    get.or_use unstakings[r0] r16 into r17;
    add r17.microcredits r11 into r18;
    add block.height r2.unstake_wait into r19;
    cast r18 r19 into r20 as Unstaking;
    set r20 into unstakings[r0];

function unstake_aleo:
    input r0 as u64.public;
    assert.neq r0 0u64;
    async unstake_aleo self.caller r0 into r1;
    output r1 as staking_v1.aleo/unstake_aleo.future;
finalize unstake_aleo:
    input r0 as address.public;
    input r1 as u64.public;
    get settings[true] into r2;
    assert.eq r2.global_paused false;
    get state[true] into r3;
    cast r1 into r4 as u128;
    cast r3.total_supply into r5 as u128;
    mul r4 r5 into r6;
    cast r3.total_reserve into r7 as u128;
    add r6 r7 into r8;
    sub r8 1u128 into r9;
    cast r3.total_reserve into r10 as u128;
    div r9 r10 into r11;
    cast r11 into r12 as u64;
    assert.neq r12 0u64;
    get.or_use account[r0] 0u64 into r13;
    sub r13 r12 into r14;
    set r14 into account[r0];
    sub r3.total_supply r12 into r15;
    sub r3.total_reserve r1 into r16;
    add r3.total_unstaking r1 into r17;
    cast r15 r16 r17 r3.total_reward into r18 as State;
    set r18 into state[true];
    cast 0u64 0u32 into r19 as Unstaking;
    get.or_use unstakings[r0] r19 into r20;
    add r20.microcredits r1 into r21;
    add block.height r2.unstake_wait into r22;
    cast r21 r22 into r23 as Unstaking;
    set r23 into unstakings[r0];

function withdraw:
    input r0 as u64.public;
    input r1 as address.public;
    assert.neq r0 0u64;
    call credits.aleo/transfer_public r1 r0 into r2;
    async withdraw self.caller r0 r2 into r3;
    output r3 as staking_v1.aleo/withdraw.future;
finalize withdraw:
    input r0 as address.public;
    input r1 as u64.public;
    input r2 as credits.aleo/transfer_public.future;
    await r2;
    get settings[true] into r3;
    assert.eq r3.global_paused false;
    get unstakings[r0] into r4;
    gte block.height r4.height into r5;
    assert.eq r5 true;
    sub r4.microcredits r1 into r6;
    cast r6 r4.height into r7 as Unstaking;
    set r7 into unstakings[r0];
    get state[true] into r8;
    sub r8.total_unstaking r1 into r9;
    cast r8.total_supply r8.total_reserve r9 r8.total_reward into r10 as State;
    set r10 into state[true];

function withdraw_private:
    input r0 as u64.public;
    input r1 as address.private;
    assert.neq r0 0u64;
    call credits.aleo/transfer_public_to_private r1 r0 into r2 r3;
    async withdraw_private self.caller r0 r3 into r4;
    output r2 as credits.aleo/credits.record;
    output r4 as staking_v1.aleo/withdraw_private.future;
finalize withdraw_private:
    input r0 as address.public;
    input r1 as u64.public;
    input r2 as credits.aleo/transfer_public_to_private.future;
    await r2;
    get settings[true] into r3;
    assert.eq r3.global_paused false;
    get unstakings[r0] into r4;
    gte block.height r4.height into r5;
    assert.eq r5 true;
    sub r4.microcredits r1 into r6;
    cast r6 r4.height into r7 as Unstaking;
    set r7 into unstakings[r0];
    get state[true] into r8;
    sub r8.total_unstaking r1 into r9;
    cast r8.total_supply r8.total_reserve r9 r8.total_reward into r10 as State;
    set r10 into state[true];

function approve_public:
    input r0 as address.public;
    input r1 as u64.public;
    cast self.caller r0 into r2 as ApprovalKey;
    hash.bhp256 r2 into r3 as field;
    async approve_public r3 r1 into r4;
    output r4 as staking_v1.aleo/approve_public.future;
finalize approve_public:
    input r0 as field.public;
    input r1 as u64.public;
    get.or_use approvals[r0] 0u64 into r2;
    sub 18446744073709551615u64 r2 into r3;
    lt r1 r3 into r4;
    add.w r2 r1 into r5;
    ternary r4 r5 18446744073709551615u64 into r6;
    set r6 into approvals[r0];

function unapprove_public:
    input r0 as address.public;
    input r1 as u64.public;
    cast self.caller r0 into r2 as ApprovalKey;
    hash.bhp256 r2 into r3 as field;
    async unapprove_public r3 r1 into r4;
    output r4 as staking_v1.aleo/unapprove_public.future;
finalize unapprove_public:
    input r0 as field.public;
    input r1 as u64.public;
    get approvals[r0] into r2;
    gt r2 r1 into r3;
    sub.w r2 r1 into r4;
    ternary r3 r4 0u64 into r5;
    set r5 into approvals[r0];

function transfer_from_public:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as u64.public;
    cast r0 self.caller into r3 as ApprovalKey;
    hash.bhp256 r3 into r4 as field;
    async transfer_from_public r4 r0 r1 r2 into r5;
    output r5 as staking_v1.aleo/transfer_from_public.future;
finalize transfer_from_public:
    input r0 as field.public;
    input r1 as address.public;
    input r2 as address.public;
    input r3 as u64.public;
    get approvals[r0] into r4;
    sub r4 r3 into r5;
    set r5 into approvals[r0];
    get account[r1] into r6;
    sub r6 r3 into r7;
    set r7 into account[r1];
    get.or_use account[r2] 0u64 into r8;
    add r8 r3 into r9;
    set r9 into account[r2];

function notify_reward:
    input r0 as u64.public;
    assert.neq r0 0u64;
    async notify_reward self.caller r0 into r1;
    output r1 as staking_v1.aleo/notify_reward.future;
finalize notify_reward:
    input r0 as address.public;
    input r1 as u64.public;
    get settings[true] into r2;
    assert.eq r2.global_paused false;
    get stakers[r0] into r3;
    assert.eq r3 true;
    lte r1 r2.max_reward_per_notify into r4;
    assert.eq r4 true;
    get state[true] into r5;
    cast r5.total_reserve into r6 as u128;
    mul r6 10000u128 into r7;
    cast r1 into r8 as u128;
    cast r2.protocol_fee into r9 as u128;
    sub 10000u128 r9 into r10;
    mul r8 r10 into r11;
    add r7 r11 into r12;
    cast r2.protocol_fee into r13 as u128;
    cast r1 into r14 as u128;
    mul r13 r14 into r15;
    cast r5.total_supply into r16 as u128;
    mul r15 r16 into r17;
    div r17 r12 into r18;
    cast r18 into r19 as u64;
    add r5.total_supply r19 into r20;
    add r5.total_reserve r1 into r21;
    add r5.total_reward r1 into r22;
    cast r20 r21 r5.total_unstaking r22 into r23 as State;
    set r23 into state[true];
    get.or_use account[r2.fee_account] 0u64 into r24;
    add r24 r19 into r25;
    set r25 into account[r2.fee_account];

function pull_aleo:
    input r0 as u64.public;
    call credits.aleo/transfer_public self.caller r0 into r1;
    async pull_aleo self.caller r1 into r2;
    output r2 as staking_v1.aleo/pull_aleo.future;
finalize pull_aleo:
    input r0 as address.public;
    input r1 as credits.aleo/transfer_public.future;
    await r1;
    get settings[true] into r2;
    assert.eq r2.global_paused false;
    get stakers[r0] into r3;
    assert.eq r3 true;

function update_settings:
    input r0 as Settings.public;
    lte r0.protocol_fee 5000u16 into r1;
    assert.eq r1 true;
    async update_settings self.caller r0 into r2;
    output r2 as staking_v1.aleo/update_settings.future;
finalize update_settings:
    input r0 as address.public;
    input r1 as Settings.public;
    get admins[r0] into r2;
    assert.eq r2 true;
    set r1 into settings[true];

function set_staker:
    input r0 as address.public;
    input r1 as boolean.public;
    async set_staker self.caller r0 r1 into r2;
    output r2 as staking_v1.aleo/set_staker.future;
finalize set_staker:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as boolean.public;
    get admins[r0] into r3;
    assert.eq r3 true;
    set r2 into stakers[r1];

function set_admin:
    input r0 as address.public;
    input r1 as boolean.public;
    assert.neq self.caller r0;
    async set_admin self.caller r0 r1 into r2;
    output r2 as staking_v1.aleo/set_admin.future;
finalize set_admin:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as boolean.public;
    get admins[r0] into r3;
    assert.eq r3 true;
    set r2 into admins[r1];

function init:
    input r0 as address.public;
    assert.eq self.caller aleo18jfrqsz4m853grpgzflrlzdkcm9art926668g80vd9ruyzv8rsqqlchzyj;
    async init r0 into r1;
    output r1 as staking_v1.aleo/init.future;
finalize init:
    input r0 as address.public;
    contains metadata[true] into r1;
    assert.eq r1 false;
    cast 394103260206656316532079u128 126943148918095u128 6u8 into r2 as Metadata;
    set r2 into metadata[true];
    set true into admins[aleo18jfrqsz4m853grpgzflrlzdkcm9art926668g80vd9ruyzv8rsqqlchzyj];
    cast 0u64 0u64 0u64 0u64 into r3 as State;
    set r3 into state[true];
    cast 360u32 false false 1000000000u64 1000u16 r0 into r4 as Settings;
    set r4 into settings[true];
    ",
    )
    .unwrap();

    let prelim_program_2 = Program::<CurrentNetwork>::from_str(
        r"
        import credits.aleo;
import staking_v1.aleo;

program staker_v1_b.aleo;

struct bond_state:
    validator as address;
    microcredits as u64;

struct unbond_state:
    microcredits as u64;
    height as u32;

struct Settings:
    unstake_wait as u32;
    stake_paused as boolean;
    global_paused as boolean;
    max_reward_per_notify as u64;
    protocol_fee as u16;
    fee_account as address;

struct StakerState:
    total_pulled as u64;
    total_pushed as u64;
    total_reward as u64;

mapping admins:
    key as address.public;
    value as boolean.public;

mapping operators:
    key as address.public;
    value as boolean.public;

mapping state:
    key as boolean.public;
    value as StakerState.public;

function notify_reward:
    input r0 as u64.public;
    call staking_v1.aleo/notify_reward r0 into r1;
    async notify_reward self.caller r0 r1 into r2;
    output r2 as staker_v1_b.aleo/notify_reward.future;
finalize notify_reward:
    input r0 as address.public;
    input r1 as u64.public;
    input r2 as staking_v1.aleo/notify_reward.future;
    await r2;
    get operators[r0] into r3;
    assert.eq r3 true;
    get.or_use credits.aleo/account[staker_v1_b.aleo] 0u64 into r4;
    cast aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc 0u64 into r5 as bond_state;
    get.or_use credits.aleo/bonded[staker_v1_b.aleo] r5 into r6;
    cast 0u64 0u32 into r7 as unbond_state;
    get.or_use credits.aleo/unbonding[staker_v1_b.aleo] r7 into r8;
    get state[true] into r9;
    add r4 r6.microcredits into r10;
    add r10 r8.microcredits into r11;
    add r11 r9.total_pushed into r12;
    sub r12 r9.total_pulled into r13;
    sub r13 r9.total_reward into r14;
    lte r1 r14 into r15;
    assert.eq r15 true;
    add r9.total_reward r1 into r16;
    cast r9.total_pulled r9.total_pushed r16 into r17 as StakerState;
    set r17 into state[true];

function set_admin:
    input r0 as address.public;
    input r1 as boolean.public;
    assert.neq self.caller r0;
    async set_admin self.caller r0 r1 into r2;
    output r2 as staker_v1_b.aleo/set_admin.future;
finalize set_admin:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as boolean.public;
    get admins[r0] into r3;
    assert.eq r3 true;
    set r2 into admins[r1];

function init:
    assert.eq self.caller aleo18jfrqsz4m853grpgzflrlzdkcm9art926668g80vd9ruyzv8rsqqlchzyj;
    async init  into r0;
    output r0 as staker_v1_b.aleo/init.future;
finalize init:
    contains admins[aleo18jfrqsz4m853grpgzflrlzdkcm9art926668g80vd9ruyzv8rsqqlchzyj] into r0;
    assert.eq r0 false;
    set true into admins[aleo18jfrqsz4m853grpgzflrlzdkcm9art926668g80vd9ruyzv8rsqqlchzyj];
    set true into operators[aleo18jfrqsz4m853grpgzflrlzdkcm9art926668g80vd9ruyzv8rsqqlchzyj];
    cast 0u64 0u64 0u64 into r1 as StakerState;
    set r1 into state[true];

function bond:
    input r0 as address.public;
    input r1 as u64.public;
    input r2 as u64.public;
    lte r2 r1 into r3;
    assert.eq r3 true;
    call staking_v1.aleo/pull_aleo r2 into r4;
    call credits.aleo/bond_public r0 staker_v1_b.aleo r1 into r5;
    async bond self.caller r2 r4 r5 into r6;
    output r6 as staker_v1_b.aleo/bond.future;
finalize bond:
    input r0 as address.public;
    input r1 as u64.public;
    input r2 as staking_v1.aleo/pull_aleo.future;
    input r3 as credits.aleo/bond_public.future;
    await r2;
    await r3;
    get operators[r0] into r4;
    assert.eq r4 true;
    get state[true] into r5;
    add r5.total_pulled r1 into r6;
    cast r6 r5.total_pushed r5.total_reward into r7 as StakerState;
    set r7 into state[true];

function unbond:
    input r0 as u64.public;
    assert.neq r0 0u64;
    call credits.aleo/unbond_public staker_v1_b.aleo r0 into r1;
    async unbond self.caller r1 into r2;
    output r2 as staker_v1_b.aleo/unbond.future;
finalize unbond:
    input r0 as address.public;
    input r1 as credits.aleo/unbond_public.future;
    await r1;
    get operators[r0] into r2;
    assert.eq r2 true;

function claim:
    call credits.aleo/claim_unbond_public staker_v1_b.aleo into r0;
    async claim self.caller r0 into r1;
    output r1 as staker_v1_b.aleo/claim.future;
finalize claim:
    input r0 as address.public;
    input r1 as credits.aleo/claim_unbond_public.future;
    await r1;
    get operators[r0] into r2;
    assert.eq r2 true;

function push_aleo:
    input r0 as u64.public;
    assert.neq r0 0u64;
    call credits.aleo/transfer_public staking_v1.aleo r0 into r1;
    async push_aleo self.caller r0 r1 into r2;
    output r2 as staker_v1_b.aleo/push_aleo.future;
finalize push_aleo:
    input r0 as address.public;
    input r1 as u64.public;
    input r2 as credits.aleo/transfer_public.future;
    await r2;
    get operators[r0] into r3;
    assert.eq r3 true;
    get state[true] into r4;
    add r4.total_pushed r1 into r5;
    cast r4.total_pulled r5 r4.total_reward into r6 as StakerState;
    set r6 into state[true];

function claim_and_push:
    input r0 as u64.public;
    call credits.aleo/claim_unbond_public staker_v1_b.aleo into r1;
    call credits.aleo/transfer_public staking_v1.aleo r0 into r2;
    async claim_and_push self.caller r0 r1 r2 into r3;
    output r3 as staker_v1_b.aleo/claim_and_push.future;
finalize claim_and_push:
    input r0 as address.public;
    input r1 as u64.public;
    input r2 as credits.aleo/claim_unbond_public.future;
    input r3 as credits.aleo/transfer_public.future;
    await r2;
    await r3;
    get operators[r0] into r4;
    assert.eq r4 true;
    get state[true] into r5;
    add r5.total_pushed r1 into r6;
    cast r5.total_pulled r6 r5.total_reward into r7 as StakerState;
    set r7 into state[true];

function set_operator:
    input r0 as address.public;
    input r1 as boolean.public;
    async set_operator self.caller r0 r1 into r2;
    output r2 as staker_v1_b.aleo/set_operator.future;
finalize set_operator:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as boolean.public;
    get admins[r0] into r3;
    assert.eq r3 true;
    set r2 into operators[r1];
        ",
    )
    .unwrap();

    let prelim_program_3 = Program::<CurrentNetwork>::from_str(
        r"
program multisig_v1.aleo;

struct Operation:
    program_id as address;
    op_type as u8;
    params as field;
    op_salt as u64;
    delay as u32;

struct Request:
    operation as Operation;
    multisig as address;

struct ReqState:
    state as u8;
    active_block as u32;

mapping admins:
    key as address.public;
    value as boolean.public;

mapping initialized:
    key as u8.public;
    value as boolean.public;

mapping requests:
    key as field.public;
    value as ReqState.public;

function init:
    input r0 as [address; 5u32].public;
    assert.neq r0[0u32] aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc;
    assert.neq r0[1u32] aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc;
    assert.neq r0[2u32] aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc;
    assert.neq r0[3u32] aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc;
    assert.neq r0[4u32] aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc;
    assert.neq r0[0u32] r0[1u32];
    assert.neq r0[0u32] r0[2u32];
    assert.neq r0[0u32] r0[3u32];
    assert.neq r0[0u32] r0[4u32];
    assert.neq r0[1u32] r0[2u32];
    assert.neq r0[1u32] r0[3u32];
    assert.neq r0[1u32] r0[4u32];
    assert.neq r0[2u32] r0[3u32];
    assert.neq r0[2u32] r0[4u32];
    assert.neq r0[3u32] r0[4u32];
    async init r0 into r1;
    output r1 as multisig_v1.aleo/init.future;
finalize init:
    input r0 as [address; 5u32].public;
    contains initialized[1u8] into r1;
    assert.eq r1 false;
    set true into initialized[1u8];
    set true into admins[r0[0u32]];
    set true into admins[r0[1u32]];
    set true into admins[r0[2u32]];
    set true into admins[r0[3u32]];
    set true into admins[r0[4u32]];

closure verify_signatures:
    input r0 as Request;
    input r1 as [address; 3u32];
    input r2 as [signature; 3u32];
    assert.neq r1[0u32] r1[1u32];
    assert.neq r1[0u32] r1[2u32];
    assert.neq r1[1u32] r1[2u32];
    hash.bhp256 r0 into r3 as field;
    sign.verify r2[0u32] r1[0u32] r3 into r4;
    assert.eq r4 true;
    sign.verify r2[1u32] r1[1u32] r3 into r5;
    assert.eq r5 true;
    sign.verify r2[2u32] r1[2u32] r3 into r6;
    assert.eq r6 true;
    output r3 as field;

function change_admin:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as u64.private;
    input r3 as [address; 3u32].public;
    input r4 as [signature; 3u32].private;
    assert.neq r0 r1;
    assert.neq r1 aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc;
    cast r0 r1 into r5 as [address; 2u32];
    hash.bhp256 r5 into r6 as field;
    cast multisig_v1.aleo 1u8 r6 r2 0u32 into r7 as Operation;
    cast r7 multisig_v1.aleo into r8 as Request;
    call verify_signatures r8 r3 r4 into r9;
    async change_admin r0 r1 r9 r3 into r10;
    output r10 as multisig_v1.aleo/change_admin.future;
finalize change_admin:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as field.public;
    input r3 as [address; 3u32].public;
    get admins[r3[0u32]] into r4;
    assert.eq r4 true;
    get admins[r3[1u32]] into r5;
    assert.eq r5 true;
    get admins[r3[2u32]] into r6;
    assert.eq r6 true;
    contains requests[r2] into r7;
    assert.eq r7 false;
    cast 1u8 block.height into r8 as ReqState;
    set r8 into requests[r2];
    get admins[r0] into r9;
    assert.eq r9 true;
    get.or_use admins[r1] false into r10;
    assert.eq r10 false;
    remove admins[r0];
    set true into admins[r1];

function new_request:
    input r0 as Operation.public;
    input r1 as boolean.public;
    input r2 as [address; 3u32].public;
    input r3 as [signature; 3u32].private;
    cast r0 multisig_v1.aleo into r4 as Request;
    call verify_signatures r4 r2 r3 into r5;
    not r1 into r6;
    is.eq r0.program_id self.caller into r7;
    or r6 r7 into r8;
    assert.eq r8 true;
    not r1 into r9;
    is.eq r0.delay 0u32 into r10;
    or r9 r10 into r11;
    assert.eq r11 true;
    async new_request r1 r0.delay r5 r2 into r12;
    output r12 as multisig_v1.aleo/new_request.future;
finalize new_request:
    input r0 as boolean.public;
    input r1 as u32.public;
    input r2 as field.public;
    input r3 as [address; 3u32].public;
    get admins[r3[0u32]] into r4;
    assert.eq r4 true;
    get admins[r3[1u32]] into r5;
    assert.eq r5 true;
    get admins[r3[2u32]] into r6;
    assert.eq r6 true;
    contains requests[r2] into r7;
    assert.eq r7 false;
    ternary r0 1u8 0u8 into r8;
    add block.height r1 into r9;
    cast r8 r9 into r10 as ReqState;
    set r10 into requests[r2];

function cancel_request:
    input r0 as field.public;
    input r1 as [address; 3u32].public;
    input r2 as [signature; 3u32].private;
    cast multisig_v1.aleo 2u8 r0 0u64 0u32 into r3 as Operation;
    cast r3 multisig_v1.aleo into r4 as Request;
    call verify_signatures r4 r1 r2 into r5;
    async cancel_request r0 r1 into r6;
    output r6 as multisig_v1.aleo/cancel_request.future;
finalize cancel_request:
    input r0 as field.public;
    input r1 as [address; 3u32].public;
    get admins[r1[0u32]] into r2;
    assert.eq r2 true;
    get admins[r1[1u32]] into r3;
    assert.eq r3 true;
    get admins[r1[2u32]] into r4;
    assert.eq r4 true;
    get requests[r0] into r5;
    assert.eq r5.state 0u8;
    cast 2u8 r5.active_block into r6 as ReqState;
    set r6 into requests[r0];

function execute:
    input r0 as Operation.public;
    assert.eq r0.program_id self.caller;
    cast r0 multisig_v1.aleo into r1 as Request;
    hash.bhp256 r1 into r2 as field;
    async execute r2 into r3;
    output r3 as multisig_v1.aleo/execute.future;
finalize execute:
    input r0 as field.public;
    get requests[r0] into r1;
    assert.eq r1.state 0u8;
    lte r1.active_block block.height into r2;
    assert.eq r2 true;
    cast 1u8 r1.active_block into r3 as ReqState;
    set r3 into requests[r0];
    ",
    )
    .unwrap();

    let program_1 = Program::<CurrentNetwork>::from_str(
        r"
        import credits.aleo;
import staking_v1.aleo;
import staker_v1_b.aleo;
import multisig_v1.aleo;

program staker_v1_multisig_b.aleo;

struct Settings:
    unstake_wait as u32;
    stake_paused as boolean;
    global_paused as boolean;
    max_reward_per_notify as u64;
    protocol_fee as u16;
    fee_account as address;
    fixed_stakers as boolean;

struct Operation:
    program_id as address;
    op_type as u8;
    params as field;
    op_salt as u64;
    delay as u32;

struct Request:
    operation as Operation;
    multisig as address;

struct SetOperatorParams:
    operator as address;
    flag as boolean;

function dummy:

function set_operator:
    input r0 as address.public;
    input r1 as boolean.public;
    input r2 as u64.public;
    input r3 as [address; 3u32].public;
    input r4 as [signature; 3u32].private;
    cast r0 r1 into r5 as SetOperatorParams;
    hash.bhp256 r5 into r6 as field;
    cast staker_v1_multisig_b.aleo 3u8 r6 r2 0u32 into r7 as Operation;
    call multisig_v1.aleo/new_request r7 true r3 r4 into r8;
    call staker_v1_b.aleo/set_operator r0 r1 into r9;
    async set_operator r8 r9 into r10;
    output r10 as staker_v1_multisig_b.aleo/set_operator.future;

finalize set_operator:
    input r0 as multisig_v1.aleo/new_request.future;
    input r1 as staker_v1_b.aleo/set_operator.future;
    await r0;
    await r1;

function add_admin:
    input r0 as address.public;
    input r1 as u64.public;
    hash.bhp256 r0 into r2 as field;
    cast staker_v1_multisig_b.aleo 1u8 r2 r1 1200u32 into r3 as Operation;
    call multisig_v1.aleo/execute r3 into r4;
    call staker_v1_b.aleo/set_admin r0 true into r5;
    async add_admin r4 r5 into r6;
    output r6 as staker_v1_multisig_b.aleo/add_admin.future;

finalize add_admin:
    input r0 as multisig_v1.aleo/execute.future;
    input r1 as staker_v1_b.aleo/set_admin.future;
    await r0;
    await r1;

function remove_admin:
    input r0 as address.public;
    input r1 as u64.public;
    input r2 as [address; 3u32].public;
    input r3 as [signature; 3u32].private;
    assert.neq self.caller r0 ;
    hash.bhp256 r0 into r4 as field;
    cast staker_v1_multisig_b.aleo 2u8 r4 r1 0u32 into r5 as Operation;
    call multisig_v1.aleo/new_request r5 true r2 r3 into r6;
    call staker_v1_b.aleo/set_admin r0 false into r7;
    async remove_admin r6 r7 into r8;
    output r8 as staker_v1_multisig_b.aleo/remove_admin.future;

finalize remove_admin:
    input r0 as multisig_v1.aleo/new_request.future;
    input r1 as staker_v1_b.aleo/set_admin.future;
    await r0;
    await r1;
        ",
    )
    .unwrap();

    // `program_2` is the same as `program_1` but with a different name.
    let program_2 = Program::<CurrentNetwork>::from_str(
        r"
        import credits.aleo;
import staking_v1.aleo;
import staker_v1_b.aleo;
import multisig_v1.aleo;

program staker_v2_multisig_b.aleo;

struct Settings:
    unstake_wait as u32;
    stake_paused as boolean;
    global_paused as boolean;
    max_reward_per_notify as u64;
    protocol_fee as u16;
    fee_account as address;
    fixed_stakers as boolean;

struct Operation:
    program_id as address;
    op_type as u8;
    params as field;
    op_salt as u64;
    delay as u32;

struct Request:
    operation as Operation;
    multisig as address;

struct SetOperatorParams:
    operator as address;
    flag as boolean;

function dummy:

function set_operator:
    input r0 as address.public;
    input r1 as boolean.public;
    input r2 as u64.public;
    input r3 as [address; 3u32].public;
    input r4 as [signature; 3u32].private;
    cast r0 r1 into r5 as SetOperatorParams;
    hash.bhp256 r5 into r6 as field;
    cast staker_v2_multisig_b.aleo 3u8 r6 r2 0u32 into r7 as Operation;
    call multisig_v1.aleo/new_request r7 true r3 r4 into r8;
    call staker_v1_b.aleo/set_operator r0 r1 into r9;
    async set_operator r8 r9 into r10;
    output r10 as staker_v2_multisig_b.aleo/set_operator.future;

finalize set_operator:
    input r0 as multisig_v1.aleo/new_request.future;
    input r1 as staker_v1_b.aleo/set_operator.future;
    await r0;
    await r1;

function add_admin:
    input r0 as address.public;
    input r1 as u64.public;
    hash.bhp256 r0 into r2 as field;
    cast staker_v2_multisig_b.aleo 1u8 r2 r1 1200u32 into r3 as Operation;
    call multisig_v1.aleo/execute r3 into r4;
    call staker_v1_b.aleo/set_admin r0 true into r5;
    async add_admin r4 r5 into r6;
    output r6 as staker_v2_multisig_b.aleo/add_admin.future;

finalize add_admin:
    input r0 as multisig_v1.aleo/execute.future;
    input r1 as staker_v1_b.aleo/set_admin.future;
    await r0;
    await r1;

function remove_admin:
    input r0 as address.public;
    input r1 as u64.public;
    input r2 as [address; 3u32].public;
    input r3 as [signature; 3u32].private;
    assert.neq self.caller r0 ;
    hash.bhp256 r0 into r4 as field;
    cast staker_v2_multisig_b.aleo 2u8 r4 r1 0u32 into r5 as Operation;
    call multisig_v1.aleo/new_request r5 true r2 r3 into r6;
    call staker_v1_b.aleo/set_admin r0 false into r7;
    async remove_admin r6 r7 into r8;
    output r8 as staker_v2_multisig_b.aleo/remove_admin.future;

finalize remove_admin:
    input r0 as multisig_v1.aleo/new_request.future;
    input r1 as staker_v1_b.aleo/set_admin.future;
    await r0;
    await r1;
        ",
    )
    .unwrap();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, .. } = crate::test_helpers::sample_test_env(rng);

    //PRELIM PROGRAMS:

    let prelim_1 = ledger.vm.deploy(&private_key, &prelim_program_1, None, 0, None, rng).unwrap();
    let block = ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![prelim_1], rng).unwrap();
    // Check that the block does not have any aborted transactions.
    assert!(block.aborted_transaction_ids().is_empty());
    // Check that the block does not have any rejected transactions.
    assert_eq!(block.transactions().num_rejected(), 0);
    ledger.check_next_block(&block, rng).unwrap();
    ledger.advance_to_next_block(&block).unwrap();

    let prelim_2 = ledger.vm.deploy(&private_key, &prelim_program_2, None, 0, None, rng).unwrap();
    println!("Prelim 2 is {}", prelim_2.id());
    let block = ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![prelim_2], rng).unwrap();
    // Check that the block does not have any aborted transactions.
    assert!(block.aborted_transaction_ids().is_empty());
    // Check that the block does not have any rejected transactions.
    assert_eq!(block.transactions().num_rejected(), 0);
    ledger.check_next_block(&block, rng).unwrap();
    ledger.advance_to_next_block(&block).unwrap();

    let prelim_3 = ledger.vm.deploy(&private_key, &prelim_program_3, None, 0, None, rng).unwrap();
    println!("Prelim 3 is {}", prelim_3.id());
    let block = ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![prelim_3], rng).unwrap();
    // Check that the block does not have any aborted transactions.
    assert!(block.aborted_transaction_ids().is_empty());
    // Check that the block does not have any rejected transactions.
    assert_eq!(block.transactions().num_rejected(), 0);
    ledger.check_next_block(&block, rng).unwrap();
    ledger.advance_to_next_block(&block).unwrap();

    // Deploy MAX_STACKS dummy deployments to test cache eviction.
    for i in 0..=<CurrentNetwork as Network>::MAX_STACKS {
        let program = Program::<CurrentNetwork>::from_str(&format!(
            r"
program testing{i}.aleo;

function compute:"
        ))
        .unwrap();
        let deployment = ledger.vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
        let block =
            ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![deployment], rng).unwrap();
        // Assert that the deployment was accepted.
        assert!(block.aborted_transaction_ids().is_empty());
        assert_eq!(block.transactions().num_rejected(), 0);
        ledger.check_next_block(&block, rng).unwrap();
        ledger.advance_to_next_block(&block).unwrap();
    }

    // Create a deployment transaction for the first program.
    let deployment_1 = ledger.vm.deploy(&private_key, &program_1, None, 0, None, rng).unwrap();
    let deployment_1_id = deployment_1.id();
    let deployment_is_ok = ledger.check_transaction_basic(&deployment_1, None, rng).is_ok();
    let fee_amount = deployment_1.fee_amount().unwrap();

    println!("\n\n\n@@@@@@@@@@@");
    println!("Program fee is {:?}", fee_amount);
    println!("Program deployment is valid - {deployment_is_ok}");
    println!("@@@@@@@@@@@\n\n\n");
    assert!(deployment_is_ok);

    // Create a block.
    let block =
        ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![deployment_1], rng).unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    let is_aborted = block.aborted_transaction_ids().contains(&deployment_1_id);
    let is_accepted = block.transactions().transaction_ids().contains(&deployment_1_id);

    println!("\n\n\n@@@@@@@@@@@");
    println!("Block height is {:?}", block.height());
    println!("Program fee is {:?}", fee_amount);
    println!("Program deployment is valid - {deployment_is_ok}");
    println!("Program is accepted: {is_accepted}");
    println!("Program is aborted: {is_aborted}");
    println!("@@@@@@@@@@@\n\n\n");
    assert!(is_accepted);

    // Add 10 dummy transactions executions, 1 each block.
    for i in 0..10 {
        println!("Adding dummy transaction {}", i);
        let dummy_execution = ledger
            .vm
            .execute(
                &private_key,
                ("staker_v1_multisig_b.aleo", "dummy"),
                Vec::<Value<CurrentNetwork>>::new().iter(),
                None,
                0,
                None,
                rng,
            )
            .unwrap();
        let block = ledger
            .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![dummy_execution], rng)
            .unwrap();
        // Check that the block does not have any aborted transactions.
        assert!(block.aborted_transaction_ids().is_empty());
        // Check that the block does not have any rejected transactions.
        assert_eq!(block.transactions().num_rejected(), 0);
        ledger.check_next_block(&block, rng).unwrap();
        ledger.advance_to_next_block(&block).unwrap();
    }

    // Create a deployment transaction for the second program.
    let deployment_2 = ledger.vm.deploy(&private_key, &program_2, None, 0, None, rng).unwrap();
    let deployment_2_id = deployment_2.id();
    let deployment_is_ok = ledger.check_transaction_basic(&deployment_2, None, rng).is_ok();
    let fee_amount = deployment_2.fee_amount().unwrap();

    println!("\n\n\n@@@@@@@@@@@");
    println!("Program fee is {:?}", fee_amount);
    println!("Program deployment is valid - {deployment_is_ok}");
    println!("@@@@@@@@@@@\n\n\n");
    assert!(deployment_is_ok);

    // Create a block.
    let block =
        ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![deployment_2], rng).unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    let is_aborted = block.aborted_transaction_ids().contains(&deployment_2_id);
    let is_accepted = block.transactions().transaction_ids().contains(&deployment_2_id);

    println!("\n\n\n@@@@@@@@@@@");
    println!("Block height is {:?}", block.height());
    println!("Program fee is {:?}", fee_amount);
    println!("Program deployment is valid - {deployment_is_ok}");
    println!("Program is accepted: {is_accepted}");
    println!("Program is aborted: {is_aborted}");
    println!("@@@@@@@@@@@\n\n\n");
    assert!(is_accepted);
}

#[test]
fn test_deployment_duplicate_program_id() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, view_key, .. } = crate::test_helpers::sample_test_env(rng);

    // A helper function to find records.
    let find_records = || {
        let microcredits = Identifier::from_str("microcredits").unwrap();
        ledger
            .find_records(&view_key, RecordsFilter::SlowUnspent(private_key))
            .unwrap()
            .filter(|(_, record)| match record.data().get(&microcredits) {
                Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => !amount.is_zero(),
                _ => false,
            })
            .collect::<indexmap::IndexMap<_, _>>()
    };

    // Fetch the unspent records.
    let records = find_records();
    let record_1 = records[0].clone();
    let record_2 = records[1].clone();

    // Create two programs with a duplicate program ID but different mappings
    let program_1 = Program::<CurrentNetwork>::from_str(
        r"
program dummy_program.aleo;
mapping abcd1:
    key as address.public;
    value as u64.public;
function foo:
    input r0 as u8.private;
    async foo r0 into r1;
    output r1 as dummy_program.aleo/foo.future;
finalize foo:
    input r0 as u8.public;
    add r0 r0 into r1;",
    )
    .unwrap();

    let program_2 = Program::<CurrentNetwork>::from_str(
        r"
program dummy_program.aleo;
mapping abcd2:
    key as address.public;
    value as u64.public;
function foo2:
    input r0 as u8.private;
    async foo2 r0 into r1;
    output r1 as dummy_program.aleo/foo2.future;
finalize foo2:
    input r0 as u8.public;
    add r0 r0 into r1;",
    )
    .unwrap();

    // Create a deployment transaction for the first program.
    let deployment_1 = ledger.vm.deploy(&private_key, &program_1, Some(record_1), 0, None, rng).unwrap();
    let deployment_1_id = deployment_1.id();
    assert!(ledger.check_transaction_basic(&deployment_1, None, rng).is_ok());

    // Create a deployment transaction for the second program.
    let deployment_2 = ledger.vm.deploy(&private_key, &program_2, Some(record_2), 0, None, rng).unwrap();
    let deployment_2_id = deployment_2.id();
    assert!(ledger.check_transaction_basic(&deployment_2, None, rng).is_ok());

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![deployment_1, deployment_2], rng)
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Enforce that the block transactions were correct.
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 1);

    // Enforce that the first program was deployed and the second was rejected.
    assert_eq!(ledger.get_program(*program_1.id()).unwrap(), program_1);
    assert!(ledger.vm.transaction_store().contains_transaction_id(&deployment_1_id).unwrap());
    assert!(ledger.vm.block_store().contains_rejected_or_aborted_transaction_id(&deployment_2_id).unwrap());
}

#[test]
fn test_split_candidate_solutions() {
    let rng = &mut TestRng::default();

    let max_solutions = CurrentNetwork::MAX_SOLUTIONS;

    const ITERATIONS: usize = 1_000;

    for _ in 0..ITERATIONS {
        let num_candidates = rng.gen_range(0..max_solutions * 2);
        let candidate_solutions: Vec<u8> = rng.sample_iter(Standard).take(num_candidates).collect();

        let (_accepted, _aborted) =
            split_candidate_solutions(candidate_solutions, max_solutions, |candidate| *candidate % 2 == 0);
    }
}

#[test]
fn test_max_committee_limit_with_bonds() {
    // Initialize an RNG.
    let rng = &mut TestRng::default();

    // Initialize the VM.
    let vm = sample_vm();

    // Construct the validators, one less than the maximum committee size.
    let validators = (0..Committee::<CurrentNetwork>::MAX_COMMITTEE_SIZE - 1)
        .map(|_| {
            let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
            let amount = MIN_VALIDATOR_STAKE;
            let is_open = true;
            (private_key, (amount, is_open))
        })
        .collect::<IndexMap<_, _>>();

    // Track the allocated amount.
    let mut allocated_amount = 0;

    // Construct the committee.
    let mut committee_map = IndexMap::new();
    for (private_key, (amount, _)) in &validators {
        let address = Address::try_from(private_key).unwrap();
        committee_map.insert(address, (*amount, true, 0));
        allocated_amount += *amount;
    }

    // Initialize two new validators.
    let first_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let first_address = Address::try_from(&first_private_key).unwrap();
    let first_withdrawal_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let first_withdrawal_address = Address::try_from(&first_withdrawal_private_key).unwrap();
    let second_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let second_address = Address::try_from(&second_private_key).unwrap();
    let second_withdrawal_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let second_withdrawal_address = Address::try_from(&second_withdrawal_private_key).unwrap();

    // Construct the public balances, allocating the remaining supply to the first validator and two new validators.
    // The remaining validators will have a balance of 0.
    let mut public_balances = IndexMap::new();
    for (private_key, _) in &validators {
        public_balances.insert(Address::try_from(private_key).unwrap(), 0);
    }
    let remaining_supply = <CurrentNetwork as Network>::STARTING_SUPPLY - allocated_amount;
    let amount = remaining_supply / 5;
    public_balances.insert(Address::try_from(validators.keys().next().unwrap()).unwrap(), amount);
    public_balances.insert(first_address, amount);
    public_balances.insert(first_withdrawal_address, amount);
    public_balances.insert(second_address, amount);
    public_balances.insert(second_withdrawal_address, remaining_supply - 4 * amount);

    // Construct the bonded balances.
    let bonded_balances = validators
        .iter()
        .map(|(private_key, (amount, _))| {
            let address = Address::try_from(private_key).unwrap();
            (address, (address, address, *amount))
        })
        .collect();

    // Construct the genesis block, which should pass.
    let genesis_block = vm
        .genesis_quorum(
            validators.keys().next().unwrap(),
            Committee::new_genesis(committee_map).unwrap(),
            public_balances,
            bonded_balances,
            rng,
        )
        .unwrap();

    // Initialize a Ledger from the genesis block.
    let ledger =
        Ledger::<CurrentNetwork, ConsensusMemory<CurrentNetwork>>::load(genesis_block, StorageMode::Production)
            .unwrap();

    // Bond the first validator.
    let bond_first_transaction = ledger
        .vm()
        .execute(
            &first_private_key,
            ("credits.aleo", "bond_validator"),
            vec![
                Value::<CurrentNetwork>::from_str(&first_withdrawal_address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{MIN_VALIDATOR_STAKE}u64")).unwrap(),
                Value::<CurrentNetwork>::from_str("10u8").unwrap(),
            ]
            .iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(
            validators.keys().next().unwrap(),
            vec![],
            vec![],
            vec![bond_first_transaction],
            rng,
        )
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Check that the first validator is not in the committee.
    let committee = ledger.latest_committee().unwrap();
    assert!(!committee.is_committee_member(first_address));

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Check that the first validator was added to the committee.
    let committee = ledger.latest_committee().unwrap();
    assert!(committee.is_committee_member(first_address));

    // Attempt to bond the second validator.
    let bond_second_transaction = ledger
        .vm()
        .execute(
            &second_private_key,
            ("credits.aleo", "bond_validator"),
            vec![
                Value::<CurrentNetwork>::from_str(&second_withdrawal_address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{MIN_VALIDATOR_STAKE}u64")).unwrap(),
                Value::<CurrentNetwork>::from_str("10u8").unwrap(),
            ]
            .iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(
            validators.keys().next().unwrap(),
            vec![],
            vec![],
            vec![bond_second_transaction],
            rng,
        )
        .unwrap();

    // Ensure that the `bond_second_transaction` is rejected.
    assert_eq!(block.transactions().num_rejected(), 1);

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Check that the second validator is not in the committee.
    let committee = ledger.latest_committee().unwrap();
    assert!(!committee.is_committee_member(second_address));

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Check that the second validator was not added to the committee.
    let committee = ledger.latest_committee().unwrap();
    assert!(!committee.is_committee_member(second_address));

    // Check that unbonding a validator first allows the second validator to bond in.

    let unbond_first_validator = ledger
        .vm()
        .execute(
            &first_withdrawal_private_key,
            ("credits.aleo", "unbond_public"),
            vec![
                Value::<CurrentNetwork>::from_str(&first_address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{MIN_VALIDATOR_STAKE}u64")).unwrap(),
            ]
            .iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();

    // Attempt to bond the second validator.
    let bond_second_validator = ledger
        .vm()
        .execute(
            &second_private_key,
            ("credits.aleo", "bond_validator"),
            vec![
                Value::<CurrentNetwork>::from_str(&second_withdrawal_address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{MIN_VALIDATOR_STAKE}u64")).unwrap(),
                Value::<CurrentNetwork>::from_str("10u8").unwrap(),
            ]
            .iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(
            validators.keys().next().unwrap(),
            vec![],
            vec![],
            vec![unbond_first_validator, bond_second_validator],
            rng,
        )
        .unwrap();

    // Ensure that no transactions are rejected.
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.transactions().num_accepted(), 2);

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Check that the first validator is in the committee and the second validator is not.
    let committee = ledger.latest_committee().unwrap();
    assert!(!committee.is_committee_member(second_address));
    assert!(committee.is_committee_member(first_address));

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Check that the first validator was removed and the second validator was added to the committee.
    let committee = ledger.latest_committee().unwrap();
    assert!(committee.is_committee_member(second_address));
    assert!(!committee.is_committee_member(first_address));
}

#[test]
fn test_deployment_exceeding_max_transaction_spend() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, .. } = crate::test_helpers::sample_test_env(rng);

    // Construct two programs, one that is allowed and one that exceeds the maximum transaction spend.
    let mut allowed_program = None;
    let mut exceeding_program = None;

    for i in 0..<CurrentNetwork as Network>::MAX_COMMANDS.ilog2() {
        // Construct the finalize body.
        let finalize_body =
            (0..2.pow(i)).map(|i| format!("hash.bhp256 0field into r{i} as field;")).collect::<Vec<_>>().join("\n");

        // Construct the program.
        let program = Program::from_str(&format!(
            r"program test_max_spend_limit_{i}.aleo;
          function foo:
          async foo into r0;
          output r0 as test_max_spend_limit_{i}.aleo/foo.future;

          finalize foo:{finalize_body}",
        ))
        .unwrap();

        // Attempt to initialize a `Stack` for the program.
        // If this fails, then by `Stack::initialize` the finalize cost exceeds the `TRANSACTION_SPEND_LIMIT`.
        if Stack::<CurrentNetwork>::new(&ledger.vm().process().read(), &program).is_err() {
            exceeding_program = Some(program);
            break;
        } else {
            allowed_program = Some(program);
        }
    }

    // Ensure that the allowed and exceeding programs are not None.
    assert!(allowed_program.is_some());
    assert!(exceeding_program.is_some());

    let allowed_program = allowed_program.unwrap();
    let exceeding_program = exceeding_program.unwrap();

    // Deploy the allowed program.
    let deployment = ledger.vm().deploy(&private_key, &allowed_program, None, 0, None, rng).unwrap();

    // Verify the deployment transaction.
    assert!(ledger.vm().check_transaction(&deployment, None, rng).is_ok());

    // Construct the next block.
    let block =
        ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![deployment], rng).unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Check that the program exists in the VM.
    assert!(ledger.vm().contains_program(allowed_program.id()));

    // Attempt to deploy the exceeding program.
    let result = ledger.vm().deploy(&private_key, &exceeding_program, None, 0, None, rng);

    // Check that the deployment failed.
    assert!(result.is_err());
}

#[test]
fn test_transaction_ordering() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, address, .. } = crate::test_helpers::sample_test_env(rng);

    // Get the public balance of the address.
    let mut public_balance = match ledger.genesis_block.ratifications().iter().next().unwrap() {
        Ratify::Genesis(_, public_balance, _) => *public_balance.get(&address).unwrap(),
        _ => panic!("Expected a genesis ratification"),
    };

    // Sample multiple private keys and addresses.
    let private_key_2 = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let address_2 = Address::try_from(&private_key_2).unwrap();
    let private_key_3 = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let address_3 = Address::try_from(&private_key_3).unwrap();

    // Fund a new address.
    let amount_1 = 100000000u64;
    let inputs =
        [Value::from_str(&format!("{address_2}")).unwrap(), Value::from_str(&format!("{amount_1}u64")).unwrap()];
    let transfer_1 = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
        .unwrap();

    let amount_2 = 100000000u64;
    let inputs =
        [Value::from_str(&format!("{address_3}")).unwrap(), Value::from_str(&format!("{amount_2}u64")).unwrap()];
    let transfer_2 = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
        .unwrap();

    // Update the public balance.
    public_balance -= *transfer_1.fee_amount().unwrap();
    public_balance -= *transfer_2.fee_amount().unwrap();

    // Create a block.
    let block = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![transfer_1, transfer_2], rng)
        .unwrap();

    // Check that the next block is valid.
    ledger.check_next_block(&block, rng).unwrap();

    // Add the block to the ledger.
    ledger.advance_to_next_block(&block).unwrap();

    // Create multiple dummy programs.
    let program_1 = Program::<CurrentNetwork>::from_str(
        r"
program dummy_program.aleo;
function foo:
    input r0 as u8.private;
    async foo r0 into r1;
    output r1 as dummy_program.aleo/foo.future;
finalize foo:
    input r0 as u8.public;
    add r0 r0 into r1;",
    )
    .unwrap();

    let program_2 = Program::<CurrentNetwork>::from_str(
        r"
program dummy_program_2.aleo;
function foo:
    input r0 as u8.private;
    async foo r0 into r1;
    output r1 as dummy_program_2.aleo/foo.future;
finalize foo:
    input r0 as u8.public;
    add r0 r0 into r1;",
    )
    .unwrap();

    let program_3 = Program::<CurrentNetwork>::from_str(
        r"
program dummy_program_3.aleo;
function foo:
    input r0 as u8.private;
    async foo r0 into r1;
    output r1 as dummy_program_3.aleo/foo.future;
finalize foo:
    input r0 as u8.public;
    add r0 r0 into r1;",
    )
    .unwrap();

    // Create a transfer transaction.
    let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("1000000u64").unwrap()];
    let initial_transfer = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
        .unwrap();
    let initial_transfer_id = initial_transfer.id();

    // Create a deployment transaction.
    let deployment_transaction = ledger.vm.deploy(&private_key_2, &program_1, None, 0, None, rng).unwrap();

    // Create a deployment transaction.
    let deployment_transaction_2 = ledger.vm.deploy(&private_key_3, &program_2, None, 0, None, rng).unwrap();

    // Create a transfer transaction.
    let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("1000000u64").unwrap()];
    let transfer_transaction = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
        .unwrap();

    // Create a rejected transfer transaction.
    let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("1000000000000000u64").unwrap()];
    let rejected_transfer = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
        .unwrap();

    // Create an aborted transfer transaction.
    let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("10u64").unwrap()];
    let aborted_transfer = ledger
        .vm
        .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, public_balance - 10, None, rng)
        .unwrap();

    // Create an aborted deployment transaction.
    let aborted_deployment = ledger.vm.deploy(&private_key, &program_3, None, public_balance - 10, None, rng).unwrap();

    const ITERATIONS: usize = 100;
    for _ in 0..ITERATIONS {
        // Create a random order for the transactions.
        let mut transactions = vec![
            deployment_transaction.clone(),
            deployment_transaction_2.clone(),
            transfer_transaction.clone(),
            rejected_transfer.clone(),
        ];
        transactions.shuffle(rng);

        // Get the confirmed transaction IDs.
        let mut confirmed_transaction_ids = transactions.iter().map(Transaction::id).collect::<Vec<_>>();

        // Randomly insert the aborted transactions.
        let mut aborted_transactions = vec![aborted_transfer.clone(), aborted_deployment.clone()];
        aborted_transactions.shuffle(rng);

        // Randomly insert the aborted transactions.
        let start_position = rng.gen_range(0..=transactions.len());
        for (index, element) in aborted_transactions.iter().enumerate() {
            transactions.insert(start_position + index, element.clone());
        }

        // Get the aborted transaction IDs.
        let aborted_transaction_ids = aborted_transactions.iter().map(Transaction::id).collect::<Vec<_>>();

        // Add the initial transfer to the list of transactions.
        transactions.insert(0, initial_transfer.clone());
        confirmed_transaction_ids.insert(0, initial_transfer_id);

        // Create a block.
        let block =
            ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], transactions, rng).unwrap();

        // Check that the next block is valid.
        ledger.check_next_block(&block, rng).unwrap();

        // Enforce that the block transactions were correct.
        assert_eq!(block.transactions().num_accepted(), 4);
        assert_eq!(block.transactions().num_rejected(), 1);

        // Enforce that the ordering of the transactions is correct.
        let block_confirmed_transactions_ids: Vec<_> = block
            .transactions()
            .iter()
            .map(|transaction| transaction.to_unconfirmed_transaction_id().unwrap())
            .collect();
        assert_eq!(block_confirmed_transactions_ids, confirmed_transaction_ids);

        // Enforce that the aborted transactions is correct.
        assert_eq!(block.aborted_transaction_ids(), &aborted_transaction_ids);
    }
}

#[test]
fn test_metadata() {
    let rng = &mut TestRng::default();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, .. } = crate::test_helpers::sample_test_env(rng);

    // Deploy a test program to the ledger.
    let program_id = ProgramID::<CurrentNetwork>::from_str("metadata.aleo").unwrap();
    let program = Program::<CurrentNetwork>::from_str(&format!(
        "
program {program_id};
function is_block:
    input r0 as u32.public;
    async is_block r0 into r1;
    output r1 as {program_id}/is_block.future;

finalize is_block:
    input r0 as u32.public;
    assert.eq r0 block.height;

function is_id:
    input r0 as u16.public;
    async is_id r0 into r1;
    output r1 as {program_id}/is_id.future;

finalize is_id:
    input r0 as u16.public;
    assert.eq r0 network.id;
    ",
    ))
    .unwrap();

    // Deploy.
    let transaction = ledger.vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
    // Verify.
    ledger.vm().check_transaction(&transaction, None, rng).unwrap();

    // Construct the next block.
    let block =
        ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![transaction], rng).unwrap();
    // Advance to the next block.
    ledger.advance_to_next_block(&block).unwrap();
    assert_eq!(ledger.latest_height(), 1);
    assert_eq!(ledger.latest_hash(), block.hash());
    assert_eq!(program, ledger.get_program(program_id).unwrap());

    // Execute functions `is_block` and `is_id` to assert that the on-chain state is as expected.
    let inputs_block: [Value<CurrentNetwork>; 1] = [Value::from_str("2u32").unwrap()];
    let tx_block =
        ledger.vm.execute(&private_key, (&program_id, "is_block"), inputs_block.iter(), None, 0, None, rng).unwrap();
    let inputs_id: [Value<CurrentNetwork>; 1] = [Value::from(Literal::U16(U16::new(CurrentNetwork::ID)))];
    let tx_id = ledger.vm.execute(&private_key, (&program_id, "is_id"), inputs_id.iter(), None, 0, None, rng).unwrap();

    // Construct the next block.
    let block_2 =
        ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![tx_id, tx_block], rng).unwrap();
    // Advance to the next block.
    ledger.advance_to_next_block(&block_2).unwrap();

    // Execute the program.
    let inputs_block_2: [Value<CurrentNetwork>; 1] = [Value::from_str("3u32").unwrap()];
    let tx_block_2 =
        ledger.vm.execute(&private_key, (&program_id, "is_block"), inputs_block_2.iter(), None, 0, None, rng).unwrap();
    let tx_id_2 =
        ledger.vm.execute(&private_key, (&program_id, "is_id"), inputs_id.iter(), None, 0, None, rng).unwrap();

    // Construct the next block.
    let block_3 = ledger
        .prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![tx_block_2, tx_id_2], rng)
        .unwrap();
    // Advance to the next block.
    ledger.advance_to_next_block(&block_3).unwrap();
}

#[test]
fn test_deployment_with_cast_from_field_to_scalar() {
    // Initialize an RNG.
    let rng = &mut TestRng::default();

    const ITERATIONS: usize = 10;

    // Construct a program that casts a field to a scalar.
    let program = Program::<CurrentNetwork>::from_str(
        r"
program test_cast_field_to_scalar.aleo;
function foo:
    input r0 as field.public;
    cast r0 into r1 as scalar;",
    )
    .unwrap();

    // Constructs a program that has a struct with a field that is cast to a scalar.
    let program_2 = Program::<CurrentNetwork>::from_str(
        r"
program test_cast_f_to_s_struct.aleo;

struct message:
    first as scalar;

function foo:
    input r0 as field.public;
    cast r0 into r1 as scalar;
    cast r1 into r2 as message;",
    )
    .unwrap();

    // Constructs a program that has an array of scalars cast from fields.
    let program_3 = Program::<CurrentNetwork>::from_str(
        r"
program test_cast_f_to_s_array.aleo;

function foo:
    input r0 as field.public;
    cast r0 into r1 as scalar;
    cast r1 r1 r1 r1 into r2 as [scalar; 4u32];",
    )
    .unwrap();

    // Initialize the test environment.
    let crate::test_helpers::TestEnv { ledger, private_key, .. } = crate::test_helpers::sample_test_env(rng);

    // Create a helper method to deploy the programs.
    let deploy_program = |program: &Program<CurrentNetwork>, rng: &mut TestRng| {
        let mut attempts = 0;
        loop {
            if attempts >= ITERATIONS {
                panic!("Failed to craft deployment after {ITERATIONS} attempts");
            }
            match try_vm_runtime!(|| ledger.vm().deploy(&private_key, program, None, 0, None, rng)) {
                Ok(result) => break result.unwrap(),
                Err(_) => attempts += 1,
            }
        }
    };

    // Deploy the programs. Keep attempting to create a deployment until it is successful.
    let deployment_tx = deploy_program(&program, rng);
    let deployment_tx_2 = deploy_program(&program_2, rng);
    let deployment_tx_3 = deploy_program(&program_3, rng);

    // Verify the deployment under different RNGs to ensure the deployment is valid.
    for _ in 0..ITERATIONS {
        let process = ledger.vm().process().clone();
        // Create a helper method to verify the deployments.
        let verify_deployment = |deployment_tx: &Transaction<CurrentNetwork>, rng: &mut TestRng| {
            let expected_result = match try_vm_runtime!(|| ledger.vm().check_transaction(deployment_tx, None, rng)) {
                Ok(result) => result.is_ok(),
                Err(_) => false,
            };
            let deployment = deployment_tx.deployment().unwrap().clone();
            for _ in 0..ITERATIONS {
                let result =
                    match try_vm_runtime!(|| process.read().verify_deployment::<CurrentAleo, _>(&deployment, rng)) {
                        Ok(result) => result.is_ok(),
                        Err(_) => false,
                    };
                assert_eq!(result, expected_result);
            }
        };

        // Verify the deployments.
        verify_deployment(&deployment_tx, rng);
        verify_deployment(&deployment_tx_2, rng);
        verify_deployment(&deployment_tx_3, rng);
    }
}

// These tests require the proof targets to be low enough to be able to generate **valid** solutions.
// This requires the 'test' feature to be enabled for the `console` dependency.
#[cfg(feature = "test")]
mod valid_solutions {
    use super::*;
    use ledger_puzzle::Solution;
    use rand::prelude::SliceRandom;
    use std::collections::HashSet;

    #[test]
    fn test_duplicate_solution_ids() {
        // Initialize an RNG.
        let rng = &mut TestRng::default();

        // Initialize the test environment.
        let crate::test_helpers::TestEnv { ledger, private_key, address, .. } =
            crate::test_helpers::sample_test_env(rng);

        // Retrieve the puzzle parameters.
        let puzzle = ledger.puzzle();
        let latest_epoch_hash = ledger.latest_epoch_hash().unwrap();
        let minimum_proof_target = ledger.latest_proof_target();

        // Create a solution that is greater than the minimum proof target.
        let mut valid_solution = puzzle.prove(latest_epoch_hash, address, rng.gen(), None).unwrap();
        while puzzle.get_proof_target(&valid_solution).unwrap() < minimum_proof_target {
            println!(
                "Solution is invalid: {} < {}",
                puzzle.get_proof_target(&valid_solution).unwrap(),
                minimum_proof_target
            );
            valid_solution = puzzle.prove(latest_epoch_hash, address, rng.gen(), None).unwrap();
        }

        // Create a valid transaction for the block.
        let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("10u64").unwrap()];
        let transfer_transaction = ledger
            .vm
            .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
            .unwrap();

        // Check that block creation fails when duplicate solution IDs are provided.
        let result = ledger.prepare_advance_to_next_beacon_block(
            &private_key,
            vec![],
            vec![valid_solution, valid_solution],
            vec![transfer_transaction.clone()],
            rng,
        );
        assert!(result.is_err());

        // Create a block.
        let block = ledger
            .prepare_advance_to_next_beacon_block(
                &private_key,
                vec![],
                vec![valid_solution],
                vec![transfer_transaction],
                rng,
            )
            .unwrap();

        // Check that the next block is valid.
        ledger.check_next_block(&block, rng).unwrap();

        // Add the deployment block to the ledger.
        ledger.advance_to_next_block(&block).unwrap();

        // Enforce that the block solution was accepted properly.
        assert_eq!(block.solutions().len(), 1);
        assert_eq!(block.aborted_solution_ids().len(), 0)
    }

    #[test]
    fn test_cumulative_proof_target_correctness() {
        // The number of blocks to test.
        const NUM_BLOCKS: u32 = 25;

        // Initialize an RNG.
        let rng = &mut TestRng::default();

        // Initialize the test environment.
        let crate::test_helpers::TestEnv { ledger, private_key, address, .. } =
            crate::test_helpers::sample_test_env(rng);

        // Retrieve the puzzle parameters.
        let puzzle = ledger.puzzle();

        // Initialize block height.
        let mut block_height = ledger.latest_height();

        // Start a local counter of proof targets.
        let mut combined_targets = 0;

        // Run through 25 blocks of target adjustment.
        while block_height < NUM_BLOCKS {
            // Get coinbase puzzle data from the latest block.
            let block = ledger.latest_block();
            let coinbase_target = block.coinbase_target();
            let coinbase_threshold = coinbase_target.saturating_div(2);
            let latest_epoch_hash = ledger.latest_epoch_hash().unwrap();
            let latest_proof_target = ledger.latest_proof_target();

            // Sample the number of solutions to generate.
            let num_solutions = rng.gen_range(1..=CurrentNetwork::MAX_SOLUTIONS);

            // Initialize a vector for valid solutions for this block.
            let mut solutions = Vec::with_capacity(num_solutions);

            // Loop through proofs until two that meet the threshold are found.
            loop {
                if let Ok(solution) = puzzle.prove(latest_epoch_hash, address, rng.gen(), Some(latest_proof_target)) {
                    // Get the proof target.
                    let proof_target = puzzle.get_proof_target(&solution).unwrap();

                    // Update the local combined target counter and store the solution.
                    combined_targets += proof_target;
                    solutions.push(solution);

                    // If two have been found, exit the solver loop.
                    if solutions.len() >= num_solutions {
                        break;
                    }
                }
            }

            // If the combined target exceeds the coinbase threshold reset it.
            if combined_targets >= coinbase_threshold {
                combined_targets = 0;
            }

            // Get a transfer transaction to ensure solutions can be included in the block.
            let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("10u64").unwrap()];
            let transfer_transaction = ledger
                .vm
                .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
                .unwrap();

            // Generate the next prospective block.
            let next_block = ledger
                .prepare_advance_to_next_beacon_block(
                    &private_key,
                    vec![],
                    solutions,
                    vec![transfer_transaction.clone()],
                    rng,
                )
                .unwrap();

            // Ensure the combined target matches the expected value.
            assert_eq!(combined_targets as u128, next_block.cumulative_proof_target());

            // Ensure the next block is correct.
            ledger.check_next_block(&next_block, rng).unwrap();

            // Advanced to the next block.
            ledger.advance_to_next_block(&next_block).unwrap();

            // Set the latest block height.
            block_height = ledger.latest_height();
        }
    }

    #[test]
    fn test_excess_invalid_solution_ids() {
        // Note that the sum of `NUM_INVALID_SOLUTIONS` and `NUM_VALID_SOLUTIONS` should exceed the maximum number of solutions.
        const NUM_INVALID_SOLUTIONS: usize = CurrentNetwork::MAX_SOLUTIONS;
        const NUM_VALID_SOLUTIONS: usize = CurrentNetwork::MAX_SOLUTIONS;

        // Initialize an RNG.
        let rng = &mut TestRng::default();

        // Initialize the test environment.
        let crate::test_helpers::TestEnv { ledger, private_key, address, .. } =
            crate::test_helpers::sample_test_env(rng);

        // Retrieve the puzzle parameters.
        let puzzle = ledger.puzzle();
        let latest_epoch_hash = ledger.latest_epoch_hash().unwrap();
        let minimum_proof_target = ledger.latest_proof_target();

        // Initialize storage for the valid and invalid solutions
        let mut valid_solutions = Vec::with_capacity(NUM_VALID_SOLUTIONS);
        let mut invalid_solutions = Vec::with_capacity(NUM_INVALID_SOLUTIONS);

        // Create solutions that are greater than the minimum proof target.
        while valid_solutions.len() < NUM_VALID_SOLUTIONS {
            let solution = puzzle.prove(latest_epoch_hash, address, rng.gen(), None).unwrap();
            if puzzle.get_proof_target(&solution).unwrap() < minimum_proof_target {
                if invalid_solutions.len() < NUM_INVALID_SOLUTIONS {
                    invalid_solutions.push(solution);
                }
            } else {
                valid_solutions.push(solution);
            }
        }
        // Create the remaining solutions that are less than the minimum proof target.
        while invalid_solutions.len() < NUM_INVALID_SOLUTIONS {
            let solution = puzzle.prove(latest_epoch_hash, address, rng.gen(), None).unwrap();
            if puzzle.get_proof_target(&solution).unwrap() < minimum_proof_target {
                invalid_solutions.push(solution);
            }
        }

        // Check the length of the valid and invalid solutions.
        assert_eq!(valid_solutions.len(), NUM_VALID_SOLUTIONS);
        assert_eq!(invalid_solutions.len(), NUM_INVALID_SOLUTIONS);

        // Concatenate and shuffle the solutions.
        let mut candidate_solutions = valid_solutions.clone();
        candidate_solutions.extend(invalid_solutions.clone());
        candidate_solutions.shuffle(rng);

        // Create a valid transaction for the block.
        let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("10u64").unwrap()];
        let transfer_transaction = ledger
            .vm
            .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
            .unwrap();

        // Create a block.
        let block = ledger
            .prepare_advance_to_next_beacon_block(
                &private_key,
                vec![],
                candidate_solutions,
                vec![transfer_transaction],
                rng,
            )
            .unwrap();

        // Check that the next block is valid.
        ledger.check_next_block(&block, rng).unwrap();

        // Add the deployment block to the ledger.
        ledger.advance_to_next_block(&block).unwrap();

        // Check that the block's solutions are well-formed.
        assert_eq!(block.aborted_solution_ids().len(), NUM_INVALID_SOLUTIONS);
        assert_eq!(block.solutions().len(), NUM_VALID_SOLUTIONS);

        let block_solutions = block.solutions().solution_ids().cloned().collect::<HashSet<_>>();
        let valid_solutions = valid_solutions.iter().map(|s| s.id()).collect::<HashSet<_>>();
        assert_eq!(block_solutions, valid_solutions, "Valid solutions do not match");

        let block_aborted_solution_ids = block.aborted_solution_ids().iter().cloned().collect::<HashSet<_>>();
        let invalid_solutions = invalid_solutions.iter().map(|s| s.id()).collect::<HashSet<_>>();
        assert_eq!(block_aborted_solution_ids, invalid_solutions, "Invalid solutions do not match");
    }

    #[test]
    fn test_excess_valid_solution_ids() {
        // Note that this should be greater than the maximum number of solutions.
        const NUM_VALID_SOLUTIONS: usize = 2 * CurrentNetwork::MAX_SOLUTIONS;

        // Initialize an RNG.
        let rng = &mut TestRng::default();

        // Initialize the test environment.
        let crate::test_helpers::TestEnv { ledger, private_key, address, .. } =
            crate::test_helpers::sample_test_env(rng);

        // Retrieve the puzzle parameters.
        let puzzle = ledger.puzzle();
        let latest_epoch_hash = ledger.latest_epoch_hash().unwrap();
        let minimum_proof_target = ledger.latest_proof_target();

        // Initialize storage for the valid solutions
        let mut valid_solutions = Vec::with_capacity(NUM_VALID_SOLUTIONS);

        // Create solutions that are greater than the minimum proof target.
        while valid_solutions.len() < NUM_VALID_SOLUTIONS {
            let solution = puzzle.prove(latest_epoch_hash, address, rng.gen(), None).unwrap();
            if puzzle.get_proof_target(&solution).unwrap() >= minimum_proof_target {
                valid_solutions.push(solution);
            }
        }

        // Check the length of the valid solutions.
        assert_eq!(valid_solutions.len(), NUM_VALID_SOLUTIONS);

        // Shuffle the solutions.
        let mut candidate_solutions = valid_solutions;
        candidate_solutions.shuffle(rng);

        // Create a valid transaction for the block.
        let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("10u64").unwrap()];
        let transfer_transaction = ledger
            .vm
            .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
            .unwrap();

        // Create a block.
        let block = ledger
            .prepare_advance_to_next_beacon_block(
                &private_key,
                vec![],
                candidate_solutions.clone(),
                vec![transfer_transaction],
                rng,
            )
            .unwrap();

        // Check that the next block is valid.
        ledger.check_next_block(&block, rng).unwrap();

        // Add the deployment block to the ledger.
        ledger.advance_to_next_block(&block).unwrap();

        // Check that the block's solutions are well-formed.
        assert_eq!(block.solutions().len(), CurrentNetwork::MAX_SOLUTIONS);
        assert_eq!(block.aborted_solution_ids().len(), NUM_VALID_SOLUTIONS - CurrentNetwork::MAX_SOLUTIONS);

        let block_solutions = block.solutions().solution_ids().cloned().collect::<HashSet<_>>();
        let expected_accepted_solutions =
            candidate_solutions.iter().take(CurrentNetwork::MAX_SOLUTIONS).map(|s| s.id()).collect::<HashSet<_>>();
        assert_eq!(block_solutions, expected_accepted_solutions, "Accepted solutions do not match");

        let block_aborted_solution_ids = block.aborted_solution_ids().iter().cloned().collect::<HashSet<_>>();
        let expected_aborted_solutions =
            candidate_solutions.iter().skip(CurrentNetwork::MAX_SOLUTIONS).map(|s| s.id()).collect::<HashSet<_>>();
        assert_eq!(block_aborted_solution_ids, expected_aborted_solutions, "Aborted solutions do not match");
    }

    #[test]
    fn test_malicious_solution() {
        // Initialize an RNG.
        let rng = &mut TestRng::default();

        // Initialize the test environment.
        let crate::test_helpers::TestEnv { ledger, private_key, address, .. } =
            crate::test_helpers::sample_test_env(rng);

        // Retrieve the puzzle parameters.
        let puzzle = ledger.puzzle();
        let latest_epoch_hash = ledger.latest_epoch_hash().unwrap();
        let minimum_proof_target = ledger.latest_proof_target();

        // Initialize a valid solution object.
        let mut valid_solution = None;
        while valid_solution.is_none() {
            let solution = puzzle.prove(latest_epoch_hash, address, rng.gen(), None).unwrap();
            if puzzle.get_proof_target(&solution).unwrap() >= minimum_proof_target {
                valid_solution = Some(solution);
            }
        }
        // Unwrap the valid solution.
        let valid_solution = valid_solution.unwrap();

        // Construct a malicious solution with a different target.
        let different_target = valid_solution.target().wrapping_sub(1);
        let malicious_solution = Solution::new(*valid_solution.partial_solution(), different_target);

        assert_eq!(
            valid_solution.id(),
            malicious_solution.id(),
            "The malicious solution should have the same ID as the valid solution"
        );
        assert_ne!(
            valid_solution.target(),
            malicious_solution.target(),
            "The malicious solution should have a different target than the valid solution"
        );

        // Create a valid transaction for the block.
        let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("10u64").unwrap()];
        let transfer_transaction = ledger
            .vm
            .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
            .unwrap();

        // Check that the block creation fixes the malformed solution.
        let expected_solution_id = valid_solution.id();
        let expected_solution = valid_solution;
        let mut check_block = |candidate_solutions: Vec<Solution<CurrentNetwork>>| {
            // Create a block.
            let block = ledger
                .prepare_advance_to_next_beacon_block(
                    &private_key,
                    vec![],
                    candidate_solutions.clone(),
                    vec![transfer_transaction.clone()],
                    rng,
                )
                .unwrap();

            // Check that the next block is valid.
            ledger.check_next_block(&block, rng).unwrap();

            // Check that the block's solutions are well-formed.
            assert_eq!(block.solutions().len(), 1);
            assert_eq!(block.aborted_solution_ids().len(), 0);

            // Fetch the solution from the block.
            let (solution_id, solution) = block.solutions().as_ref().unwrap().first().unwrap();
            assert_eq!(*solution_id, expected_solution_id, "Check that the block has the correct solution ID");
            assert_eq!(*solution, expected_solution, "Check that the block has the correct solution");
        };

        // Case 1: The malicious solution is included in the block construction.
        let candidate_solutions = vec![malicious_solution];
        check_block(candidate_solutions);

        // Case 2: The valid solution is included in the block construction.
        let candidate_solutions = vec![valid_solution];
        check_block(candidate_solutions);
    }

    #[test]
    fn test_solution_with_insufficient_target() {
        // Initialize an RNG.
        let rng = &mut TestRng::default();

        // Initialize the test environment.
        let crate::test_helpers::TestEnv { ledger, private_key, address, .. } =
            crate::test_helpers::sample_test_env(rng);

        // Retrieve the puzzle parameters.
        let puzzle = ledger.puzzle();
        let latest_epoch_hash = ledger.latest_epoch_hash().unwrap();
        let minimum_proof_target = ledger.latest_proof_target();

        // Initialize a valid solution object.
        let mut invalid_solution = None;
        while invalid_solution.is_none() {
            let solution = puzzle.prove(latest_epoch_hash, address, rng.gen(), None).unwrap();
            if puzzle.get_proof_target(&solution).unwrap() < minimum_proof_target {
                invalid_solution = Some(solution);
            }
        }
        // Unwrap the invalid solution.
        let invalid_solution = invalid_solution.unwrap();

        // Create a valid transaction for the block.
        let inputs = [Value::from_str(&format!("{address}")).unwrap(), Value::from_str("10u64").unwrap()];
        let transfer_transaction = ledger
            .vm
            .execute(&private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
            .unwrap();

        // Create a block.
        let block = ledger
            .prepare_advance_to_next_beacon_block(
                &private_key,
                vec![],
                vec![invalid_solution],
                vec![transfer_transaction],
                rng,
            )
            .unwrap();

        // Check that the next block is valid.
        ledger.check_next_block(&block, rng).unwrap();

        // Add the deployment block to the ledger.
        ledger.advance_to_next_block(&block).unwrap();

        // Check that the block's solutions are well-formed.
        assert_eq!(block.solutions().len(), 0);
        assert_eq!(block.aborted_solution_ids().len(), 1);

        // Check that the aborted solution is correct.
        let block_aborted_solution_id = block.aborted_solution_ids().first().unwrap();
        assert_eq!(*block_aborted_solution_id, invalid_solution.id(), "Aborted solutions do not match");
    }
}

#[test]
fn test_forged_block_subdags() {
    let rng = &mut TestRng::default();

    // Sample the genesis private key.
    let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    // Initialize the store.
    let store = ConsensusStore::<_, ConsensusMemory<_>>::open(None).unwrap();
    // Create a genesis block with a seeded RNG to reproduce the same genesis private keys.
    let seed: u64 = rng.gen();
    let genesis_rng = &mut TestRng::from_seed(seed);
    let genesis = VM::from(store).unwrap().genesis_beacon(&private_key, genesis_rng).unwrap();

    // Extract the private keys from the genesis committee by using the same RNG to sample private keys.
    let genesis_rng = &mut TestRng::from_seed(seed);
    let private_keys = [
        private_key,
        PrivateKey::new(genesis_rng).unwrap(),
        PrivateKey::new(genesis_rng).unwrap(),
        PrivateKey::new(genesis_rng).unwrap(),
    ];

    // Construct 3 quorum blocks.
    let mut quorum_blocks = construct_quorum_blocks(private_keys.to_vec(), genesis.clone(), 3, rng);

    // Extract the individual blocks.
    let block_1 = quorum_blocks.remove(0);
    let block_2 = quorum_blocks.remove(0);
    let block_3 = quorum_blocks.remove(0);

    // Construct the ledger.
    let ledger =
        Ledger::<CurrentNetwork, ConsensusMemory<CurrentNetwork>>::load(genesis, StorageMode::Production).unwrap();
    ledger.advance_to_next_block(&block_1).unwrap();
    ledger.check_next_block(&block_2, rng).unwrap();

    ////////////////////////////////////////////////////////////////////////////
    // Attack 1: Forge block 2' with the subdag of block 3.
    ////////////////////////////////////////////////////////////////////////////
    {
        let block_3_subdag =
            if let Authority::Quorum(subdag) = block_3.authority() { subdag } else { unreachable!("") };

        // Fetch the transmissions.
        let transmissions = extract_transmissions(&block_3);

        // Forge the block.
        let forged_block_2 = ledger
            .prepare_advance_to_next_quorum_block(block_3_subdag.clone(), transmissions, &mut rand::thread_rng())
            .unwrap();

        assert_ne!(forged_block_2, block_2);

        // Attempt to verify the forged block.
        assert!(ledger.check_next_block(&forged_block_2, &mut rand::thread_rng()).is_err());
    }

    ////////////////////////////////////////////////////////////////////////////
    // Attack 2: Forge block 2' with the combined subdag of block 2 and 3.
    ////////////////////////////////////////////////////////////////////////////
    {
        // Fetch the subdags.
        let block_2_subdag =
            if let Authority::Quorum(subdag) = block_2.authority() { subdag } else { unreachable!("") };
        let block_3_subdag =
            if let Authority::Quorum(subdag) = block_3.authority() { subdag } else { unreachable!("") };

        // Combined the subdags.
        let mut combined_subdag = block_2_subdag.deref().clone();
        for (round, certificates) in block_3_subdag.iter() {
            combined_subdag
                .entry(*round)
                .and_modify(|c| c.extend(certificates.clone()))
                .or_insert(certificates.clone());
        }

        // Fetch the transmissions.
        let block_2_transmissions = extract_transmissions(&block_2);
        let block_3_transmissions = extract_transmissions(&block_3);

        // Combine the transmissions.
        let mut combined_transmissions = block_2_transmissions;
        combined_transmissions.extend(block_3_transmissions);

        // Forge the block.
        let forged_block_2_from_both_subdags = ledger
            .prepare_advance_to_next_quorum_block(
                Subdag::from(combined_subdag).unwrap(),
                combined_transmissions,
                &mut rand::thread_rng(),
            )
            .unwrap();

        assert_ne!(forged_block_2_from_both_subdags, block_1);

        // Attempt to verify the forged block.
        assert!(ledger.check_next_block(&forged_block_2_from_both_subdags, &mut rand::thread_rng()).is_err());
    }
}
