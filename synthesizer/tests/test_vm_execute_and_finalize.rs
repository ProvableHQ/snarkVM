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

mod utilities;

use aleo_std::StorageMode;
use snarkvm_console::{
    account::{PrivateKey, ViewKey},
    network::prelude::*,
    program::{Entry, Identifier, Literal, Plaintext, ProgramID, Record, U64, Value},
    types::{Boolean, Field},
};
use snarkvm_ledger_block::{
    Block,
    ConfirmedTransaction,
    Header,
    Metadata,
    Ratifications,
    Transaction,
    Transactions,
    Transition,
};
use snarkvm_ledger_store::{ConsensusStorage, ConsensusStore};
use snarkvm_synthesizer::{Authorization, VM, program::FinalizeOperation};
use snarkvm_synthesizer_process::{execution_cost, execution_cost_for_authorization, execution_cost_for_call};
use snarkvm_synthesizer_program::{FinalizeGlobalState, Program};

use anyhow::Result;
use indexmap::IndexMap;
use snarkvm_console::account::Address;
use utilities::*;

#[cfg(not(feature = "rocks"))]
type LedgerType = snarkvm_ledger_store::helpers::memory::ConsensusMemory<CurrentNetwork>;
#[cfg(feature = "rocks")]
type LedgerType = snarkvm_ledger_store::helpers::rocksdb::ConsensusDB<CurrentNetwork>;

#[test]
#[test_log::test]
fn test_vm_execute_and_finalize() {
    // Load the tests.
    let tests =
        load_tests::<_, ProgramTest>("./tests/vm/execute_and_finalize", "./expectations/vm/execute_and_finalize");

    // Run each test and compare it against its corresponding expectation.
    tests.iter().for_each(|test| {
        // Run the test.
        let output = run_test(test);
        // Check against the expected output.
        test.check(&output).unwrap();
        // Save the output.
        test.save(&output).unwrap();
    });
}

// A helper function to run the test and extract the outputs as YAML, to be compared against the expectation.
fn run_test(test: &ProgramTest) -> serde_yaml::Mapping {
    // Initialize the RNG.
    let rng = &mut match test.randomness() {
        None => TestRng::fixed(123456789),
        Some(randomness) => TestRng::fixed(randomness),
    };

    // RNG used only in `execution_cost_for_call`
    let cost_rng = &mut TestRng::default();

    // Initialize a private key.
    let genesis_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();

    // Initialize the VM.
    let (vm, _) = initialize_vm(&genesis_private_key, test.start_height(), rng);

    // Fund the additional keys.
    for key in test.keys() {
        // Transfer 1_000_000_000_000
        let transaction = vm
            .execute(
                &genesis_private_key,
                ("credits.aleo", "transfer_public"),
                vec![
                    Value::Plaintext(Plaintext::from(Literal::Address(Address::try_from(key).unwrap()))),
                    Value::Plaintext(Plaintext::from(Literal::U64(U64::new(1_000_000_000_000)))),
                ]
                .iter(),
                None,
                0,
                None,
                rng,
            )
            .unwrap();
        let time_since_last_block = CurrentNetwork::BLOCK_TIME as i64;
        let (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations) = vm
            .speculate(
                construct_finalize_global_state(&vm, time_since_last_block),
                time_since_last_block,
                Some(0u64),
                vec![],
                &None.into(),
                [transaction].iter(),
                rng,
            )
            .unwrap();
        assert!(aborted_transaction_ids.is_empty());

        let block = construct_next_block(
            &vm,
            time_since_last_block,
            &genesis_private_key,
            ratifications,
            transactions,
            aborted_transaction_ids,
            ratified_finalize_operations,
            rng,
        );
        vm.add_next_block(&block.unwrap()).unwrap();
    }

    // Deploy the programs.
    for program in test.programs() {
        let transaction = match vm.deploy(&genesis_private_key, program, None, 0, None, rng) {
            Ok(transaction) => transaction,
            Err(error) => {
                let mut output = serde_yaml::Mapping::new();
                output.insert(
                    serde_yaml::Value::String("errors".to_string()),
                    serde_yaml::Value::Sequence(vec![serde_yaml::Value::String(format!(
                        "Failed to run `VM::deploy for program {}: {}",
                        program.id(),
                        error
                    ))]),
                );
                output
                    .insert(serde_yaml::Value::String("outputs".to_string()), serde_yaml::Value::Sequence(Vec::new()));
                return output;
            }
        };

        let time_since_last_block = CurrentNetwork::BLOCK_TIME as i64;
        let (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations) = vm
            .speculate(
                construct_finalize_global_state(&vm, time_since_last_block),
                time_since_last_block,
                Some(0u64),
                vec![],
                &None.into(),
                [transaction].iter(),
                rng,
            )
            .unwrap();
        if !aborted_transaction_ids.is_empty() {
            // Print the program ID that was aborted.
            println!("Aborted program deployment: {:?}", program.id());
            assert!(aborted_transaction_ids.is_empty());
        }

        let block = construct_next_block(
            &vm,
            time_since_last_block,
            &genesis_private_key,
            ratifications,
            transactions,
            aborted_transaction_ids,
            ratified_finalize_operations,
            rng,
        )
        .unwrap();
        vm.add_next_block(&block).unwrap();
    }

    // Run each test case, aggregating the errors, outputs, and additional information.
    let mut outputs = Vec::with_capacity(test.cases().len());
    let mut additional = Vec::with_capacity(test.cases().len());

    for value in test.cases() {
        // TODO: Dedup from other integration tests.
        // Extract the function name, inputs, and optional private key.
        let value = value.as_mapping().expect("expected mapping for test case");
        let program_id = ProgramID::<CurrentNetwork>::from_str(
            value
                .get("program")
                .expect("expected program name for test case")
                .as_str()
                .expect("expected string for program name"),
        )
        .expect("unable to parse program name");
        let function_name = Identifier::<CurrentNetwork>::from_str(
            value
                .get("function")
                .expect("expected function name for test case")
                .as_str()
                .expect("expected string for function name"),
        )
        .expect("unable to parse function name");
        let inputs = value
            .get("inputs")
            .expect("expected inputs for test case")
            .as_sequence()
            .expect("expected sequence for inputs")
            .iter()
            .map(|input| match &input {
                serde_yaml::Value::Bool(bool) => Value::<CurrentNetwork>::from(Literal::Boolean(Boolean::new(*bool))),
                _ => Value::<CurrentNetwork>::from_str(input.as_str().expect("expected string for input"))
                    .expect("unable to parse input"),
            })
            .collect_vec();
        // TODO: Support fee records for custom private keys.
        let private_key = match value.get("private_key") {
            Some(private_key) => {
                PrivateKey::<CurrentNetwork>::from_str(private_key.as_str().expect("expected string for private key"))
                    .expect("unable to parse private key")
            }
            None => genesis_private_key,
        };

        let address = Address::try_from(&private_key).unwrap();

        // A helper function to run the test and extract the outputs as YAML, to be compared against the expectation.
        let mut run_test = || -> (serde_yaml::Value, serde_yaml::Value) {
            // Create a mapping to store the result of the test.
            let mut result = serde_yaml::Mapping::new();
            // Create a mapping to store the other items.
            let mut other = serde_yaml::Mapping::new();

            // Execute the function, extracting the transaction.
            let transaction =
                match vm.execute(&private_key, (program_id, function_name), inputs.iter(), None, 0u64, None, rng) {
                    Ok(transaction) => transaction,
                    // If the execution fails, return the error.
                    Err(err) => {
                        result.insert(
                            serde_yaml::Value::String("execute".to_string()),
                            serde_yaml::Value::String(err.to_string()),
                        );
                        return (serde_yaml::Value::Mapping(result), serde_yaml::Value::Mapping(Default::default()));
                    }
                };

            let consensus_version = CurrentNetwork::CONSENSUS_VERSION(vm.block_store().current_block_height()).unwrap();
            let execution = transaction.execution().unwrap();

            // Test cost computation given the Authorization and the request
            if consensus_version >= ConsensusVersion::V4 {
                let actual_cost = execution_cost(vm.process(), execution, consensus_version).unwrap();

                let authorization = Authorization::from_unchecked((vec![], execution.transitions().cloned().collect()));
                let expected_cost_given_authorization =
                    execution_cost_for_authorization(vm.process(), &authorization, consensus_version).unwrap();
                assert_eq!(actual_cost, expected_cost_given_authorization);

                let expected_cost_given_call = execution_cost_for_call::<CurrentAleo, _>(
                    vm.process(),
                    address,
                    program_id,
                    function_name,
                    inputs.iter(),
                    consensus_version,
                    cost_rng,
                )
                .unwrap();

                assert_eq!(actual_cost, expected_cost_given_call);
            }

            // Attempt to verify the transaction.
            let verified = vm.check_transaction(&transaction, None, rng).is_ok();
            // Store the verification result.
            result.insert(serde_yaml::Value::String("verified".to_string()), serde_yaml::Value::Bool(verified));

            // For each root transition in the transaction, extract the transition outputs and the inputs for finalize.
            let mut execute = serde_yaml::Mapping::new();
            // Store the outputs for child transitions separately, so that they are not checked for consistency.
            let mut child_outputs = serde_yaml::Mapping::new();

            let transitions = transaction.transitions().collect::<Vec<_>>();
            for transition in transitions.iter() {
                let mut transition_output = serde_yaml::Mapping::new();
                let outputs = transition
                    .outputs()
                    .iter()
                    .map(|output| serde_yaml::Value::String(output.to_string()))
                    .collect::<Vec<_>>();
                transition_output
                    .insert(serde_yaml::Value::String("outputs".to_string()), serde_yaml::Value::Sequence(outputs));

                // If this is the last transition, add the outputs to the `execute` mapping.
                if transition.program_id() == &program_id && transition.function_name() == &function_name {
                    execute.insert(
                        serde_yaml::Value::String(format!(
                            "{}/{}",
                            transition.program_id(),
                            transition.function_name()
                        )),
                        serde_yaml::Value::Mapping(transition_output),
                    );
                }
                // Otherwise, add the outputs to the `child_outputs` mapping.
                // This is done to avoid checking the sub-transitions for consistency (since they change every execution).
                else {
                    child_outputs.insert(
                        serde_yaml::Value::String(format!(
                            "{}/{}",
                            transition.program_id(),
                            transition.function_name()
                        )),
                        serde_yaml::Value::Mapping(transition_output),
                    );
                }
            }

            // Add the `execute` mapping to `result` mapping.
            result.insert(serde_yaml::Value::String("execute".to_string()), serde_yaml::Value::Mapping(execute));
            // Add the child outputs to the `other` mapping.
            other.insert(
                serde_yaml::Value::String("child_outputs".to_string()),
                serde_yaml::Value::Mapping(child_outputs),
            );

            // Speculate on the ratifications, solutions, and transaction.
            let time_since_last_block = CurrentNetwork::BLOCK_TIME as i64;
            let (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations) = match vm
                .speculate(
                    construct_finalize_global_state(&vm, time_since_last_block),
                    time_since_last_block,
                    Some(0u64),
                    vec![],
                    &None.into(),
                    [transaction].iter(),
                    rng,
                ) {
                Ok((ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations)) => {
                    result.insert(
                        serde_yaml::Value::String("speculate".to_string()),
                        serde_yaml::Value::String(match transactions.iter().next().unwrap() {
                            ConfirmedTransaction::AcceptedExecute(_, _, _) => "the execution was accepted".to_string(),
                            ConfirmedTransaction::RejectedExecute(_, _, _, _) => {
                                "the execution was rejected".to_string()
                            }
                            ConfirmedTransaction::AcceptedDeploy(_, _, _)
                            | ConfirmedTransaction::RejectedDeploy(_, _, _, _) => {
                                unreachable!("unexpected deployment transaction")
                            }
                        }),
                    );
                    (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations)
                }
                Err(err) => {
                    result.insert(
                        serde_yaml::Value::String("speculate".to_string()),
                        serde_yaml::Value::String(err.to_string()),
                    );
                    return (serde_yaml::Value::Mapping(result), serde_yaml::Value::Mapping(Default::default()));
                }
            };
            if !aborted_transaction_ids.is_empty() {
                // Print the function that was aborted.
                println!("Aborted call to {program_id}/{function_name}");
                assert!(aborted_transaction_ids.is_empty());
            }

            // Construct the next block.
            let block = construct_next_block(
                &vm,
                time_since_last_block,
                &private_key,
                ratifications,
                transactions,
                aborted_transaction_ids,
                ratified_finalize_operations,
                rng,
            )
            .unwrap();
            // Add the next block.
            result.insert(
                serde_yaml::Value::String("add_next_block".to_string()),
                serde_yaml::Value::String(match vm.add_next_block(&block) {
                    Ok(_) => "succeeded.".to_string(),
                    Err(err) => err.to_string(),
                }),
            );
            (serde_yaml::Value::Mapping(result), serde_yaml::Value::Mapping(other))
        };

        // Run the test.
        let (result, other) = run_test();
        outputs.push(result);
        additional.push(other);
    }

    let mut output = serde_yaml::Mapping::new();
    output.insert(serde_yaml::Value::String("errors".to_string()), serde_yaml::Value::Sequence(vec![]));
    output.insert(serde_yaml::Value::String("outputs".to_string()), serde_yaml::Value::Sequence(outputs));
    output.insert(serde_yaml::Value::String("additional".to_string()), serde_yaml::Value::Sequence(additional));
    output
}

// A helper function to initialize the VM.
// Returns a VM and the first record in the genesis block.
#[allow(clippy::type_complexity)]
fn initialize_vm<R: Rng + CryptoRng>(
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

// A helper function construct the desired number of fee records from an initial record, all owned by the same key.
#[allow(unused)]
fn construct_fee_records<C: ConsensusStorage<CurrentNetwork>, R: Rng + CryptoRng>(
    vm: &VM<CurrentNetwork, C>,
    private_key: &PrivateKey<CurrentNetwork>,
    records: Vec<Record<CurrentNetwork, Plaintext<CurrentNetwork>>>,
    num_fee_records: usize,
    rng: &mut R,
) -> Vec<(Record<CurrentNetwork, Plaintext<CurrentNetwork>>, u64)> {
    // Helper function to get the balance of a `credits.aleo` record.
    let get_balance = |record: &Record<CurrentNetwork, Plaintext<CurrentNetwork>>| -> u64 {
        match record.data().get(&Identifier::from_str("microcredits").unwrap()).unwrap() {
            Entry::Private(Plaintext::Literal(Literal::U64(amount), ..)) => **amount,
            _ => unreachable!("Invalid entry type for credits.aleo."),
        }
    };

    println!("Splitting the initial fee record into {num_fee_records} fee records.");

    // Construct fee records for the tests.
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

        // Create a block for the fee transactions and add them to the VM.
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

// A helper function to construct the next block.
#[allow(clippy::too_many_arguments)]
fn construct_next_block<C: ConsensusStorage<CurrentNetwork>, R: Rng + CryptoRng>(
    vm: &VM<CurrentNetwork, C>,
    time_since_last_block: i64,
    private_key: &PrivateKey<CurrentNetwork>,
    ratifications: Ratifications<CurrentNetwork>,
    transactions: Transactions<CurrentNetwork>,
    aborted_transaction_ids: Vec<<CurrentNetwork as Network>::TransactionID>,
    ratified_finalize_operations: Vec<FinalizeOperation<CurrentNetwork>>,
    rng: &mut R,
) -> Result<Block<CurrentNetwork>> {
    // Get the most recent block.
    let block_hash = vm.block_store().get_block_hash(vm.block_store().max_height().unwrap()).unwrap().unwrap();
    let previous_block = vm.block_store().get_block(&block_hash).unwrap().unwrap();

    // Construct the metadata associated with the block.
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
    // Construct the block header.
    let header = Header::from(
        vm.block_store().current_state_root(),
        transactions.to_transactions_root().unwrap(),
        transactions.to_finalize_root(ratified_finalize_operations).unwrap(),
        ratifications.to_ratifications_root().unwrap(),
        Field::zero(),
        Field::zero(),
        metadata,
    )?;

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
}

// A helper function to invoke `credits.aleo/split`.
#[allow(clippy::type_complexity, unused)]
fn split<C: ConsensusStorage<CurrentNetwork>, R: Rng + CryptoRng>(
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

// Construct `FinalizeGlobalState` from the current `VM` state.
fn construct_finalize_global_state<C: ConsensusStorage<CurrentNetwork>>(
    vm: &VM<CurrentNetwork, C>,
    time_since_last_block: i64,
) -> FinalizeGlobalState {
    // Retrieve the latest block.
    let block_height = vm.block_store().max_height().unwrap();
    let latest_block_hash = vm.block_store().get_block_hash(block_height).unwrap().unwrap();
    let latest_block = vm.block_store().get_block(&latest_block_hash).unwrap().unwrap();
    // Retrieve the latest round.
    let latest_round = latest_block.round();
    // Retrieve the latest height.
    let latest_height = latest_block.height();
    // Retrieve the latest cumulative weight.
    let latest_cumulative_weight = latest_block.cumulative_weight();

    // Compute the next round number./
    let next_round = latest_round.saturating_add(1);
    // Compute the next height.
    let next_height = latest_height.saturating_add(1);

    // Determine the block timestamp based on the consensus version.
    let block_timestamp =
        match next_height >= CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V12).unwrap_or_default() {
            true => Some(latest_block.timestamp().saturating_add(time_since_last_block)),
            false => None,
        };
    // Construct the finalize state.
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

#[test]
#[test_log::test]
fn test_finalize_emit_event() {
    // Initialize VM and genesis. Start past V9 height so `constructor:` syntax is accepted.
    // Under `--features test`, V9 activates at height 12.
    let rng = &mut TestRng::fixed(123456789);
    let genesis_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let (vm, _) = initialize_vm(&genesis_private_key, 20, rng);

    // Define a program whose finalize body emits its public input.
    let program_source = r#"program emit_test.aleo;

function emit_value:
    input r0 as u64.public;
    async emit_value r0 into r1;
    output r1 as emit_test.aleo/emit_value.future;

finalize emit_value:
    input r0 as u64.public;
    emit r0;

constructor:
    assert.eq edition 0u16;
"#;
    let program = Program::<CurrentNetwork>::from_str(program_source).unwrap();

    // Deploy the program.
    let time = CurrentNetwork::BLOCK_TIME as i64;
    let deploy_tx = vm.deploy(&genesis_private_key, &program, None, 0, None, rng).unwrap();
    let (ratifications, transactions, aborted, ratified_ops) = vm
        .speculate(
            construct_finalize_global_state(&vm, time),
            time,
            Some(0u64),
            vec![],
            &None.into(),
            [deploy_tx].iter(),
            rng,
        )
        .unwrap();
    assert!(aborted.is_empty(), "deploy was aborted");
    let block =
        construct_next_block(&vm, time, &genesis_private_key, ratifications, transactions, aborted, ratified_ops, rng)
            .unwrap();
    vm.add_next_block(&block).unwrap();

    // Execute `emit_value(42u64)`.
    let input = Value::Plaintext(Plaintext::from(Literal::U64(U64::new(42))));
    let exec_tx = vm
        .execute(&genesis_private_key, ("emit_test.aleo", "emit_value"), [input].iter(), None, 0u64, None, rng)
        .unwrap();

    // Speculate to obtain a `ConfirmedTransaction`.
    let (_ratifications, transactions, aborted, _ratified_ops) = vm
        .speculate(
            construct_finalize_global_state(&vm, time),
            time,
            Some(0u64),
            vec![],
            &None.into(),
            [exec_tx].iter(),
            rng,
        )
        .unwrap();
    assert!(aborted.is_empty(), "execution was aborted");

    // The confirmed transaction must be accepted (not rejected).
    let confirmed = transactions.iter().next().expect("expected one confirmed transaction");
    assert!(
        matches!(confirmed, ConfirmedTransaction::AcceptedExecute(_, _, _)),
        "execution must be accepted, got {confirmed:?}"
    );

    // Inspect finalize operations: expect exactly one `EmitEvent` with the input plaintext.
    let finalize_ops = confirmed.finalize_operations();
    let emit_events: Vec<&Plaintext<CurrentNetwork>> = finalize_ops
        .iter()
        .filter_map(|op| match op {
            FinalizeOperation::EmitEvent(plaintext) => Some(plaintext.as_ref()),
            _ => None,
        })
        .collect();
    assert_eq!(emit_events.len(), 1, "expected exactly one EmitEvent, got {finalize_ops:#?}");

    let expected = Plaintext::<CurrentNetwork>::from(Literal::U64(U64::new(42)));
    assert_eq!(emit_events[0], &expected, "EmitEvent payload did not match emitted value");
}

#[test]
#[test_log::test]
fn test_finalize_assert_with_reason() {
    // Initialize VM and genesis above V9 height so the `constructor:` syntax is accepted.
    let rng = &mut TestRng::fixed(987654321);
    let genesis_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let (vm, _) = initialize_vm(&genesis_private_key, 20, rng);

    // Program exercising the with-reason branches: bare vs with-reason, eq vs neq,
    // reason as register vs literal vs struct. Each function/finalize pair is grouped
    // (the Aleo IR parser expects them interleaved, not all functions then all finalizes).
    let program_source = r#"program assert_reason_test.aleo;

struct Reason:
    code as u64;
    severity as u64;

function check_match:
    input r0 as u64.public;
    input r1 as u64.public;
    input r2 as u64.public;
    async check_match r0 r1 r2 into r3;
    output r3 as assert_reason_test.aleo/check_match.future;

finalize check_match:
    input r0 as u64.public;
    input r1 as u64.public;
    input r2 as u64.public;
    assert.eq r0 r1 with r2;

function check_distinct:
    input r0 as u64.public;
    input r1 as u64.public;
    input r2 as u64.public;
    async check_distinct r0 r1 r2 into r3;
    output r3 as assert_reason_test.aleo/check_distinct.future;

finalize check_distinct:
    input r0 as u64.public;
    input r1 as u64.public;
    input r2 as u64.public;
    assert.neq r0 r1 with r2;

function bare_check:
    input r0 as u64.public;
    input r1 as u64.public;
    async bare_check r0 r1 into r2;
    output r2 as assert_reason_test.aleo/bare_check.future;

finalize bare_check:
    input r0 as u64.public;
    input r1 as u64.public;
    assert.eq r0 r1;

function check_literal_reason:
    input r0 as u64.public;
    input r1 as u64.public;
    async check_literal_reason r0 r1 into r2;
    output r2 as assert_reason_test.aleo/check_literal_reason.future;

finalize check_literal_reason:
    input r0 as u64.public;
    input r1 as u64.public;
    assert.eq r0 r1 with 12345u64;

function check_struct_reason:
    input r0 as u64.public;
    input r1 as u64.public;
    input r2 as u64.public;
    input r3 as u64.public;
    cast r2 r3 into r4 as Reason;
    async check_struct_reason r0 r1 r4 into r5;
    output r5 as assert_reason_test.aleo/check_struct_reason.future;

finalize check_struct_reason:
    input r0 as u64.public;
    input r1 as u64.public;
    input r2 as Reason.public;
    assert.eq r0 r1 with r2;

constructor:
    assert.eq edition 0u16;
"#;
    let program = Program::<CurrentNetwork>::from_str(program_source).unwrap();

    // Deploy.
    let time = CurrentNetwork::BLOCK_TIME as i64;
    let deploy_tx = vm.deploy(&genesis_private_key, &program, None, 0, None, rng).unwrap();
    let (ratifications, transactions, aborted, ratified_ops) = vm
        .speculate(
            construct_finalize_global_state(&vm, time),
            time,
            Some(0u64),
            vec![],
            &None.into(),
            [deploy_tx].iter(),
            rng,
        )
        .unwrap();
    assert!(aborted.is_empty(), "deploy was aborted");
    let block =
        construct_next_block(&vm, time, &genesis_private_key, ratifications, transactions, aborted, ratified_ops, rng)
            .unwrap();
    vm.add_next_block(&block).unwrap();

    // Helper: speculate a single execution and return its confirmed transaction.
    let speculate_one = |function: &str, inputs: Vec<Value<CurrentNetwork>>, rng: &mut TestRng| {
        let exec_tx = vm
            .execute(&genesis_private_key, ("assert_reason_test.aleo", function), inputs.iter(), None, 0u64, None, rng)
            .unwrap();
        let (_, transactions, aborted, _) = vm
            .speculate(
                construct_finalize_global_state(&vm, time),
                time,
                Some(0u64),
                vec![],
                &None.into(),
                [exec_tx].iter(),
                rng,
            )
            .unwrap();
        assert!(aborted.is_empty(), "speculation aborted '{function}'");
        transactions.iter().next().expect("expected one confirmed transaction").clone()
    };

    let u64_literal = |v: u64| Value::Plaintext(Plaintext::from(Literal::U64(U64::new(v))));

    // Case 1: assert.eq matches → accepted; diagnostics map has no entry for this tx.
    {
        let confirmed = speculate_one("check_match", vec![u64_literal(7), u64_literal(7), u64_literal(99)], rng);
        assert!(
            matches!(confirmed, ConfirmedTransaction::AcceptedExecute(_, _, _)),
            "matching inputs must be accepted, got {confirmed:?}"
        );
        assert!(
            vm.pending_rejection_diagnostic(&confirmed.id()).is_none(),
            "accepted tx should have no rejection diagnostics"
        );
    }

    // Case 2: assert.eq mismatch with register reason → rejected; diagnostics has resolved reason.
    {
        let confirmed = speculate_one("check_match", vec![u64_literal(7), u64_literal(8), u64_literal(99)], rng);
        assert!(
            matches!(confirmed, ConfirmedTransaction::RejectedExecute(_, _, _, _)),
            "mismatched inputs must be rejected, got {confirmed:?}"
        );
        let diagnostics = vm
            .pending_rejection_diagnostic(&confirmed.id())
            .expect("expected rejection diagnostics for the rejected tx");
        assert_eq!(
            diagnostics.resolved_reason.as_ref(),
            Some(&Plaintext::<CurrentNetwork>::from(Literal::U64(U64::new(99)))),
            "resolved reason did not match the assert.eq `with` operand"
        );
        assert!(
            diagnostics.error_message.contains("(reason: 99u64)"),
            "error message should surface the resolved reason: {}",
            diagnostics.error_message
        );
    }

    // Case 3: assert.neq distinct inputs (7 != 8) → accepted; no diagnostics.
    {
        let confirmed = speculate_one("check_distinct", vec![u64_literal(7), u64_literal(8), u64_literal(42)], rng);
        assert!(
            matches!(confirmed, ConfirmedTransaction::AcceptedExecute(_, _, _)),
            "distinct inputs to assert.neq must be accepted, got {confirmed:?}"
        );
        assert!(vm.pending_rejection_diagnostic(&confirmed.id()).is_none());
    }

    // Case 4: assert.neq with equal inputs → rejected; diagnostics has resolved reason.
    {
        let confirmed = speculate_one("check_distinct", vec![u64_literal(7), u64_literal(7), u64_literal(42)], rng);
        assert!(matches!(confirmed, ConfirmedTransaction::RejectedExecute(_, _, _, _)));
        let diagnostics = vm.pending_rejection_diagnostic(&confirmed.id()).expect("expected diagnostics");
        assert_eq!(
            diagnostics.resolved_reason.as_ref(),
            Some(&Plaintext::<CurrentNetwork>::from(Literal::U64(U64::new(42)))),
            "neq-with-reason failure should surface the third operand as the reason"
        );
        assert!(diagnostics.error_message.contains("(reason: 42u64)"));
    }

    // Case 5: bare assert.eq mismatch → rejected; diagnostics present but resolved_reason is None.
    // The error message does NOT mention a reason (no phantom reason from bare asserts).
    {
        let confirmed = speculate_one("bare_check", vec![u64_literal(7), u64_literal(8)], rng);
        assert!(matches!(confirmed, ConfirmedTransaction::RejectedExecute(_, _, _, _)));
        let diagnostics = vm.pending_rejection_diagnostic(&confirmed.id()).expect("expected diagnostics");
        assert!(
            diagnostics.resolved_reason.is_none(),
            "bare assert.eq failures must not surface a resolved reason, got {:?}",
            diagnostics.resolved_reason
        );
        assert!(
            !diagnostics.error_message.contains("reason:"),
            "bare assert.eq error message should not mention a reason: {}",
            diagnostics.error_message
        );
    }

    // Case 6: with-reason using a literal operand (no register indirection).
    {
        let confirmed = speculate_one("check_literal_reason", vec![u64_literal(7), u64_literal(8)], rng);
        assert!(matches!(confirmed, ConfirmedTransaction::RejectedExecute(_, _, _, _)));
        let diagnostics = vm.pending_rejection_diagnostic(&confirmed.id()).expect("expected diagnostics");
        assert_eq!(
            diagnostics.resolved_reason.as_ref(),
            Some(&Plaintext::<CurrentNetwork>::from(Literal::U64(U64::new(12345)))),
            "literal-operand reason should be captured verbatim"
        );
    }

    // Case 7: struct-shaped reason — the typed-error use case. The function constructs a
    // `Reason { code, severity }` struct from u64 inputs and passes it to finalize, where
    // the failing assert attaches it as the reason. Verifies the diagnostics carry the
    // resolved struct plaintext (not just a primitive).
    {
        let confirmed = speculate_one(
            "check_struct_reason",
            vec![u64_literal(7), u64_literal(8), u64_literal(5), u64_literal(9)],
            rng,
        );
        assert!(matches!(confirmed, ConfirmedTransaction::RejectedExecute(_, _, _, _)));
        let diagnostics = vm.pending_rejection_diagnostic(&confirmed.id()).expect("expected diagnostics");
        let expected_struct = Plaintext::<CurrentNetwork>::from_str("{ code: 5u64, severity: 9u64 }").unwrap();
        assert_eq!(
            diagnostics.resolved_reason.as_ref(),
            Some(&expected_struct),
            "struct reason should roundtrip through Display → from_str"
        );
    }

    // Case 8: multiple rejected txs in a single speculation — verify the diagnostics map
    // keys each rejection independently (no aliasing, no clobbering).
    {
        let tx_a = vm
            .execute(
                &genesis_private_key,
                ("assert_reason_test.aleo", "check_match"),
                [u64_literal(1), u64_literal(2), u64_literal(111)].iter(),
                None,
                0u64,
                None,
                rng,
            )
            .unwrap();
        let tx_b = vm
            .execute(
                &genesis_private_key,
                ("assert_reason_test.aleo", "check_distinct"),
                [u64_literal(7), u64_literal(7), u64_literal(222)].iter(),
                None,
                0u64,
                None,
                rng,
            )
            .unwrap();
        let (_, transactions, aborted, _) = vm
            .speculate(
                construct_finalize_global_state(&vm, time),
                time,
                Some(0u64),
                vec![],
                &None.into(),
                [tx_a, tx_b].iter(),
                rng,
            )
            .unwrap();
        assert!(aborted.is_empty(), "multi-tx speculation aborted");
        // Both must be rejected. Rejected-execute confirmed txs use the fee tx's id as
        // their confirmed id, so we don't try to map original→confirmed; instead we
        // gather the resolved reasons via the diagnostics map and assert the set.
        let mut reasons: Vec<String> = Vec::new();
        for confirmed in transactions.iter() {
            assert!(
                matches!(confirmed, ConfirmedTransaction::RejectedExecute(_, _, _, _)),
                "expected RejectedExecute, got {confirmed:?}"
            );
            let diagnostics =
                vm.pending_rejection_diagnostic(&confirmed.id()).expect("expected diagnostics for each rejection");
            let reason = diagnostics.resolved_reason.as_ref().expect("expected a resolved reason");
            reasons.push(reason.to_string());
        }
        reasons.sort();
        assert_eq!(
            reasons,
            vec!["111u64".to_string(), "222u64".to_string()],
            "both rejected txs must surface their reasons in the same pass"
        );
    }

    // Case 9: diagnostics are cleared at the start of each new speculation — a stale entry
    // from a prior pass must not leak into the next.
    {
        // Speculate one rejection, capture its tx id.
        let stale = speculate_one("check_match", vec![u64_literal(3), u64_literal(4), u64_literal(555)], rng);
        assert!(matches!(stale, ConfirmedTransaction::RejectedExecute(_, _, _, _)));
        let stale_id = stale.id();
        assert!(vm.pending_rejection_diagnostic(&stale_id).is_some(), "stale diagnostic must be present pre-clear");

        // Speculate another tx (any kind); the previous diagnostic must be gone.
        let _next = speculate_one("check_match", vec![u64_literal(9), u64_literal(9), u64_literal(0)], rng);
        assert!(
            vm.pending_rejection_diagnostic(&stale_id).is_none(),
            "diagnostic from the previous speculation pass must be cleared"
        );
    }
}
