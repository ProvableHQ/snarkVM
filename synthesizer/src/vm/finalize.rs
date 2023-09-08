// Copyright (C) 2019-2023 Aleo Systems Inc.
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
use ledger_block::{ConfirmedTransaction, Rejected, Transactions};
use ledger_coinbase::CoinbaseSolution;

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// Speculates on the given list of transactions in the VM,
    /// returning the confirmed transactions, aborted transactions, and the finalize digest.
    #[inline]
    pub fn speculate<'a>(
        &self,
        state: FinalizeGlobalState,
        ratifications: &[Ratify<N>],
        solutions: Option<&CoinbaseSolution<N>>,
        transactions: impl ExactSizeIterator<Item = &'a Transaction<N>>,
    ) -> Result<(Transactions<N>, Vec<Transaction<N>>, Field<N>)> {
        let timer = timer!("VM::speculate");

        // Performs a **dry-run** over the list of ratifications, solutions, and transactions.
        let (confirmed_transactions, aborted_transactions, write_batch) =
            self.atomic_speculate(state, ratifications, solutions, transactions)?;

        // TODO (raychu86): Optimize this. We can use any hash function.
        // Compute the finalize digest.
        let input = write_batch.iter().flat_map(|b| b.to_bits_le()).collect::<Vec<_>>();
        let finalize_digest = N::hash_bhp1024(&input)?;

        finish!(timer, "Finished dry-run of the transactions");

        // Return the transactions.
        Ok((confirmed_transactions.into_iter().collect(), aborted_transactions, finalize_digest))
    }

    /// Finalizes the given transactions into the VM, returning the finalize digest.
    #[inline]
    pub fn finalize(
        &self,
        state: FinalizeGlobalState,
        ratifications: &[Ratify<N>],
        solutions: Option<&CoinbaseSolution<N>>,
        transactions: &Transactions<N>,
    ) -> Result<Field<N>> {
        let timer = timer!("VM::finalize");

        // Performs a **real-run** of finalize over the list of ratifications, solutions, and transactions.
        let write_batch = self.atomic_finalize(state, ratifications, solutions, transactions)?;

        // TODO (raychu86): Optimize this. We can use any hash function.
        // Compute the finalize digest.
        let input = write_batch.iter().flat_map(|b| b.to_bits_le()).collect::<Vec<_>>();
        let finalize_digest = N::hash_bhp1024(&input)?;

        finish!(timer, "Finished real-run of finalize");
        Ok(finalize_digest)
    }
}

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// Performs atomic speculation over a list of transactions,
    /// and returns the confirmed transactions, aborted transactions,
    /// and the finalize store write batch.
    fn atomic_speculate<'a>(
        &self,
        state: FinalizeGlobalState,
        ratifications: &[Ratify<N>],
        solutions: Option<&CoinbaseSolution<N>>,
        transactions: impl ExactSizeIterator<Item = &'a Transaction<N>>,
    ) -> Result<(Vec<ConfirmedTransaction<N>>, Vec<Transaction<N>>, Vec<u8>)> {
        let timer = timer!("VM::atomic_speculate");

        // Retrieve the number of transactions.
        let num_transactions = transactions.len();

        // Perform the finalize operation on the preset finalize mode.
        atomic_finalize!(self.finalize_store(), FinalizeMode::DryRun, {
            // Initialize an iterator for ratifications before finalize.
            let pre_ratifications = ratifications.iter().filter(|r| match r {
                Ratify::Genesis(_, _) => true,
                Ratify::BlockReward(..) | Ratify::PuzzleReward(..) => false,
            });
            // Initialize an iterator for ratifications after finalize.
            let post_ratifications = ratifications.iter().filter(|r| match r {
                Ratify::Genesis(_, _) => false,
                Ratify::BlockReward(..) | Ratify::PuzzleReward(..) => true,
            });

            // Retrieve the finalize store.
            let store = self.finalize_store();

            /* Perform the ratifications before finalize. */

            if let Err(e) = Self::atomic_pre_ratify(store, state, pre_ratifications) {
                // Note: This will abort the entire atomic batch.
                return Err(format!("Failed to pre-ratify - {e}"));
            }

            /* Perform the atomic finalize over the transactions. */

            // Acquire the write lock on the process.
            // Note: Due to the highly-sensitive nature of processing all `finalize` calls,
            // we choose to acquire the write lock for the entire duration of this atomic batch.
            let process = self.process.write();

            // Initialize a list of the confirmed transactions.
            let mut confirmed = Vec::with_capacity(num_transactions);
            // Initialize a list of the aborted transactions.
            let mut aborted = Vec::new();

            // Finalize the transactions.
            'outer: for (index, transaction) in transactions.enumerate() {
                // Convert the transaction index to a u32.
                // Note: On failure, this will abort the entire atomic batch.
                let index = u32::try_from(index).map_err(|_| "Failed to convert transaction index".to_string())?;

                // Process the transaction in an isolated atomic batch.
                // - If the transaction succeeds, the finalize operations are stored.
                // - If the transaction fails, the atomic batch is aborted and no finalize operations are stored.
                let outcome = match transaction {
                    // The finalize operation here involves appending the 'stack',
                    // and adding the program to the finalize tree.
                    Transaction::Deploy(_, program_owner, deployment, fee) => {
                        match process.finalize_deployment(state, store, deployment, fee) {
                            // Construct the accepted deploy transaction.
                            Ok((_, finalize)) => {
                                ConfirmedTransaction::accepted_deploy(index, transaction.clone(), finalize)
                                    .map_err(|e| e.to_string())
                            }
                            // Construct the rejected deploy transaction.
                            Err(_error) => {
                                // Finalize the fee, to ensure it is valid.
                                if let Err(error) = process.finalize_fee(state, store, fee) {
                                    // Note: On failure, skip this transaction, and continue speculation.
                                    #[cfg(debug_assertions)]
                                    eprintln!("Failed to finalize the fee in a rejected deploy - {error}");
                                    // Store the aborted transaction.
                                    aborted.push(transaction.clone());
                                    continue 'outer;
                                }
                                // Construct the fee transaction.
                                // Note: On failure, this will abort the entire atomic batch.
                                let fee_tx = Transaction::from_fee(fee.clone()).map_err(|e| e.to_string())?;
                                // Construct the rejected deployment.
                                let rejected = Rejected::new_deployment(*program_owner, *deployment.clone());
                                // Construct the rejected deploy transaction.
                                ConfirmedTransaction::rejected_deploy(index, fee_tx, rejected)
                                    .map_err(|e| e.to_string())
                            }
                        }
                    }
                    // The finalize operation here involves calling 'update_key_value',
                    // and update the respective leaves of the finalize tree.
                    Transaction::Execute(_, execution, fee) => {
                        match process.finalize_execution(state, store, execution, fee.as_ref()) {
                            // Construct the accepted execute transaction.
                            Ok(finalize) => {
                                ConfirmedTransaction::accepted_execute(index, transaction.clone(), finalize)
                                    .map_err(|e| e.to_string())
                            }
                            // Construct the rejected execute transaction.
                            Err(_error) => match fee {
                                Some(fee) => {
                                    // Finalize the fee, to ensure it is valid.
                                    if let Err(error) = process.finalize_fee(state, store, fee) {
                                        // Note: On failure, skip this transaction, and continue speculation.
                                        #[cfg(debug_assertions)]
                                        eprintln!("Failed to finalize the fee in a rejected execute - {error}");
                                        // Store the aborted transaction.
                                        aborted.push(transaction.clone());
                                        continue 'outer;
                                    }
                                    // Construct the fee transaction.
                                    // Note: On failure, this will abort the entire atomic batch.
                                    let fee_tx = Transaction::from_fee(fee.clone()).map_err(|e| e.to_string())?;
                                    // Construct the rejected execution.
                                    let rejected = Rejected::new_execution(execution.clone());
                                    // Construct the rejected execute transaction.
                                    ConfirmedTransaction::rejected_execute(index, fee_tx, rejected)
                                        .map_err(|e| e.to_string())
                                }
                                // This is a foundational bug - the caller is violating protocol rules.
                                // Note: This will abort the entire atomic batch.
                                None => Err("Rejected execute transaction has no fee".to_string()),
                            },
                        }
                    }
                    // There are no finalize operations here.
                    // Note: This will abort the entire atomic batch.
                    Transaction::Fee(..) => Err("Cannot speculate on a fee transaction".to_string()),
                };
                lap!(timer, "Speculated on transaction '{}'", transaction.id());

                match outcome {
                    // If the transaction succeeded, store it and continue to the next transaction.
                    Ok(confirmed_transaction) => confirmed.push(confirmed_transaction),
                    // If the transaction failed, abort the entire batch.
                    Err(error) => {
                        eprintln!("Critical bug in speculate: {error}\n\n{transaction}");
                        // Note: This will abort the entire atomic batch.
                        return Err(format!("Failed to speculate on transaction - {error}"));
                    }
                }
            }

            // Ensure all transactions were processed.
            if confirmed.len() != num_transactions {
                // Note: This will abort the entire atomic batch.
                return Err("Not all transactions were processed in 'VM::atomic_speculate'".to_string());
            }

            /* Perform the ratifications after finalize. */

            if let Err(e) = Self::atomic_post_ratify(store, state, post_ratifications, solutions) {
                // Note: This will abort the entire atomic batch.
                return Err(format!("Failed to post-ratify - {e}"));
            }

            finish!(timer);

            // Get the write batch of the finalize store.
            let write_batch = match store.get_write_batch() {
                Ok(write_batch) => write_batch,
                Err(e) => {
                    // Note: This will abort the entire atomic batch.
                    return Err(format!("Failed to get the write batch - {e}"));
                }
            };

            // On return, 'atomic_finalize!' will abort the batch, and return the confirmed transactions, aborted transactions and write batch.
            Ok((confirmed, aborted, write_batch))
        })
    }

    /// Performs atomic finalization over a list of transactions. Returns the write batch.
    #[inline]
    fn atomic_finalize(
        &self,
        state: FinalizeGlobalState,
        ratifications: &[Ratify<N>],
        solutions: Option<&CoinbaseSolution<N>>,
        transactions: &Transactions<N>,
    ) -> Result<Vec<u8>> {
        let timer = timer!("VM::atomic_finalize");

        // Perform the finalize operation on the preset finalize mode.
        atomic_finalize!(self.finalize_store(), FinalizeMode::RealRun, {
            // Initialize an iterator for ratifications before finalize.
            let pre_ratifications = ratifications.iter().filter(|r| match r {
                Ratify::Genesis(_, _) => true,
                Ratify::BlockReward(..) | Ratify::PuzzleReward(..) => false,
            });
            // Initialize an iterator for ratifications after finalize.
            let post_ratifications = ratifications.iter().filter(|r| match r {
                Ratify::Genesis(_, _) => false,
                Ratify::BlockReward(..) | Ratify::PuzzleReward(..) => true,
            });

            // Retrieve the finalize store.
            let store = self.finalize_store();

            /* Perform the ratifications before finalize. */

            if let Err(e) = Self::atomic_pre_ratify(store, state, pre_ratifications) {
                // Note: This will abort the entire atomic batch.
                return Err(format!("Failed to pre-ratify - {e}"));
            }

            /* Perform the atomic finalize over the transactions. */

            // Acquire the write lock on the process.
            // Note: Due to the highly-sensitive nature of processing all `finalize` calls,
            // we choose to acquire the write lock for the entire duration of this atomic batch.
            let mut process = self.process.write();

            // Initialize a list for the deployed stacks.
            let mut stacks = Vec::new();

            // Finalize the transactions.
            for (index, transaction) in transactions.iter().enumerate() {
                // Convert the transaction index to a u32.
                // Note: On failure, this will abort the entire atomic batch.
                let index = u32::try_from(index).map_err(|_| "Failed to convert transaction index".to_string())?;
                // Ensure the index matches the expected index.
                if index != transaction.index() {
                    // Note: This will abort the entire atomic batch.
                    return Err(format!("Mismatch in {} transaction index", transaction.variant()));
                }
                // Process the transaction in an isolated atomic batch.
                // - If the transaction succeeds, the finalize operations are stored.
                // - If the transaction fails, the atomic batch is aborted and no finalize operations are stored.
                let outcome: Result<(), String> = match transaction {
                    ConfirmedTransaction::AcceptedDeploy(_, transaction, finalize) => {
                        // Extract the deployment and fee from the transaction.
                        let (deployment, fee) = match transaction {
                            Transaction::Deploy(_, _, deployment, fee) => (deployment, fee),
                            // Note: This will abort the entire atomic batch.
                            _ => return Err("Expected deploy transaction".to_string()),
                        };
                        // The finalize operation here involves appending the 'stack', and adding the program to the finalize tree.
                        match process.finalize_deployment(state, store, deployment, fee) {
                            // Ensure the finalize operations match the expected.
                            Ok((stack, finalize_operations)) => match finalize == &finalize_operations {
                                // Store the stack.
                                true => stacks.push(stack),
                                // Note: This will abort the entire atomic batch.
                                false => {
                                    return Err(format!(
                                        "Mismatch in finalize operations for an accepted deploy - (found: {finalize_operations:?}, expected: {finalize:?})"
                                    ));
                                }
                            },
                            // Note: This will abort the entire atomic batch.
                            Err(error) => {
                                return Err(format!("Failed to finalize an accepted deploy transaction - {error}"));
                            }
                        };
                        Ok(())
                    }
                    ConfirmedTransaction::AcceptedExecute(_, transaction, finalize) => {
                        // Extract the execution and fee from the transaction.
                        let (execution, fee) = match transaction {
                            Transaction::Execute(_, execution, fee) => (execution, fee),
                            // Note: This will abort the entire atomic batch.
                            _ => return Err("Expected execute transaction".to_string()),
                        };
                        // The finalize operation here involves calling 'update_key_value',
                        // and update the respective leaves of the finalize tree.
                        match process.finalize_execution(state, store, execution, fee.as_ref()) {
                            // Ensure the finalize operations match the expected.
                            Ok(finalize_operations) => {
                                if finalize != &finalize_operations {
                                    // Note: This will abort the entire atomic batch.
                                    return Err(format!(
                                        "Mismatch in finalize operations for an accepted execute - (found: {finalize_operations:?}, expected: {finalize:?})"
                                    ));
                                }
                            }
                            // Note: This will abort the entire atomic batch.
                            Err(error) => {
                                return Err(format!("Failed to finalize an accepted execute transaction - {error}"));
                            }
                        }
                        Ok(())
                    }
                    ConfirmedTransaction::RejectedDeploy(_, Transaction::Fee(_, fee), rejected) => {
                        // Extract the rejected deployment.
                        let Some(deployment) = rejected.deployment() else {
                            // Note: This will abort the entire atomic batch.
                            return Err("Expected rejected deployment".to_string());
                        };
                        // Compute the expected deployment ID.
                        let Ok(expected_deployment_id) = deployment.to_deployment_id() else {
                            // Note: This will abort the entire atomic batch.
                            return Err("Failed to compute the deployment ID for a rejected deployment".to_string());
                        };
                        // Retrieve the candidate deployment ID.
                        let Ok(candidate_deployment_id) = fee.deployment_or_execution_id() else {
                            // Note: This will abort the entire atomic batch.
                            return Err("Failed to retrieve the deployment ID from the fee".to_string());
                        };
                        // Ensure this fee corresponds to the deployment.
                        if candidate_deployment_id != expected_deployment_id {
                            // Note: This will abort the entire atomic batch.
                            return Err("Mismatch in fee for a rejected deploy transaction".to_string());
                        }
                        // Attempt to finalize the deployment, which should fail.
                        #[cfg(debug_assertions)]
                        if process.finalize_deployment(state, store, deployment, fee).is_ok() {
                            // Note: This will abort the entire atomic batch.
                            return Err("Failed to reject a rejected deploy transaction".to_string());
                        }
                        // Lastly, finalize the fee.
                        if let Err(_error) = process.finalize_fee(state, store, fee) {
                            // Note: This will abort the entire atomic batch.
                            return Err("Failed to finalize the fee in a rejected deploy transaction".to_string());
                        }
                        Ok(())
                    }
                    ConfirmedTransaction::RejectedExecute(_, Transaction::Fee(_, fee), rejected) => {
                        // Extract the rejected execution.
                        let Some(execution) = rejected.execution() else {
                            // Note: This will abort the entire atomic batch.
                            return Err("Expected rejected execution".to_string());
                        };
                        // Compute the expected execution ID.
                        let Ok(expected_execution_id) = execution.to_execution_id() else {
                            // Note: This will abort the entire atomic batch.
                            return Err("Failed to compute the execution ID for a rejected execution".to_string());
                        };
                        // Retrieve the candidate execution ID.
                        let Ok(candidate_execution_id) = fee.deployment_or_execution_id() else {
                            // Note: This will abort the entire atomic batch.
                            return Err("Failed to retrieve the execution ID from the fee".to_string());
                        };
                        // Ensure this fee corresponds to the execution.
                        if candidate_execution_id != expected_execution_id {
                            // Note: This will abort the entire atomic batch.
                            return Err("Mismatch in fee for a rejected execute transaction".to_string());
                        }
                        // Attempt to finalize the execution, which should fail.
                        #[cfg(debug_assertions)]
                        if process.finalize_execution(state, store, execution, Some(fee)).is_ok() {
                            // Note: This will abort the entire atomic batch.
                            return Err("Failed to reject a rejected execute transaction".to_string());
                        }
                        // Lastly, finalize the fee.
                        if let Err(_error) = process.finalize_fee(state, store, fee) {
                            // Note: This will abort the entire atomic batch.
                            return Err("Failed to finalize the fee in a rejected execute transaction".to_string());
                        }
                        Ok(())
                    }
                    // Note: This will abort the entire atomic batch.
                    _ => return Err("Invalid confirmed transaction type".to_string()),
                };
                lap!(timer, "Finalizing transaction {}", transaction.id());

                match outcome {
                    // If the transaction succeeded to finalize, continue to the next transaction.
                    Ok(()) => (),
                    // If the transaction failed to finalize, abort and continue to the next transaction.
                    Err(error) => {
                        eprintln!("Critical bug in finalize: {error}\n\n{transaction}");
                        // Note: This will abort the entire atomic batch.
                        return Err(format!("Failed to finalize on transaction - {error}"));
                    }
                }
            }

            /* Perform the ratifications after finalize. */

            if let Err(e) = Self::atomic_post_ratify(store, state, post_ratifications, solutions) {
                // Note: This will abort the entire atomic batch.
                return Err(format!("Failed to post-ratify - {e}"));
            }

            /* Start the commit process. */

            // Commit all of the stacks to the process.
            if !stacks.is_empty() {
                stacks.into_iter().for_each(|stack| process.add_stack(stack))
            }

            // Get the write batch of the finalize store.
            let write_batch = match store.get_write_batch() {
                Ok(write_batch) => write_batch,
                Err(e) => {
                    // Note: This will abort the entire atomic batch.
                    return Err(format!("Failed to get the write batch - {e}"));
                }
            };

            finish!(timer); // <- Note: This timer does **not** include the time to write batch to DB.

            Ok(write_batch)
        })
    }

    /// Performs the pre-ratifications before finalizing transactions.
    #[inline]
    fn atomic_pre_ratify<'a>(
        store: &FinalizeStore<N, C::FinalizeStorage>,
        state: FinalizeGlobalState,
        pre_ratifications: impl Iterator<Item = &'a Ratify<N>>,
    ) -> Result<()> {
        // Construct the program ID.
        let program_id = ProgramID::from_str("credits.aleo")?;
        // Construct the committee mapping name.
        let committee_mapping = Identifier::from_str("committee")?;
        // Construct the bonded mapping name.
        let bonded_mapping = Identifier::from_str("bonded")?;
        // Construct the account mapping name.
        let account_mapping = Identifier::from_str("account")?;

        // Iterate over the ratifications.
        for ratify in pre_ratifications {
            match ratify {
                Ratify::Genesis(committee, public_balances) => {
                    // Ensure this is the genesis block.
                    ensure!(state.block_height() == 0, "Ratify::Genesis(..) expected a genesis block");
                    // Ensure the genesis committee round is 0.
                    ensure!(
                        committee.starting_round() == 0,
                        "Ratify::Genesis(..) expected a genesis committee round of 0"
                    );

                    // Initialize the stakers.
                    let mut stakers = IndexMap::with_capacity(committee.members().len());
                    // Iterate over the committee members.
                    for (validator, (microcredits, _)) in committee.members() {
                        // Insert the validator into the stakers.
                        stakers.insert(*validator, (*validator, *microcredits));
                    }

                    // Construct the next committee map and next bonded map.
                    let (next_committee_map, next_bonded_map) =
                        to_next_commitee_map_and_bonded_map(committee, &stakers);

                    // Insert the next committee into storage.
                    store.committee_store().insert(state.block_height(), committee.clone())?;
                    // Replace the committee mapping in storage.
                    store.replace_mapping(&program_id, &committee_mapping, next_committee_map)?;
                    // Replace the bonded mapping in storage.
                    store.replace_mapping(&program_id, &bonded_mapping, next_bonded_map)?;

                    // Iterate over the public balances.
                    for (address, amount) in public_balances {
                        // Construct the key.
                        let key = Plaintext::from(Literal::Address(*address));
                        // Retrieve the current public balance.
                        let value = store.get_value_speculative(&program_id, &account_mapping, &key)?;
                        // Compute the next public balance.
                        let next_value = Value::from(Literal::U64(U64::new(match value {
                            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(value), _))) => {
                                (*value).saturating_add(*amount)
                            }
                            None => *amount,
                            v => bail!("Critical bug in pre-ratify - Invalid public balance type ({v:?})"),
                        })));
                        // Update the public balance in finalize storage.
                        store.update_key_value(&program_id, &account_mapping, key, next_value)?;
                    }
                }
                Ratify::BlockReward(..) | Ratify::PuzzleReward(..) => continue,
            }
        }
        Ok(())
    }

    /// Performs the post-ratifications after finalizing transactions.
    #[inline]
    fn atomic_post_ratify<'a>(
        store: &FinalizeStore<N, C::FinalizeStorage>,
        state: FinalizeGlobalState,
        post_ratifications: impl Iterator<Item = &'a Ratify<N>>,
        solutions: Option<&CoinbaseSolution<N>>,
    ) -> Result<()> {
        // Construct the program ID.
        let program_id = ProgramID::from_str("credits.aleo")?;
        // Construct the committee mapping name.
        let committee_mapping = Identifier::from_str("committee")?;
        // Construct the bonded mapping name.
        let bonded_mapping = Identifier::from_str("bonded")?;
        // Construct the account mapping name.
        let account_mapping = Identifier::from_str("account")?;

        // Iterate over the ratifications.
        for ratify in post_ratifications {
            match ratify {
                Ratify::Genesis(..) => continue,
                Ratify::BlockReward(block_reward) => {
                    // Retrieve the committee from storage.
                    let current_committee = store.committee_store().current_committee()?;
                    // Retrieve the committee mapping from storage.
                    let current_committee_map = store.get_mapping_speculative(&program_id, &committee_mapping)?;
                    // Retrieve the bonded mapping from storage.
                    let current_bonded_map = store.get_mapping_speculative(&program_id, &bonded_mapping)?;
                    // Convert the bonded map into stakers.
                    let current_stakers = bonded_map_into_stakers(current_bonded_map)?;

                    // Ensure the committee matches the committee mapping.
                    ensure_committee_matches(&current_committee, &current_committee_map)?;
                    // Ensure the committee matches the bonded mapping.
                    ensure_stakers_matches(&current_committee, &current_stakers)?;

                    // Compute the updated stakers, using the committee and block reward.
                    let next_stakers = staking_rewards(&current_stakers, &current_committee, *block_reward);
                    // Compute the updated committee, using the stakers.
                    let next_committee = to_next_committee(&current_committee, state.block_round(), &next_stakers)?;
                    // Construct the next committee map and next bonded map.
                    let (next_committee_map, next_bonded_map) =
                        to_next_commitee_map_and_bonded_map(&next_committee, &next_stakers);

                    // Insert the next committee into storage.
                    store.committee_store().insert(state.block_height(), next_committee)?;
                    // Replace the committee mapping in storage.
                    store.replace_mapping(&program_id, &committee_mapping, next_committee_map)?;
                    // Replace the bonded mapping in storage.
                    store.replace_mapping(&program_id, &bonded_mapping, next_bonded_map)?;
                }
                Ratify::PuzzleReward(puzzle_reward) => {
                    // If the puzzle reward is zero, skip.
                    if *puzzle_reward == 0 {
                        continue;
                    }
                    // Retrieve the solutions.
                    let Some(solutions) = solutions else {
                        continue;
                    };
                    // Compute the proof targets, with the corresponding addresses.
                    let proof_targets = solutions
                        .partial_solutions()
                        .iter()
                        .map(|s| Ok((s.address(), s.to_target()?)))
                        .collect::<Result<Vec<_>>>()?;
                    // Calculate the proving rewards.
                    let proving_rewards = proving_rewards(proof_targets, *puzzle_reward);
                    // Iterate over the proving rewards.
                    for (address, amount) in proving_rewards {
                        // Construct the key.
                        let key = Plaintext::from(Literal::Address(address));
                        // Retrieve the current public balance.
                        let value = store.get_value_speculative(&program_id, &account_mapping, &key)?;
                        // Compute the next public balance.
                        let next_value = Value::from(Literal::U64(U64::new(match value {
                            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(value), _))) => {
                                (*value).saturating_add(amount)
                            }
                            None => amount,
                            v => bail!("Critical bug in post-ratify puzzle reward- Invalid amount ({v:?})"),
                        })));
                        // Update the public balance in finalize storage.
                        store.update_key_value(&program_id, &account_mapping, key, next_value)?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::{test_helpers, test_helpers::sample_finalize_state};
    use console::{
        account::{Address, PrivateKey, ViewKey},
        program::{Ciphertext, Entry, Record},
        types::Field,
    };
    use ledger_block::{Block, Header, Metadata, Transaction, Transition};
    use ledger_store::helpers::memory::ConsensusMemory;
    use synthesizer_program::Program;

    use rand::distributions::DistString;

    type CurrentNetwork = test_helpers::CurrentNetwork;

    /// Sample a new program and deploy it to the VM. Returns the program name.
    fn new_program_deployment<R: Rng + CryptoRng>(
        vm: &VM<CurrentNetwork, ConsensusMemory<CurrentNetwork>>,
        private_key: &PrivateKey<CurrentNetwork>,
        previous_block: &Block<CurrentNetwork>,
        unspent_records: &mut Vec<Record<CurrentNetwork, Ciphertext<CurrentNetwork>>>,
        rng: &mut R,
    ) -> Result<(String, Block<CurrentNetwork>)> {
        let program_name = format!("a{}.aleo", Alphanumeric.sample_string(rng, 8).to_lowercase());

        let program = Program::<CurrentNetwork>::from_str(&format!(
            "
program {program_name};

mapping account:
    // The token owner.
    key owner as address.public;
    // The token amount.
    value amount as u64.public;

function mint_public:
    input r0 as address.public;
    input r1 as u64.public;
    finalize r0 r1;

finalize mint_public:
    input r0 as address.public;
    input r1 as u64.public;

    get.or_use account[r0] 0u64 into r2;
    add r2 r1 into r3;
    set r3 into account[r0];

function transfer_public:
    input r0 as address.public;
    input r1 as u64.public;

    finalize self.caller r0 r1;

finalize transfer_public:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as u64.public;

    get.or_use account[r0] 0u64 into r3;
    get.or_use account[r1] 0u64 into r4;

    sub r3 r2 into r5;
    add r4 r2 into r6;

    set r5 into account[r0];
    set r6 into account[r1];"
        ))?;

        // Prepare the additional fee.
        let view_key = ViewKey::<CurrentNetwork>::try_from(private_key)?;
        let credits = Some(unspent_records.pop().unwrap().decrypt(&view_key)?);

        // Deploy.
        let transaction = vm.deploy(private_key, &program, credits, 10, None, rng)?;

        // Construct the new block.
        let next_block = sample_next_block(vm, private_key, &[transaction], previous_block, unspent_records, rng)?;

        Ok((program_name, next_block))
    }

    /// Construct a new block based on the given transactions.
    fn sample_next_block<R: Rng + CryptoRng>(
        vm: &VM<CurrentNetwork, ConsensusMemory<CurrentNetwork>>,
        private_key: &PrivateKey<CurrentNetwork>,
        transactions: &[Transaction<CurrentNetwork>],
        previous_block: &Block<CurrentNetwork>,
        unspent_records: &mut Vec<Record<CurrentNetwork, Ciphertext<CurrentNetwork>>>,
        rng: &mut R,
    ) -> Result<Block<CurrentNetwork>> {
        // Construct the new block header.
        let (transactions, _, finalize_root) =
            vm.speculate(sample_finalize_state(1), &[], None, transactions.iter())?;
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
            CurrentNetwork::GENESIS_TIMESTAMP + 1,
        )?;

        let header = Header::from(
            vm.block_store().current_state_root(),
            transactions.to_transactions_root().unwrap(),
            finalize_root,
            crate::vm::test_helpers::sample_ratifications_root(),
            Field::zero(),
            metadata,
        )?;

        let block = Block::new_beacon(private_key, previous_block.hash(), header, vec![], None, transactions, rng)?;

        // Track the new records.
        let new_records = block
            .transitions()
            .cloned()
            .flat_map(Transition::into_records)
            .map(|(_, record)| record)
            .collect::<Vec<_>>();
        unspent_records.extend(new_records);

        Ok(block)
    }

    /// Generate split transactions for the unspent records.
    fn generate_splits<R: Rng + CryptoRng>(
        vm: &VM<CurrentNetwork, ConsensusMemory<CurrentNetwork>>,
        private_key: &PrivateKey<CurrentNetwork>,
        previous_block: &Block<CurrentNetwork>,
        unspent_records: &mut Vec<Record<CurrentNetwork, Ciphertext<CurrentNetwork>>>,
        rng: &mut R,
    ) -> Result<Block<CurrentNetwork>> {
        // Prepare the additional fee.
        let view_key = ViewKey::<CurrentNetwork>::try_from(private_key)?;

        // Generate split transactions.
        let mut transactions = Vec::new();
        while !unspent_records.is_empty() {
            let record = unspent_records.pop().unwrap().decrypt(&view_key)?;

            // Fetch the record balance and divide it in half.
            let split_balance = match record.find(&[Identifier::from_str("microcredits")?]) {
                Ok(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => *amount / 2,
                _ => bail!("fee record does not contain a microcredits entry"),
            };

            // Prepare the inputs.
            let inputs = [
                Value::<CurrentNetwork>::Record(record),
                Value::<CurrentNetwork>::from_str(&format!("{split_balance}u64")).unwrap(),
            ]
            .into_iter();

            // Execute.
            let transaction = vm.execute(private_key, ("credits.aleo", "split"), inputs, None, 0, None, rng).unwrap();

            transactions.push(transaction);
        }

        // Construct the new block.
        sample_next_block(vm, private_key, &transactions, previous_block, unspent_records, rng)
    }

    /// Create an execution transaction.
    fn create_execution(
        vm: &VM<CurrentNetwork, ConsensusMemory<CurrentNetwork>>,
        caller_private_key: PrivateKey<CurrentNetwork>,
        program_id: &str,
        function_name: &str,
        inputs: Vec<Value<CurrentNetwork>>,
        unspent_records: &mut Vec<Record<CurrentNetwork, Ciphertext<CurrentNetwork>>>,
        rng: &mut TestRng,
    ) -> Transaction<CurrentNetwork> {
        assert!(vm.contains_program(&ProgramID::from_str(program_id).unwrap()));

        // Prepare the additional fee.
        let view_key = ViewKey::<CurrentNetwork>::try_from(caller_private_key).unwrap();
        let credits = Some(unspent_records.pop().unwrap().decrypt(&view_key).unwrap());

        // Execute.
        let transaction = vm
            .execute(&caller_private_key, (program_id, function_name), inputs.into_iter(), credits, 1, None, rng)
            .unwrap();
        // Verify.
        assert!(vm.verify_transaction(&transaction, None));

        // Return the transaction.
        transaction
    }

    /// Sample a public mint transaction.
    fn sample_mint_public(
        vm: &VM<CurrentNetwork, ConsensusMemory<CurrentNetwork>>,
        caller_private_key: PrivateKey<CurrentNetwork>,
        program_id: &str,
        recipient: Address<CurrentNetwork>,
        amount: u64,
        unspent_records: &mut Vec<Record<CurrentNetwork, Ciphertext<CurrentNetwork>>>,
        rng: &mut TestRng,
    ) -> Transaction<CurrentNetwork> {
        let inputs = vec![
            Value::<CurrentNetwork>::from_str(&recipient.to_string()).unwrap(),
            Value::<CurrentNetwork>::from_str(&format!("{amount}u64")).unwrap(),
        ];

        create_execution(vm, caller_private_key, program_id, "mint_public", inputs, unspent_records, rng)
    }

    /// Sample a public transfer transaction.
    fn sample_transfer_public(
        vm: &VM<CurrentNetwork, ConsensusMemory<CurrentNetwork>>,
        caller_private_key: PrivateKey<CurrentNetwork>,
        program_id: &str,
        recipient: Address<CurrentNetwork>,
        amount: u64,
        unspent_records: &mut Vec<Record<CurrentNetwork, Ciphertext<CurrentNetwork>>>,
        rng: &mut TestRng,
    ) -> Transaction<CurrentNetwork> {
        let inputs = vec![
            Value::<CurrentNetwork>::from_str(&recipient.to_string()).unwrap(),
            Value::<CurrentNetwork>::from_str(&format!("{amount}u64")).unwrap(),
        ];

        create_execution(vm, caller_private_key, program_id, "transfer_public", inputs, unspent_records, rng)
    }

    /// A helper method to construct the rejected transaction format for `atomic_finalize`.
    fn reject(index: u32, transaction: &Transaction<CurrentNetwork>) -> ConfirmedTransaction<CurrentNetwork> {
        match transaction {
            Transaction::Execute(_, execution, fee) => ConfirmedTransaction::RejectedExecute(
                index,
                Transaction::from_fee(fee.clone().unwrap()).unwrap(),
                Rejected::new_execution(execution.clone()),
            ),
            _ => panic!("only reject execution transactions"),
        }
    }

    #[test]
    fn test_finalize_duplicate_deployment() {
        let rng = &mut TestRng::default();

        let vm = crate::vm::test_helpers::sample_vm();

        // Fetch a deployment transaction.
        let deployment_transaction = crate::vm::test_helpers::sample_deployment_transaction(rng);
        let deployment_transaction_id = deployment_transaction.id();

        // Construct the program name.
        let program_id = ProgramID::from_str("testing.aleo").unwrap();

        // Prepare the confirmed transactions.
        let (confirmed_transactions, _, _) =
            vm.speculate(sample_finalize_state(1), &[], None, [deployment_transaction.clone()].iter()).unwrap();

        // Ensure the VM does not contain this program.
        assert!(!vm.contains_program(&program_id));

        // Finalize the transaction.
        assert!(vm.finalize(sample_finalize_state(1), &[], None, &confirmed_transactions).is_ok());

        // Ensure the VM contains this program.
        assert!(vm.contains_program(&program_id));

        // Ensure the VM can't redeploy the same transaction.
        assert!(vm.finalize(sample_finalize_state(1), &[], None, &confirmed_transactions).is_err());

        // Ensure the VM contains this program.
        assert!(vm.contains_program(&program_id));

        // Ensure the dry run of the redeployment will cause a reject transaction to be created.
        let (candidate_transactions, _, _) =
            vm.atomic_speculate(sample_finalize_state(1), &[], None, [deployment_transaction].iter()).unwrap();
        assert_eq!(candidate_transactions.len(), 1);
        assert!(matches!(candidate_transactions[0], ConfirmedTransaction::RejectedDeploy(..)));

        // Check that the unconfirmed transaction id of the rejected deployment is correct.
        assert_eq!(candidate_transactions[0].unconfirmed_id().unwrap(), deployment_transaction_id);
    }

    #[test]
    fn test_atomic_finalize_many() {
        let rng = &mut TestRng::default();

        // Sample a private key and address for the caller.
        let caller_private_key = test_helpers::sample_genesis_private_key(rng);
        let caller_address = Address::try_from(&caller_private_key).unwrap();

        // Sample a private key and address for the recipient.
        let recipient_private_key = PrivateKey::new(rng).unwrap();
        let recipient_address = Address::try_from(&recipient_private_key).unwrap();

        // Initialize the vm.
        let vm = test_helpers::sample_vm_with_genesis_block(rng);

        // Deploy a new program.
        let genesis =
            vm.block_store().get_block(&vm.block_store().get_block_hash(0).unwrap().unwrap()).unwrap().unwrap();

        // Get the unspent records.
        let mut unspent_records = genesis
            .transitions()
            .cloned()
            .flat_map(Transition::into_records)
            .map(|(_, record)| record)
            .collect::<Vec<_>>();

        // Construct the deployment block.
        let (program_id, deployment_block) =
            new_program_deployment(&vm, &caller_private_key, &genesis, &mut unspent_records, rng).unwrap();

        // Add the deployment block to the VM.
        vm.add_next_block(&deployment_block).unwrap();

        // Generate more records to use for the next block.
        let splits_block =
            generate_splits(&vm, &caller_private_key, &deployment_block, &mut unspent_records, rng).unwrap();

        // Add the splits block to the VM.
        vm.add_next_block(&splits_block).unwrap();

        // Construct the initial mint.
        let initial_mint =
            sample_mint_public(&vm, caller_private_key, &program_id, caller_address, 20, &mut unspent_records, rng);
        let initial_mint_block =
            sample_next_block(&vm, &caller_private_key, &[initial_mint], &splits_block, &mut unspent_records, rng)
                .unwrap();

        // Add the block to the vm.
        vm.add_next_block(&initial_mint_block).unwrap();

        // Construct a mint and a transfer.
        let mint_10 =
            sample_mint_public(&vm, caller_private_key, &program_id, caller_address, 10, &mut unspent_records, rng);
        let mint_20 =
            sample_mint_public(&vm, caller_private_key, &program_id, caller_address, 20, &mut unspent_records, rng);
        let transfer_10 = sample_transfer_public(
            &vm,
            caller_private_key,
            &program_id,
            recipient_address,
            10,
            &mut unspent_records,
            rng,
        );
        let transfer_20 = sample_transfer_public(
            &vm,
            caller_private_key,
            &program_id,
            recipient_address,
            20,
            &mut unspent_records,
            rng,
        );
        let transfer_30 = sample_transfer_public(
            &vm,
            caller_private_key,
            &program_id,
            recipient_address,
            30,
            &mut unspent_records,
            rng,
        );

        // TODO (raychu86): Confirm that the finalize_operations here are correct.

        // Starting Balance = 20
        // Mint_10 -> Balance = 20 + 10  = 30
        // Transfer_10 -> Balance = 30 - 10 = 20
        // Transfer_20 -> Balance = 20 - 20 = 0
        {
            let transactions = [mint_10.clone(), transfer_10.clone(), transfer_20.clone()];
            let (confirmed_transactions, _, _) =
                vm.atomic_speculate(sample_finalize_state(1), &[], None, transactions.iter()).unwrap();

            // Assert that all the transactions are accepted.
            assert_eq!(confirmed_transactions.len(), 3);
            confirmed_transactions.iter().for_each(|confirmed_tx| assert!(confirmed_tx.is_accepted()));

            assert_eq!(confirmed_transactions[0].transaction(), &mint_10);
            assert_eq!(confirmed_transactions[1].transaction(), &transfer_10);
            assert_eq!(confirmed_transactions[2].transaction(), &transfer_20);
        }

        // Starting Balance = 20
        // Transfer_20 -> Balance = 20 - 20 = 0
        // Mint_10 -> Balance = 0 + 10 = 10
        // Mint_20 -> Balance = 10 + 20 = 30
        // Transfer_30 -> Balance = 30 - 30 = 0
        {
            let transactions = [transfer_20.clone(), mint_10.clone(), mint_20.clone(), transfer_30.clone()];
            let (confirmed_transactions, _, _) =
                vm.atomic_speculate(sample_finalize_state(1), &[], None, transactions.iter()).unwrap();

            // Assert that all the transactions are accepted.
            assert_eq!(confirmed_transactions.len(), 4);
            confirmed_transactions.iter().for_each(|confirmed_tx| assert!(confirmed_tx.is_accepted()));

            // Ensure that the transactions are in the correct order.
            assert_eq!(confirmed_transactions[0].transaction(), &transfer_20);
            assert_eq!(confirmed_transactions[1].transaction(), &mint_10);
            assert_eq!(confirmed_transactions[2].transaction(), &mint_20);
            assert_eq!(confirmed_transactions[3].transaction(), &transfer_30);
        }

        // Starting Balance = 20
        // Transfer_20 -> Balance = 20 - 20 = 0
        // Transfer_10 -> Balance = 0 - 10 = -10 (should be rejected)
        {
            let transactions = [transfer_20.clone(), transfer_10.clone()];
            let (confirmed_transactions, _, _) =
                vm.atomic_speculate(sample_finalize_state(1), &[], None, transactions.iter()).unwrap();

            // Assert that the accepted and rejected transactions are correct.
            assert_eq!(confirmed_transactions.len(), 2);

            assert!(confirmed_transactions[0].is_accepted());
            assert!(confirmed_transactions[1].is_rejected());

            assert_eq!(confirmed_transactions[0].transaction(), &transfer_20);
            assert_eq!(confirmed_transactions[1], reject(1, &transfer_10));
        }

        // Starting Balance = 20
        // Mint_20 -> Balance = 20 + 20
        // Transfer_30 -> Balance = 40 - 30 = 10
        // Transfer_20 -> Balance = 10 - 20 = -10 (should be rejected)
        // Transfer_10 -> Balance = 10 - 10 = 0
        {
            let transactions = [mint_20.clone(), transfer_30.clone(), transfer_20.clone(), transfer_10.clone()];
            let (confirmed_transactions, _, _) =
                vm.atomic_speculate(sample_finalize_state(1), &[], None, transactions.iter()).unwrap();

            // Assert that the accepted and rejected transactions are correct.
            assert_eq!(confirmed_transactions.len(), 4);

            assert!(confirmed_transactions[0].is_accepted());
            assert!(confirmed_transactions[1].is_accepted());
            assert!(confirmed_transactions[2].is_rejected());
            assert!(confirmed_transactions[3].is_accepted());

            assert_eq!(confirmed_transactions[0].transaction(), &mint_20);
            assert_eq!(confirmed_transactions[1].transaction(), &transfer_30);
            assert_eq!(confirmed_transactions[2], reject(2, &transfer_20));
            assert_eq!(confirmed_transactions[3].transaction(), &transfer_10);
        }
    }

    #[test]
    fn test_finalize_catch_halt() {
        let rng = &mut TestRng::default();

        // Sample a private key, view key, and address for the caller.
        let caller_private_key = test_helpers::sample_genesis_private_key(rng);
        let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();

        for finalize_logic in &[
            "finalize ped_hash:
    input r0 as u128.public;
    hash.ped64 r0 into r1 as field;
    set r1 into hashes[r0];",
            "finalize ped_hash:
    input r0 as u128.public;
    div r0 0u128 into r1;",
        ] {
            // Initialize the vm.
            let vm = test_helpers::sample_vm_with_genesis_block(rng);

            // Deploy a new program.
            let genesis =
                vm.block_store().get_block(&vm.block_store().get_block_hash(0).unwrap().unwrap()).unwrap().unwrap();

            // Get the unspent records.
            let mut unspent_records = genesis
                .transitions()
                .cloned()
                .flat_map(Transition::into_records)
                .map(|(_, record)| record)
                .collect::<Vec<_>>();

            // Create a program that will always cause a E::halt in the finalize execution.
            let program_id = "testing.aleo";
            let program = Program::<CurrentNetwork>::from_str(&format!(
                "
program {program_id};

mapping hashes:
    key preimage as u128.public;
    value val as field.public;

function ped_hash:
    input r0 as u128.public;
    // hash.ped64 r0 into r1 as field; // <--- This will cause a E::halt.
    finalize r0;

{finalize_logic}"
            ))
            .unwrap();

            let credits = Some(unspent_records.pop().unwrap().decrypt(&caller_view_key).unwrap());

            // Deploy the program.
            let deployment_transaction = vm.deploy(&caller_private_key, &program, credits, 10, None, rng).unwrap();

            // Construct the deployment block.
            let deployment_block = sample_next_block(
                &vm,
                &caller_private_key,
                &[deployment_transaction],
                &genesis,
                &mut unspent_records,
                rng,
            )
            .unwrap();

            // Add the deployment block to the VM.
            vm.add_next_block(&deployment_block).unwrap();

            // Construct a transaction that will cause a E::halt in the finalize execution.
            let inputs = vec![Value::<CurrentNetwork>::from_str("1u128").unwrap()];
            let transaction =
                create_execution(&vm, caller_private_key, program_id, "ped_hash", inputs, &mut unspent_records, rng);

            // Speculatively execute the transaction. Ensure that this call does not panic and returns a rejected transaction.
            let (confirmed_transactions, _, _) =
                vm.speculate(sample_finalize_state(1), &[], None, [transaction.clone()].iter()).unwrap();

            // Ensure that the transaction is rejected.
            assert_eq!(confirmed_transactions.len(), 1);
            assert!(transaction.is_execute());
            if let Transaction::Execute(_, execution, fee) = transaction {
                let fee_transaction = Transaction::from_fee(fee.unwrap()).unwrap();
                let expected_confirmed_transaction =
                    ConfirmedTransaction::RejectedExecute(0, fee_transaction, Rejected::new_execution(execution));

                let confirmed_transaction = confirmed_transactions.iter().next().unwrap();
                assert_eq!(confirmed_transaction, &expected_confirmed_transaction);
            }
        }
    }

    #[test]
    fn test_rejected_transaction_should_not_update_storage() {
        let rng = &mut TestRng::default();

        // Sample a private key.
        let private_key = test_helpers::sample_genesis_private_key(rng);
        let address = Address::try_from(&private_key).unwrap();

        // Initialize the vm.
        let vm = test_helpers::sample_vm_with_genesis_block(rng);

        // Deploy a new program.
        let genesis =
            vm.block_store().get_block(&vm.block_store().get_block_hash(0).unwrap().unwrap()).unwrap().unwrap();

        // Get the unspent records.
        let mut unspent_records = genesis
            .transitions()
            .cloned()
            .flat_map(Transition::into_records)
            .map(|(_, record)| record)
            .collect::<Vec<_>>();

        // Generate more records to use for the next block.
        let splits_block = generate_splits(&vm, &private_key, &genesis, &mut unspent_records, rng).unwrap();

        // Add the splits block to the VM.
        vm.add_next_block(&splits_block).unwrap();

        // Construct the deployment block.
        let deployment_block = {
            let program = Program::<CurrentNetwork>::from_str(
                "
program testing.aleo;

mapping entries:
    key owner as address.public;
    value data as u8.public;

function compute:
    input r0 as u8.public;
    finalize self.caller r0;

finalize compute:
    input r0 as address.public;
    input r1 as u8.public;
    get.or_use entries[r0] r1 into r2;
    add r1 r2 into r3;
    set r3 into entries[r0];
    get entries[r0] into r4;
    add r4 r1 into r5;
    set r5 into entries[r0];
",
            )
            .unwrap();

            // Prepare the additional fee.
            let view_key = ViewKey::<CurrentNetwork>::try_from(private_key).unwrap();
            let credits = Some(unspent_records.pop().unwrap().decrypt(&view_key).unwrap());

            // Deploy.
            let transaction = vm.deploy(&private_key, &program, credits, 10, None, rng).unwrap();

            // Construct the new block.
            sample_next_block(&vm, &private_key, &[transaction], &splits_block, &mut unspent_records, rng).unwrap()
        };

        // Add the deployment block to the VM.
        vm.add_next_block(&deployment_block).unwrap();

        // Generate more records to use for the next block.
        let splits_block = generate_splits(&vm, &private_key, &deployment_block, &mut unspent_records, rng).unwrap();

        // Add the splits block to the VM.
        vm.add_next_block(&splits_block).unwrap();

        // Create an execution transaction, that will be rejected.
        let r0 = Value::<CurrentNetwork>::from_str("100u8").unwrap();
        let first = create_execution(&vm, private_key, "testing.aleo", "compute", vec![r0], &mut unspent_records, rng);

        // Construct the next block.
        let next_block =
            sample_next_block(&vm, &private_key, &[first], &splits_block, &mut unspent_records, rng).unwrap();

        // Check that the transaction was rejected.
        assert!(next_block.transactions().iter().next().unwrap().is_rejected());

        // Add the next block to the VM.
        vm.add_next_block(&next_block).unwrap();

        // Check that the storage was not updated.
        let program_id = ProgramID::from_str("testing.aleo").unwrap();
        let mapping_name = Identifier::from_str("entries").unwrap();
        let value = vm
            .finalize_store()
            .get_value_speculative(&program_id, &mapping_name, &Plaintext::from(Literal::Address(address)))
            .unwrap();
        println!("{:?}", value);
        assert!(
            !vm.finalize_store()
                .contains_key_confirmed(&program_id, &mapping_name, &Plaintext::from(Literal::Address(address)))
                .unwrap()
        );

        // Create an execution transaction, that will be rejected.
        let r0 = Value::<CurrentNetwork>::from_str("100u8").unwrap();
        let first = create_execution(&vm, private_key, "testing.aleo", "compute", vec![r0], &mut unspent_records, rng);

        // Create an execution transaction, that will be accepted.
        let r0 = Value::<CurrentNetwork>::from_str("1u8").unwrap();
        let second = create_execution(&vm, private_key, "testing.aleo", "compute", vec![r0], &mut unspent_records, rng);

        // Construct the next block.
        let next_block =
            sample_next_block(&vm, &private_key, &[first, second], &next_block, &mut unspent_records, rng).unwrap();

        // Check that the first transaction was rejected.
        assert!(next_block.transactions().iter().next().unwrap().is_rejected());

        // Add the next block to the VM.
        vm.add_next_block(&next_block).unwrap();

        // Check that the storage was updated correctly.
        let value = vm
            .finalize_store()
            .get_value_speculative(&program_id, &mapping_name, &Plaintext::from(Literal::Address(address)))
            .unwrap()
            .unwrap();
        let expected = Value::<CurrentNetwork>::from_str("3u8").unwrap();
        assert_eq!(value, expected);
    }
}
