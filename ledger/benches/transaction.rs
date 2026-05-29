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

#![allow(clippy::type_complexity)]

#[macro_use]
extern crate criterion;

use snarkvm_console::{
    account::*,
    network::{ConsensusVersion, MainnetV0, Network},
    program::{Plaintext, Record, Value},
    types::Field,
};
use snarkvm_ledger_block::{Block, Header, Metadata, Transaction, Transition};
use snarkvm_ledger_store::ConsensusStore;
use snarkvm_synthesizer::{
    VM,
    program::{FinalizeGlobalState, Program},
};

use aleo_std::StorageMode;
use criterion::Criterion;

#[cfg(not(feature = "rocks"))]
type LedgerType = snarkvm_ledger_store::helpers::memory::ConsensusMemory<MainnetV0>;
#[cfg(feature = "rocks")]
type LedgerType = snarkvm_ledger_store::helpers::rocksdb::ConsensusDB<MainnetV0>;

fn initialize_vm<R: Rng + CryptoRng>(
    private_key: &PrivateKey<MainnetV0>,
    rng: &mut R,
) -> (VM<MainnetV0, LedgerType>, Vec<Record<MainnetV0, Plaintext<MainnetV0>>>) {
    // Initialize the VM.
    let vm = VM::from(ConsensusStore::open(StorageMode::new_test(None)).unwrap()).unwrap();

    // Initialize the genesis block.
    let genesis = vm.genesis_beacon(private_key, rng).unwrap();

    // Update the VM.
    vm.add_next_block(&genesis).unwrap();

    // Advance the VM so that the benchmarks run starting at `ConsensusVersion::V16`.
    // NOTE: This requires the `test` feature, which enables the small test consensus heights;
    //       under the mainnet heights `V16` activates at `u32::MAX` and is unreachable here.
    advance_to_consensus_version(&vm, private_key, ConsensusVersion::V16, rng);

    // Mint fresh records at the current (post-upgrade) height.
    // The genesis records were created at height 0, before the `V8` inclusion upgrade
    // (`INCLUSION_UPGRADE_HEIGHT`), so they cannot be spent privately under the `V16` inclusion
    // rules. Minting records here ensures they satisfy the record-height check.
    let records = mint_records(&vm, private_key, rng);

    (vm, records)
}

/// Mints fresh spendable records for the given private key at the VM's current height by
/// converting public credits into a private record via `credits.aleo/transfer_public_to_private`.
fn mint_records<R: Rng + CryptoRng>(
    vm: &VM<MainnetV0, LedgerType>,
    private_key: &PrivateKey<MainnetV0>,
    rng: &mut R,
) -> Vec<Record<MainnetV0, Plaintext<MainnetV0>>> {
    // Prepare the inputs: send a large amount of public credits to the caller as a private record.
    let address = Address::try_from(private_key).unwrap();
    let inputs =
        [Value::from_str(&address.to_string()).unwrap(), Value::<MainnetV0>::from_str("1000000000000u64").unwrap()];

    // Create an execution transaction that converts public credits into a private record.
    let transaction = vm
        .execute(private_key, ("credits.aleo", "transfer_public_to_private"), inputs.into_iter(), None, 0, None, rng)
        .unwrap();

    // Include the transaction in the next block and update the VM.
    let block = sample_next_block(vm, private_key, &[transaction], rng);
    vm.add_next_block(&block).unwrap();

    // Decrypt and return the newly-minted records owned by the caller.
    let view_key = ViewKey::try_from(private_key).unwrap();
    block
        .transitions()
        .cloned()
        .flat_map(Transition::into_records)
        .filter_map(|(_, record)| record.decrypt(&view_key).ok())
        .collect()
}

/// Advances the VM by producing empty beacon blocks until the given consensus version is active.
fn advance_to_consensus_version<R: Rng + CryptoRng>(
    vm: &VM<MainnetV0, LedgerType>,
    private_key: &PrivateKey<MainnetV0>,
    version: ConsensusVersion,
    rng: &mut R,
) {
    // Determine the activation height of the requested consensus version.
    let target_height = MainnetV0::CONSENSUS_HEIGHT(version).unwrap();
    // Produce empty beacon blocks until the VM reaches the target height.
    while vm.block_store().current_block_height() < target_height {
        let block = sample_next_block(vm, private_key, &[], rng);
        vm.add_next_block(&block).unwrap();
    }
}

/// Samples the next beacon block containing the given transactions for the given VM.
fn sample_next_block<R: Rng + CryptoRng>(
    vm: &VM<MainnetV0, LedgerType>,
    private_key: &PrivateKey<MainnetV0>,
    transactions: &[Transaction<MainnetV0>],
    rng: &mut R,
) -> Block<MainnetV0> {
    // Get the most recent block.
    let block_hash = vm.block_store().get_block_hash(vm.block_store().max_height().unwrap()).unwrap().unwrap();
    let previous_block = vm.block_store().get_block(&block_hash).unwrap().unwrap();

    // Create the finalize state for the next block height.
    let next_block_height = previous_block.height() + 1;
    let time_since_last_block = MainnetV0::BLOCK_TIME as i64;
    let next_block_timestamp = previous_block.timestamp().saturating_add(time_since_last_block);
    let next_timestamp = (next_block_height >= MainnetV0::CONSENSUS_HEIGHT(ConsensusVersion::V12).unwrap_or_default())
        .then_some(next_block_timestamp);
    let finalize_state =
        FinalizeGlobalState::from(next_block_height as u64, next_block_height, next_timestamp, [0u8; 32], None, None);

    // Speculate on the given transactions.
    let (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations) = vm
        .speculate(finalize_state, time_since_last_block, Some(0u64), vec![], &None.into(), transactions.iter(), rng)
        .unwrap();

    // Construct the metadata associated with the block.
    let metadata = Metadata::new(
        MainnetV0::ID,
        previous_block.round() + 1,
        next_block_height,
        0,
        0,
        MainnetV0::GENESIS_COINBASE_TARGET,
        MainnetV0::GENESIS_PROOF_TARGET,
        previous_block.last_coinbase_target(),
        previous_block.last_coinbase_timestamp(),
        next_block_timestamp,
    )
    .unwrap();

    // Construct the new block header.
    let header = Header::from(
        vm.block_store().current_state_root(),
        transactions.to_transactions_root().unwrap(),
        transactions.to_finalize_root(ratified_finalize_operations).unwrap(),
        ratifications.to_ratifications_root().unwrap(),
        Field::zero(),
        Field::zero(),
        metadata,
    )
    .unwrap();

    // Construct the new block.
    Block::new_beacon(
        private_key,
        previous_block.hash(),
        header,
        ratifications,
        None.into(),
        vec![],
        transactions,
        aborted_transaction_ids,
        rng,
    )
    .unwrap()
}

fn deploy(c: &mut Criterion) {
    let rng = &mut TestRng::default();

    // Sample a new private key and address.
    let private_key = PrivateKey::<MainnetV0>::new(rng).unwrap();

    // Initialize the VM.
    let (vm, records) = initialize_vm(&private_key, rng);

    // Create a sample program.
    let program = Program::<MainnetV0>::from_str(
        r"
program helloworld.aleo;

function hello:
    input r0 as u32.private;
    input r1 as u32.private;
    add r0 r1 into r2;
    output r2 as u32.private;

constructor:
    assert.eq true true;
",
    )
    .unwrap();

    c.bench_function("Transaction::Deploy", |b| {
        b.iter(|| vm.deploy(&private_key, &program, Some(records[0].clone()), 600000, None, rng).unwrap())
    });

    c.bench_function("Transaction::Deploy - verify", |b| {
        let transaction = vm.deploy(&private_key, &program, Some(records[0].clone()), 600000, None, rng).unwrap();
        b.iter(|| vm.check_transaction(&transaction, None, rng).unwrap())
    });
}

fn execute(c: &mut Criterion) {
    let rng = &mut TestRng::default();

    // Sample a new private key and address.
    let private_key = PrivateKey::<MainnetV0>::new(rng).unwrap();
    let address = Address::try_from(&private_key).unwrap();

    // Initialize the VM.
    let (vm, records) = initialize_vm(&private_key, rng);

    {
        // Prepare the inputs.
        let inputs = [
            Value::<MainnetV0>::from_str(&address.to_string()).unwrap(),
            Value::<MainnetV0>::from_str("1u64").unwrap(),
        ]
        .into_iter();

        // Authorize the execution.
        let execute_authorization = vm.authorize(&private_key, "credits.aleo", "transfer_public", inputs, rng).unwrap();
        // Retrieve the execution ID.
        let execution_id = execute_authorization.to_execution_id().unwrap();
        // Authorize the fee.
        let fee_authorization = vm.authorize_fee_public(&private_key, 300000, 1000, execution_id, rng).unwrap();

        c.bench_function("Transaction::Execute(transfer_public)", |b| {
            b.iter(|| {
                vm.execute_authorization(
                    execute_authorization.replicate(),
                    Some(fee_authorization.replicate()),
                    None,
                    rng,
                )
                .unwrap();
            })
        });

        let transaction = vm
            .execute_authorization(execute_authorization.replicate(), Some(fee_authorization.replicate()), None, rng)
            .unwrap();

        // Bench the Transaction.write_le method using the LimitedWriter.
        c.bench_function("LimitedWriter::new - transfer_public", |b| {
            let mut buffer = Vec::with_capacity(3000);
            b.iter(|| transaction.write_le(LimitedWriter::new(&mut buffer, MainnetV0::LATEST_MAX_TRANSACTION_SIZE())))
        });

        // Bench the execution of transfer_public.
        c.bench_function("Transaction::Execute(transfer_public) - verify", |b| {
            b.iter(|| vm.check_transaction(&transaction, None, rng).unwrap())
        });
    }

    {
        // Prepare the inputs.
        let inputs = [
            Value::<MainnetV0>::Record(records[0].clone()),
            Value::<MainnetV0>::from_str(&address.to_string()).unwrap(),
            Value::<MainnetV0>::from_str("1u64").unwrap(),
        ]
        .into_iter();

        // Authorize the execution.
        let execute_authorization =
            vm.authorize(&private_key, "credits.aleo", "transfer_private", inputs, rng).unwrap();
        // Retrieve the execution ID.
        let execution_id = execute_authorization.to_execution_id().unwrap();
        // Authorize the fee.
        let fee_authorization = vm.authorize_fee_public(&private_key, 300000, 1000, execution_id, rng).unwrap();

        // Bench the execution of transfer_private.
        c.bench_function("Transaction::Execute(transfer_private)", |b| {
            b.iter(|| {
                vm.execute_authorization(
                    execute_authorization.replicate(),
                    Some(fee_authorization.replicate()),
                    None,
                    rng,
                )
                .unwrap();
            })
        });

        let transaction = vm
            .execute_authorization(execute_authorization.replicate(), Some(fee_authorization.replicate()), None, rng)
            .unwrap();

        // Bench the Transaction.write_le method using the LimitedWriter.
        c.bench_function("LimitedWriter::new - transfer_private", |b| {
            let mut buffer = Vec::with_capacity(3000);
            b.iter(|| transaction.write_le(LimitedWriter::new(&mut buffer, MainnetV0::LATEST_MAX_TRANSACTION_SIZE())))
        });

        // Bench the check_transaction method.
        c.bench_function("Transaction::Execute(transfer_private) - verify", |b| {
            b.iter(|| vm.check_transaction(&transaction, None, rng).unwrap())
        });
    }

    // Bench Transaction.write_le + VM.check_transaction methods for transactions above the maximum transaction size.
    {
        // Define a program that will create an execution transaction larger than the maximum transaction size.
        let program = Program::<MainnetV0>::from_str(
            r"
program too_big.aleo;

struct all_groups:
    g1 as [[[group; 4u32]; 4u32]; 4u32];
    g2 as [[[group; 4u32]; 4u32]; 4u32];

struct nested_groups:
    g1 as all_groups;
    g2 as all_groups;

function main:
    // Input the amount of microcredits to unbond.
    input r0 as group.public;
    cast r0 r0 r0 r0 into r1 as [group; 4u32];
    cast r1 r1 r1 r1 into r2 as [[group; 4u32]; 4u32];
    cast r2 r2 r2 r2 into r3 as [[[group; 4u32]; 4u32]; 4u32];
    cast r3 r3 into r4 as all_groups;
    cast r4 r4 into r5 as nested_groups;
    cast r4 r4 into r6 as nested_groups;
    cast r4 r4 into r7 as nested_groups;
    cast r4 r4 into r8 as nested_groups;
    cast r4 r4 into r9 as nested_groups;
    cast r4 r4 into r10 as nested_groups;
    cast r4 r4 into r11 as nested_groups;
    cast r4 r4 into r12 as nested_groups;
    cast r4 r4 into r13 as nested_groups;
    cast r4 r4 into r14 as nested_groups;
    cast r4 r4 into r15 as nested_groups;
    cast r4 r4 into r16 as nested_groups;
    cast r4 r4 into r17 as nested_groups;
    cast r4 r4 into r18 as nested_groups;
    cast r4 r4 into r19 as nested_groups;
    cast r4 r4 into r20 as nested_groups;
    cast r4 r4 into r21 as nested_groups;
    cast r4 r4 into r22 as nested_groups;
    cast r4 r4 into r23 as nested_groups;
    cast r4 r4 into r24 as nested_groups;
    cast r4 r4 into r25 as nested_groups;
    cast r4 r4 into r26 as nested_groups;
    cast r4 r4 into r27 as nested_groups;
    cast r4 r4 into r28 as nested_groups;
    cast r4 r4 into r29 as nested_groups;
    cast r4 r4 into r30 as nested_groups;
    cast r4 r4 into r31 as nested_groups;
    output r7 as nested_groups.public;
    output r8 as nested_groups.public;
    output r9 as nested_groups.public;
    output r10 as nested_groups.public;
    output r11 as nested_groups.public;
    output r12 as nested_groups.public;
    output r13 as nested_groups.public;
    output r14 as nested_groups.public;
    output r15 as nested_groups.public;
    output r16 as nested_groups.public;
    output r17 as nested_groups.public;
    output r18 as nested_groups.public;
    output r19 as nested_groups.public;
    output r20 as nested_groups.public;
    output r21 as nested_groups.public;
    output r22 as nested_groups.public;
    ",
        )
        .unwrap();
        // Prepare the inputs.
        let inputs = [Value::from_str("2group").unwrap()].into_iter();

        // Add the program to the VM.
        vm.process().lock().add_program(&program).unwrap();

        // Create an execution transaction that is 164613 bytes in size.
        let transaction = vm.execute(&private_key, ("too_big.aleo", "main"), inputs, None, 0, None, rng).unwrap();

        // Bench the Transaction.write_le method using the LimitedWriter.
        c.bench_function("LimitedWriter::new - too_big.aleo", |b| {
            let mut buffer = Vec::with_capacity(MainnetV0::LATEST_MAX_TRANSACTION_SIZE());
            b.iter(|| transaction.write_le(LimitedWriter::new(&mut buffer, MainnetV0::LATEST_MAX_TRANSACTION_SIZE())))
        });

        // Bench the check_transaction method.
        c.bench_function("Transaction::Execute(too_big.aleo) - verify", |b| {
            b.iter(|| vm.check_transaction(&transaction, None, rng))
        });
    }
}

criterion_group! {
    name = transaction;
    config = Criterion::default().sample_size(10);
    targets = deploy, execute
}

criterion_main!(transaction);
