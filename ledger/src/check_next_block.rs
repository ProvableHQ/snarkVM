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

use super::*;

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Checks the given block is valid next block.
    pub fn check_next_block<R: CryptoRng + Rng>(&self, block: &Block<N>, rng: &mut R) -> Result<()> {
        let height = block.height();

        // Ensure the block hash does not already exist.
        if self.contains_block_hash(&block.hash())? {
            bail!("Block hash '{}' already exists in the ledger", block.hash())
        }

        // Ensure the block height does not already exist.
        if self.contains_block_height(block.height())? {
            bail!("Block height '{height}' already exists in the ledger")
        }

        // Ensure the solutions do not already exist.
        for solution_id in block.solutions().solution_ids() {
            if self.contains_solution_id(solution_id)? {
                bail!("Solution ID {solution_id} already exists in the ledger");
            }
        }

        // TODO (howardwu): Remove this after moving the total supply into credits.aleo.
        {
            // // Retrieve the latest total supply.
            // let latest_total_supply = self.latest_total_supply_in_microcredits();
            // // Retrieve the block reward from the first block ratification.
            // let block_reward = match block.ratifications()[0] {
            //     Ratify::BlockReward(block_reward) => block_reward,
            //     _ => bail!("Block {height} is invalid - the first ratification must be a block reward"),
            // };
            // // Retrieve the puzzle reward from the second block ratification.
            // let puzzle_reward = match block.ratifications()[1] {
            //     Ratify::PuzzleReward(puzzle_reward) => puzzle_reward,
            //     _ => bail!("Block {height} is invalid - the second ratification must be a puzzle reward"),
            // };
            // // Compute the next total supply in microcredits.
            // let next_total_supply_in_microcredits =
            //     update_total_supply(latest_total_supply, block_reward, puzzle_reward, block.transactions())?;
            // // Ensure the total supply in microcredits is correct.
            // if next_total_supply_in_microcredits != block.total_supply_in_microcredits() {
            //     bail!("Invalid total supply in microcredits")
            // }
        }

        // Construct the finalize state.
        let state = FinalizeGlobalState::new::<N>(
            block.round(),
            block.height(),
            block.cumulative_weight(),
            block.cumulative_proof_target(),
            block.previous_hash(),
        )?;

        // Ensure speculation over the unconfirmed transactions is correct and ensure each transaction is well-formed and unique.
        let ratified_finalize_operations =
            self.vm.check_speculate(state, block.ratifications(), block.solutions(), block.transactions(), rng)?;

        // Retrieve the committee lookback.
        let committee_lookback = {
            // Determine the round number for the previous committee. Note, we subtract 2 from odd rounds,
            // because committees are updated in even rounds.
            let previous_round = match block.round() % 2 == 0 {
                true => block.round().saturating_sub(1),
                false => block.round().saturating_sub(2),
            };
            // Determine the committee lookback round.
            let committee_lookback_round = previous_round.saturating_sub(Committee::<N>::COMMITTEE_LOOKBACK_RANGE);
            // Output the committee lookback.
            self.get_committee_for_round(committee_lookback_round)?
                .ok_or(anyhow!("Failed to fetch committee for round {committee_lookback_round}"))?
        };

        // Retrieve the previous committee lookback.
        let previous_committee_lookback = {
            // Calculate the penultimate round, which is the round before the anchor round.
            let penultimate_round = block.round().saturating_sub(1);
            // Determine the round number for the previous committee. Note, we subtract 2 from odd rounds,
            // because committees are updated in even rounds.
            let previous_penultimate_round = match penultimate_round % 2 == 0 {
                true => penultimate_round.saturating_sub(1),
                false => penultimate_round.saturating_sub(2),
            };
            // Determine the previous committee lookback round.
            let penultimate_committee_lookback_round =
                previous_penultimate_round.saturating_sub(Committee::<N>::COMMITTEE_LOOKBACK_RANGE);
            // Output the previous committee lookback.
            self.get_committee_for_round(penultimate_committee_lookback_round)?
                .ok_or(anyhow!("Failed to fetch committee for round {penultimate_committee_lookback_round}"))?
        };

        // Ensure the block is correct.
        let (expected_existing_solution_ids, expected_existing_transaction_ids) = block.verify(
            &self.latest_block(),
            self.latest_state_root(),
            &previous_committee_lookback,
            &committee_lookback,
            self.puzzle(),
            self.latest_epoch_hash()?,
            OffsetDateTime::now_utc().unix_timestamp(),
            ratified_finalize_operations,
        )?;

        // Determine if the block subdag is correctly constructed and is not a combination of multiple subdags.
        self.check_block_subdag_atomicity(block)?;

        // Ensure that each existing solution ID from the block exists in the ledger.
        for existing_solution_id in expected_existing_solution_ids {
            if !self.contains_solution_id(&existing_solution_id)? {
                bail!("Solution ID '{existing_solution_id}' does not exist in the ledger");
            }
        }

        // Ensure that each existing transaction ID from the block exists in the ledger.
        for existing_transaction_id in expected_existing_transaction_ids {
            if !self.contains_transaction_id(&existing_transaction_id)? {
                bail!("Transaction ID '{existing_transaction_id}' does not exist in the ledger");
            }
        }

        Ok(())
    }

    /// Checks that the block subdag can not be split into multiple valid subdags.
    fn check_block_subdag_atomicity(&self, block: &Block<N>) -> Result<()> {
        // Returns `true` if there is a path from the previous certificate to the current certificate.
        fn is_linked<N: Network>(
            subdag: &Subdag<N>,
            previous_certificate: &BatchCertificate<N>,
            current_certificate: &BatchCertificate<N>,
        ) -> Result<bool> {
            // Initialize the list containing the traversal.
            let mut traversal = vec![current_certificate];
            // Iterate over the rounds from the current certificate to the previous certificate.
            for round in (previous_certificate.round()..current_certificate.round()).rev() {
                // Retrieve all of the certificates for this past round.
                let certificates = subdag.get(&round).ok_or(anyhow!("No certificates found for round {round}"))?;
                // Filter the certificates to only include those that are in the traversal.
                traversal = certificates
                    .into_iter()
                    .filter(|p| traversal.iter().any(|c| c.previous_certificate_ids().contains(&p.id())))
                    .collect();
            }
            Ok(traversal.contains(&previous_certificate))
        }

        // Check if the block has a subdag.
        let subdag = match block.authority() {
            Authority::Quorum(subdag) => subdag,
            _ => return Ok(()),
        };

        // Iterate over the rounds to find possible leader certificates.
        for round in (self.latest_round().saturating_add(2)..=subdag.anchor_round().saturating_sub(2)).rev().step_by(2)
        {
            // Retrieve the previous committee lookback.
            let previous_committee_lookback = self
                .get_committee_lookback_for_round(round)?
                .ok_or_else(|| anyhow!("No committee lookback found for round {round}"))?;

            // Compute the leader for the commit round.
            let computed_leader = previous_committee_lookback
                .get_leader(round)
                .map_err(|e| anyhow!("Failed to compute leader for round {round}: {e}"))?;

            // Retrieve the previous leader certificates.
            let previous_certificate = match subdag.get(&round).and_then(|certificates| {
                certificates.iter().find(|certificate| certificate.author() == computed_leader)
            }) {
                Some(cert) => cert,
                None => continue,
            };

            // Determine if there is a path between the previous certificate and the subdag's leader certificate.
            if is_linked(subdag, previous_certificate, subdag.leader_certificate())? {
                bail!(
                    "The previous certificate should not be linked to the current certificate in block {}",
                    block.height()
                );
            }
        }

        Ok(())
    }
}
