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

//! VM-level test helpers shared across integration tests.
//!
//! Originally lived inline in `test_vm_execute_and_finalize.rs`. Pulled out here so
//! per-feature test files (e.g. `test_v15.rs`) can reuse them without copy-pasting
//! ~200 lines of VM/block-construction boilerplate.

use crate::utilities::CurrentNetwork;
use aleo_std::StorageMode;
use anyhow::Result;
use indexmap::IndexMap;
use itertools::Itertools;
use rand::{CryptoRng, Rng};
use snarkvm_console::{
    account::{PrivateKey, ViewKey},
    network::prelude::*,
    program::{Entry, Identifier, Literal, Plaintext, Record, U64, Value},
    types::Field,
};
use snarkvm_ledger_block::{Block, Header, Metadata, Ratifications, Transaction, Transactions, Transition};
use snarkvm_ledger_store::{ConsensusStorage, ConsensusStore};
use snarkvm_synthesizer::{VM, program::FinalizeOperation};
use snarkvm_synthesizer_program::FinalizeGlobalState;

#[cfg(not(feature = "rocks"))]
pub type LedgerType = snarkvm_ledger_store::helpers::memory::ConsensusMemory<CurrentNetwork>;
#[cfg(feature = "rocks")]
pub type LedgerType = snarkvm_ledger_store::helpers::rocksdb::ConsensusDB<CurrentNetwork>;

/// Initializes a VM with a genesis block and advances it by `height` empty blocks.
/// Returns the VM and the records produced in the genesis block (owned by `private_key`).
#[allow(clippy::type_complexity)]
pub fn initialize_vm<R: Rng + CryptoRng>(
    private_key: &PrivateKey<CurrentNetwork>,
    height: u32,
    rng: &mut R,
) -> (VM<CurrentNetwork, LedgerType>, Vec<Record<CurrentNetwork, Plaintext<CurrentNetwork>>>) {
    // Initialize a VM.
    let vm: VM<CurrentNetwork, LedgerType> =
        VM::from(ConsensusStore::open(StorageMode::new_test(None)).unwrap()).unwrap();

    // Initialize the genesis block.
    let genesis = vm.genesis_beacon(private_key, rng).unwrap();

    // Select a record to spend.
    let view_key = ViewKey::try_from(private_key).unwrap();
    let records = genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
    let records = records.values().map(|record| record.decrypt(&view_key).unwrap()).collect::<Vec<_>>();

    // Add the genesis block to the VM.
    vm.add_next_block(&genesis).unwrap();

    // If the desired height is greater than zero, add additional blocks to the VM.
    for _ in 0..height {
        let time_since_last_block = CurrentNetwork::BLOCK_TIME as i64;
        let (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations) = vm
            .speculate(
                construct_finalize_global_state(&vm, time_since_last_block),
                time_since_last_block,
                Some(0u64),
                vec![],
                &None.into(),
                [].into_iter(),
                rng,
            )
            .unwrap();
        assert!(aborted_transaction_ids.is_empty());

        let block = construct_next_block(
            &vm,
            time_since_last_block,
            private_key,
            ratifications,
            transactions,
            aborted_transaction_ids,
            ratified_finalize_operations,
            rng,
        )
        .unwrap();
        vm.add_next_block(&block).unwrap();
    }

    (vm, records)
}

/// Splits an initial record into `num_fee_records` records owned by `private_key`.
#[allow(clippy::type_complexity, unused)]
pub fn construct_fee_records<C: ConsensusStorage<CurrentNetwork>, R: Rng + CryptoRng>(
    vm: &VM<CurrentNetwork, C>,
    private_key: &PrivateKey<CurrentNetwork>,
    records: Vec<Record<CurrentNetwork, Plaintext<CurrentNetwork>>>,
    num_fee_records: usize,
    rng: &mut R,
) -> Vec<(Record<CurrentNetwork, Plaintext<CurrentNetwork>>, u64)> {
    let get_balance = |record: &Record<CurrentNetwork, Plaintext<CurrentNetwork>>| -> u64 {
        match record.data().get(&Identifier::from_str("microcredits").unwrap()).unwrap() {
            Entry::Private(Plaintext::Literal(Literal::U64(amount), ..)) => **amount,
            _ => unreachable!("Invalid entry type for credits.aleo."),
        }
    };

    println!("Splitting the initial fee record into {num_fee_records} fee records.");

    let mut fee_records = records
        .into_iter()
        .map(|record| {
            let balance = get_balance(&record);
            (record, balance)
        })
        .collect::<Vec<_>>();
    let mut fee_counter = 1;
    while fee_records.len() < num_fee_records {
        let mut transactions = Vec::with_capacity(fee_records.len());
        for (fee_record, balance) in fee_records.drain(..).collect_vec() {
            if fee_counter < num_fee_records {
                println!("Splitting out the {}-th record of size {}.", fee_counter, balance / 2);
                let (mut records, txns) = split(vm, private_key, fee_record, balance / 2, rng);
                let second = records.pop().unwrap();
                let first = records.pop().unwrap();
                let balance = get_balance(&first);
                fee_records.push((first, balance));
                let balance = get_balance(&second);
                fee_records.push((second, balance));
                transactions.extend(txns);
                fee_counter += 1;
            } else {
                fee_records.push((fee_record, balance));
            }
        }

        let time_since_last_block = CurrentNetwork::BLOCK_TIME as i64;
        let (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations) = vm
            .speculate(
                construct_finalize_global_state(vm, time_since_last_block),
                time_since_last_block,
                Some(0u64),
                vec![],
                &None.into(),
                transactions.iter(),
                rng,
            )
            .unwrap();
        assert!(aborted_transaction_ids.is_empty());

        let block = construct_next_block(
            vm,
            time_since_last_block,
            private_key,
            ratifications,
            transactions,
            aborted_transaction_ids,
            ratified_finalize_operations,
            rng,
        )
        .unwrap();
        vm.add_next_block(&block).unwrap();
    }

    println!("Constructed fee records.");

    fee_records
}

/// Builds the next block from the speculation result and adds it to the VM's stores.
#[allow(clippy::too_many_arguments)]
pub fn construct_next_block<C: ConsensusStorage<CurrentNetwork>, R: Rng + CryptoRng>(
    vm: &VM<CurrentNetwork, C>,
    time_since_last_block: i64,
    private_key: &PrivateKey<CurrentNetwork>,
    ratifications: Ratifications<CurrentNetwork>,
    transactions: Transactions<CurrentNetwork>,
    aborted_transaction_ids: Vec<<CurrentNetwork as Network>::TransactionID>,
    ratified_finalize_operations: Vec<FinalizeOperation<CurrentNetwork>>,
    rng: &mut R,
) -> Result<Block<CurrentNetwork>> {
    let block_hash = vm.block_store().get_block_hash(vm.block_store().max_height().unwrap()).unwrap().unwrap();
    let previous_block = vm.block_store().get_block(&block_hash).unwrap().unwrap();

    let metadata = Metadata::new(
        CurrentNetwork::ID,
        previous_block.round() + 1,
        previous_block.height() + 1,
        0,
        0,
        CurrentNetwork::GENESIS_COINBASE_TARGET,
        CurrentNetwork::GENESIS_PROOF_TARGET,
        previous_block.last_coinbase_target(),
        previous_block.last_coinbase_timestamp(),
        previous_block.timestamp().saturating_add(time_since_last_block),
    )?;
    let header = Header::from(
        vm.block_store().current_state_root(),
        transactions.to_transactions_root().unwrap(),
        transactions.to_finalize_root(ratified_finalize_operations).unwrap(),
        ratifications.to_ratifications_root().unwrap(),
        Field::zero(),
        Field::zero(),
        metadata,
    )?;

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
}

/// Invokes `credits.aleo/split` to create two records from one.
#[allow(clippy::type_complexity, unused)]
pub fn split<C: ConsensusStorage<CurrentNetwork>, R: Rng + CryptoRng>(
    vm: &VM<CurrentNetwork, C>,
    private_key: &PrivateKey<CurrentNetwork>,
    record: Record<CurrentNetwork, Plaintext<CurrentNetwork>>,
    amount: u64,
    rng: &mut R,
) -> (Vec<Record<CurrentNetwork, Plaintext<CurrentNetwork>>>, Vec<Transaction<CurrentNetwork>>) {
    let inputs = vec![Value::Record(record), Value::Plaintext(Plaintext::from(Literal::U64(U64::new(amount))))];
    let transaction = vm.execute(private_key, ("credits.aleo", "split"), inputs.iter(), None, 0, None, rng).unwrap();
    let records = transaction
        .records()
        .map(|(_, record)| record.decrypt(&ViewKey::try_from(private_key).unwrap()).unwrap())
        .collect_vec();
    assert_eq!(records.len(), 2);
    (records, vec![transaction])
}

/// Constructs a `FinalizeGlobalState` from the current `VM` state, ready to feed into
/// the next speculation pass.
pub fn construct_finalize_global_state<C: ConsensusStorage<CurrentNetwork>>(
    vm: &VM<CurrentNetwork, C>,
    time_since_last_block: i64,
) -> FinalizeGlobalState {
    let block_height = vm.block_store().max_height().unwrap();
    let latest_block_hash = vm.block_store().get_block_hash(block_height).unwrap().unwrap();
    let latest_block = vm.block_store().get_block(&latest_block_hash).unwrap().unwrap();
    let latest_round = latest_block.round();
    let latest_height = latest_block.height();
    let latest_cumulative_weight = latest_block.cumulative_weight();

    let next_round = latest_round.saturating_add(1);
    let next_height = latest_height.saturating_add(1);

    let block_timestamp =
        match next_height >= CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V12).unwrap_or_default() {
            true => Some(latest_block.timestamp().saturating_add(time_since_last_block)),
            false => None,
        };
    FinalizeGlobalState::new::<CurrentNetwork>(
        next_round,
        next_height,
        block_timestamp,
        latest_cumulative_weight,
        0u128,
        latest_block.hash(),
    )
    .unwrap()
}
