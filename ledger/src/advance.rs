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

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Returns a candidate for the next block in the ledger, using a committed subdag and its transmissions.
    pub fn prepare_advance_to_next_quorum_block<R: Rng + CryptoRng>(
        &self,
        subdag: Subdag<N>,
        transmissions: IndexMap<TransmissionID<N>, Transmission<N>>,
        rng: &mut R,
    ) -> Result<Block<N>> {
        // Retrieve the latest block as the previous block (for the next block).
        let previous_block = self.latest_block();

        // Decouple the transmissions into ratifications, solutions, and transactions.
        let (ratifications, solutions, transactions) = decouple_transmissions(transmissions.into_iter())?;
        // Currently, we do not support ratifications from the memory pool.
        ensure!(ratifications.is_empty(), "Ratifications are currently unsupported from the memory pool");
        // Construct the block template.
        let (header, ratifications, solutions, aborted_solution_ids, transactions, aborted_transaction_ids) =
            self.construct_block_template(&previous_block, Some(&subdag), ratifications, solutions, transactions, rng)?;

        // Construct the new quorum block.
        Block::new_quorum(
            previous_block.hash(),
            header,
            subdag,
            ratifications,
            solutions,
            aborted_solution_ids,
            transactions,
            aborted_transaction_ids,
        )
    }

    /// Returns a candidate for the next block in the ledger.
    pub fn prepare_advance_to_next_beacon_block<R: Rng + CryptoRng>(
        &self,
        private_key: &PrivateKey<N>,
        candidate_ratifications: Vec<Ratify<N>>,
        candidate_solutions: Vec<Solution<N>>,
        candidate_transactions: Vec<Transaction<N>>,
        rng: &mut R,
    ) -> Result<Block<N>> {
        // Currently, we do not support ratifications from the memory pool.
        ensure!(candidate_ratifications.is_empty(), "Ratifications are currently unsupported from the memory pool");

        // Retrieve the latest block as the previous block (for the next block).
        let previous_block = self.latest_block();

        // Construct the block template.
        let (header, ratifications, solutions, aborted_solution_ids, transactions, aborted_transaction_ids) = self
            .construct_block_template(
                &previous_block,
                None,
                candidate_ratifications,
                candidate_solutions,
                candidate_transactions,
                rng,
            )?;

        // Construct the new beacon block.
        Block::new_beacon(
            private_key,
            previous_block.hash(),
            header,
            ratifications,
            solutions,
            aborted_solution_ids,
            transactions,
            aborted_transaction_ids,
            rng,
        )
    }

    /// Adds the given block as the next block in the ledger.
    pub fn advance_to_next_block(&self, block: &Block<N>) -> Result<()> {
        // Acquire the write lock on the current block.
        let mut current_block = self.current_block.write();
        // Update the VM.
        self.vm.add_next_block(block)?;
        // Update the current block.
        *current_block = block.clone();
        // Drop the write lock on the current block.
        drop(current_block);

        // Update the cached committee from storage.
        if let Ok(current_committee) = self.vm.finalize_store().committee_store().current_committee() {
            *self.current_committee.write() = Some(current_committee);
        }

        // If the block is the start of a new epoch, or the epoch hash has not been set, update the current epoch hash.
        if block.height() % N::NUM_BLOCKS_PER_EPOCH == 0 || self.current_epoch_hash.read().is_none() {
            // Update and log the current epoch hash.
            match self.get_epoch_hash(block.height()).ok() {
                Some(epoch_hash) => {
                    trace!("Updating the current epoch hash at block {} to '{epoch_hash}'", block.height());
                    *self.current_epoch_hash.write() = Some(epoch_hash);
                }
                None => {
                    error!("Failed to update the current epoch hash at block {}", block.height());
                }
            }
        }

        Ok(())
    }
}

/// Splits candidate solutions into a collection of accepted ones and aborted ones.
pub fn split_candidate_solutions<T, F>(
    mut candidate_solutions: Vec<T>,
    max_solutions: usize,
    verification_fn: F,
) -> (Vec<T>, Vec<T>)
where
    T: Sized + Copy,
    F: Fn(&mut T) -> bool,
{
    // Separate the candidate solutions into valid and aborted solutions.
    let mut valid_candidate_solutions = Vec::with_capacity(max_solutions);
    let mut aborted_candidate_solutions = Vec::new();
    // Reverse the candidate solutions in order to be able to chunk them more efficiently.
    candidate_solutions.reverse();
    // Verify the candidate solutions in chunks. This is done so that we can potentially
    // perform these operations in parallel while keeping the end result deterministic.
    let chunk_size = 16;
    while !candidate_solutions.is_empty() {
        // Check if the collection of valid solutions is full.
        if valid_candidate_solutions.len() >= max_solutions {
            // If that's the case, mark the rest of the candidates as aborted.
            aborted_candidate_solutions.extend(candidate_solutions.into_iter().rev());
            break;
        }

        // Split off a chunk of the candidate solutions.
        let mut candidates_chunk = if candidate_solutions.len() > chunk_size {
            candidate_solutions.split_off(candidate_solutions.len() - chunk_size)
        } else {
            std::mem::take(&mut candidate_solutions)
        };

        // Verify the solutions in the chunk.
        let verification_results = candidates_chunk.iter_mut().rev().map(|solution| {
            let verified = verification_fn(solution);
            (solution, verified)
        });

        // Process the results of the verification.
        for (solution, is_valid) in verification_results.into_iter() {
            if is_valid && valid_candidate_solutions.len() < max_solutions {
                valid_candidate_solutions.push(*solution);
            } else {
                aborted_candidate_solutions.push(*solution);
            }
        }
    }

    (valid_candidate_solutions, aborted_candidate_solutions)
}

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Constructs a block template for the next block in the ledger.
    #[allow(clippy::type_complexity)]
    fn construct_block_template<R: Rng + CryptoRng>(
        &self,
        previous_block: &Block<N>,
        subdag: Option<&Subdag<N>>,
        candidate_ratifications: Vec<Ratify<N>>,
        candidate_solutions: Vec<Solution<N>>,
        candidate_transactions: Vec<Transaction<N>>,
        rng: &mut R,
    ) -> Result<(Header<N>, Ratifications<N>, Solutions<N>, Vec<SolutionID<N>>, Transactions<N>, Vec<N::TransactionID>)>
    {
        // Construct the solutions.
        let (solutions, aborted_solutions, solutions_root, combined_proof_target) = match candidate_solutions.is_empty()
        {
            true => (None, vec![], Field::<N>::zero(), 0u128),
            false => {
                // Retrieve the latest epoch hash.
                let latest_epoch_hash = self.latest_epoch_hash()?;
                // Retrieve the latest proof target.
                let latest_proof_target = self.latest_proof_target();
                // Separate the candidate solutions into valid and aborted solutions.
                let (valid_candidate_solutions, aborted_candidate_solutions) =
                    split_candidate_solutions(candidate_solutions, N::MAX_SOLUTIONS, |solution| {
                        self.puzzle().check_solution_mut(solution, latest_epoch_hash, latest_proof_target).is_ok()
                    });

                // Check if there are any valid solutions.
                match valid_candidate_solutions.is_empty() {
                    true => (None, aborted_candidate_solutions, Field::<N>::zero(), 0u128),
                    false => {
                        // Construct the solutions.
                        let solutions = PuzzleSolutions::new(valid_candidate_solutions)?;
                        // Compute the solutions root.
                        let solutions_root = solutions.to_accumulator_point()?;
                        // Compute the combined proof target.
                        let combined_proof_target = self.puzzle().get_combined_proof_target(&solutions)?;
                        // Output the solutions, solutions root, and combined proof target.
                        (Some(solutions), aborted_candidate_solutions, solutions_root, combined_proof_target)
                    }
                }
            }
        };
        // Prepare the solutions.
        let solutions = Solutions::from(solutions);

        // Construct the aborted solution IDs.
        let aborted_solution_ids = aborted_solutions.iter().map(Solution::id).collect::<Vec<_>>();

        // Retrieve the latest state root.
        let latest_state_root = self.latest_state_root();
        // Retrieve the latest cumulative proof target.
        let latest_cumulative_proof_target = previous_block.cumulative_proof_target();
        // Retrieve the latest coinbase target.
        let latest_coinbase_target = previous_block.coinbase_target();
        // Retrieve the latest cumulative weight.
        let latest_cumulative_weight = previous_block.cumulative_weight();
        // Retrieve the last coinbase target.
        let last_coinbase_target = previous_block.last_coinbase_target();
        // Retrieve the last coinbase timestamp.
        let last_coinbase_timestamp = previous_block.last_coinbase_timestamp();

        // Compute the next round number.
        let next_round = match subdag {
            Some(subdag) => subdag.anchor_round(),
            None => previous_block.round().saturating_add(1),
        };
        // Compute the next height.
        let next_height = previous_block.height().saturating_add(1);
        // Determine the timestamp for the next block.
        let next_timestamp = match subdag {
            Some(subdag) => {
                // Retrieve the previous committee lookback.
                let previous_committee_lookback = {
                    // Calculate the penultimate round, which is the round before the anchor round.
                    let penultimate_round = subdag.anchor_round().saturating_sub(1);
                    // Get the round number for the previous committee. Note, we subtract 2 from odd rounds,
                    // because committees are updated in even rounds.
                    let previous_penultimate_round = match penultimate_round % 2 == 0 {
                        true => penultimate_round.saturating_sub(1),
                        false => penultimate_round.saturating_sub(2),
                    };
                    // Get the previous committee lookback round.
                    let penultimate_committee_lookback_round =
                        previous_penultimate_round.saturating_sub(Committee::<N>::COMMITTEE_LOOKBACK_RANGE);
                    // Output the previous committee lookback.
                    self.get_committee_for_round(penultimate_committee_lookback_round)?
                        .ok_or(anyhow!("Failed to fetch committee for round {penultimate_committee_lookback_round}"))?
                };
                // Return the timestamp for the given committee lookback.
                subdag.timestamp(&previous_committee_lookback)
            }
            None => OffsetDateTime::now_utc().unix_timestamp(),
        };

        // Calculate the next coinbase targets and timestamps.
        let (
            next_coinbase_target,
            next_proof_target,
            next_cumulative_proof_target,
            next_cumulative_weight,
            next_last_coinbase_target,
            next_last_coinbase_timestamp,
        ) = to_next_targets::<N>(
            latest_cumulative_proof_target,
            combined_proof_target,
            latest_coinbase_target,
            latest_cumulative_weight,
            last_coinbase_target,
            last_coinbase_timestamp,
            next_timestamp,
        )?;

        // Calculate the coinbase reward.
        let coinbase_reward = coinbase_reward(
            next_height,
            N::STARTING_SUPPLY,
            N::ANCHOR_HEIGHT,
            N::BLOCK_TIME,
            combined_proof_target,
            u64::try_from(latest_cumulative_proof_target)?,
            latest_coinbase_target,
        )?;

        // Construct the finalize state.
        let state = FinalizeGlobalState::new::<N>(
            next_round,
            next_height,
            next_cumulative_weight,
            next_cumulative_proof_target,
            previous_block.hash(),
        )?;
        // Speculate over the ratifications, solutions, and transactions.
        let (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations) = self.vm.speculate(
            state,
            Some(coinbase_reward),
            candidate_ratifications,
            &solutions,
            candidate_transactions.iter(),
            rng,
        )?;

        // Compute the ratifications root.
        let ratifications_root = ratifications.to_ratifications_root()?;

        // Construct the subdag root.
        let subdag_root = match subdag {
            Some(subdag) => subdag.to_subdag_root()?,
            None => Field::zero(),
        };

        // Construct the metadata.
        let metadata = Metadata::new(
            N::ID,
            next_round,
            next_height,
            next_cumulative_weight,
            next_cumulative_proof_target,
            next_coinbase_target,
            next_proof_target,
            next_last_coinbase_target,
            next_last_coinbase_timestamp,
            next_timestamp,
        )?;

        // Construct the header.
        let header = Header::from(
            latest_state_root,
            transactions.to_transactions_root()?,
            transactions.to_finalize_root(ratified_finalize_operations)?,
            ratifications_root,
            solutions_root,
            subdag_root,
            metadata,
        )?;

        // Return the block template.
        Ok((header, ratifications, solutions, aborted_solution_ids, transactions, aborted_transaction_ids))
    }
}
