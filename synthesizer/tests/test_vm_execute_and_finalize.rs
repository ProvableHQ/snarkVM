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

use snarkvm_console::{
    account::{Address, PrivateKey},
    network::prelude::*,
    program::{Identifier, Literal, Plaintext, ProgramID, U64, Value},
    types::Boolean,
};
use snarkvm_ledger_block::ConfirmedTransaction;
use snarkvm_synthesizer::Authorization;
use snarkvm_synthesizer_process::{execution_cost, execution_cost_for_authorization, execution_cost_for_call};

use utilities::*;

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
