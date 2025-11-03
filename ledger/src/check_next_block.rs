// Copyright (c) 2019-2025 Provable Inc.
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

use crate::narwhal::BatchHeader;

use anyhow::{Context, bail};

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Checks the given block is valid next block.
    pub fn check_next_block<R: CryptoRng + Rng>(&self, block: &Block<N>, rng: &mut R) -> Result<()> {
        let height = block.height();
        let latest_block = self.latest_block();

        // Check that this is actually the next block.
        if height != latest_block.height() + 1 {
            bail!("Block height is {height}, but expected {}", latest_block.height() + 1);
        }

        // Ensure the block hash does not already exist.
        if self.contains_block_hash(&block.hash())? {
            bail!("Block hash '{}' already exists in the ledger", block.hash())
        }

        // Ensure the solutions do not already exist.
        for solution_id in block.solutions().solution_ids() {
            if self.contains_solution_id(solution_id)? {
                bail!("Solution ID {solution_id} already exists in the ledger");
            }
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
        let time_since_last_block = block.timestamp().saturating_sub(self.latest_timestamp());
        let ratified_finalize_operations = self
            .vm
            .check_speculate(
                state,
                time_since_last_block,
                block.ratifications(),
                block.solutions(),
                block.transactions(),
                rng,
            )
            .with_context(|| "Failed to speculate over unconfirmed transactions")?;

        // Retrieve the committee lookback.
        let committee_lookback = self
            .get_committee_lookback_for_round(block.round())?
            .ok_or(anyhow!("Failed to fetch committee lookback for round {}", block.round()))?;

        // Retrieve the previous committee lookback.
        let previous_committee_lookback = {
            // Calculate the penultimate round, which is the round before the anchor round.
            let penultimate_round = block.round().saturating_sub(1);
            // Output the committee lookback for the penultimate round.
            self.get_committee_lookback_for_round(penultimate_round)?
                .ok_or(anyhow!("Failed to fetch committee lookback for round {penultimate_round}"))?
        };

        // Ensure the block is correct.
        let (expected_existing_solution_ids, expected_existing_transaction_ids) = block
            .verify(
                &latest_block,
                self.latest_state_root(),
                &previous_committee_lookback,
                &committee_lookback,
                self.puzzle(),
                self.latest_epoch_hash()?,
                OffsetDateTime::now_utc().unix_timestamp(),
                ratified_finalize_operations,
            )
            .with_context(|| "Failed to verify block")?;

        // Ensure that the provers are within their stake bounds.
        if let Some(solutions) = block.solutions().deref() {
            let mut accepted_solutions: IndexMap<Address<N>, u64> = IndexMap::new();
            for solution in solutions.values() {
                let prover_address = solution.address();
                let num_accepted_solutions = *accepted_solutions.get(&prover_address).unwrap_or(&0);
                // Check if the prover has reached their solution limit.
                if self.is_solution_limit_reached(&prover_address, num_accepted_solutions) {
                    bail!("Prover '{prover_address}' has reached their solution limit for the current epoch");
                }
                // Track the already accepted solutions.
                *accepted_solutions.entry(prover_address).or_insert(0) += 1;
            }
        }

        // Ensure the certificates in the block subdag have met quorum requirements.
        self.check_block_subdag_quorum(block)?;

        // Determine if the block subdag is correctly constructed and is not a combination of multiple subdags.
        self.check_block_subdag_atomicity(block)?;

        // Ensure that all leaves of the subdag point to valid batches in other subdags/blocks.
        self.check_block_subdag_leaves(block)?;

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

    /// Check that leaves in the subdag point to batches in other blocks that are valid.
    ///
    /// This does not verify that the batches are signed correctly or that the edges are valid
    /// (only point to the previous round), as those checks already happened when the node received the batch.
    fn check_block_subdag_leaves(&self, block: &Block<N>) -> Result<()> {
        // Check if the block has a subdag.
        let Authority::Quorum(subdag) = block.authority() else {
            return Ok(());
        };

        // Store the IDs of all certificates in this subDAG.
        // This allows determining which edges point to other subDAGs/blocks.
        let subdag_certs: HashSet<_> = subdag.certificate_ids().collect();

        // Generate a set of all external certificates this subDAG references.
        // If multiple certificates reference the same external certificate, the id and round number will be
        // identical and the set will contain only one entry for the external certificate.
        let leaf_edges: HashSet<_> = subdag
            .certificates()
            .flat_map(|cert| cert.previous_certificate_ids().iter().map(|prev_id| (cert.round() - 1, prev_id)))
            .filter(|(_, prev_id)| !subdag_certs.contains(prev_id))
            .collect();

        cfg_iter!(leaf_edges).try_for_each(|(prev_round, prev_id)| {
            if prev_round + (BatchHeader::<N>::MAX_GC_ROUNDS as u64) - 1 <= block.round() {
                // If the previous round is at the end of GC, we cannot (and do not need to) verify the next batch.
                // For this leaf we are at the maximum length of the DAG, so any following batches are not allowed
                // to be part of the block and, thus, a malicious actor cannot remove them.
                return Ok::<(), Error>(());
            }

            // Ensure that the certificate is associated with a previous block.
            if !self.vm.block_store().contains_block_for_certificate(prev_id)? {
                bail!(
                    "Batch(es) in the block point(s) to a certificate {prev_id} in round {prev_round} that is not associated with a previous block"
                )
            }

            Ok(())
        })
    }

    /// Check that the certificates in the block subdag have met quorum requirements.
    fn check_block_subdag_quorum(&self, block: &Block<N>) -> Result<()> {
        // Check if the block has a subdag.
        let subdag = match block.authority() {
            Authority::Quorum(subdag) => subdag,
            _ => return Ok(()),
        };

        // Check that all certificates on each round have met quorum requirements.
        cfg_iter!(subdag).try_for_each(|(round, certificates)| {
            // Retrieve the committee lookback for the round.
            let committee_lookback = self
                .get_committee_lookback_for_round(*round)
                .with_context(|| format!("Failed to get committee lookback for round {round}"))?
                .ok_or_else(|| anyhow!("No committee lookback for round {round}"))?;

            // Check that each certificate for this round has met quorum requirements.
            // Note that we do not need to check the quorum requirement for the previous certificates
            // because that is done during construction in `BatchCertificate::new`.
            cfg_iter!(certificates).try_for_each(|certificate| {
                // Collect the certificate signers.
                let mut signers: HashSet<_> =
                    certificate.signatures().map(|signature| signature.to_address()).collect();
                // Append the certificate author.
                signers.insert(certificate.author());

                // Ensure that the signers of the certificate reach the quorum threshold.
                ensure!(
                    committee_lookback.is_quorum_threshold_reached(&signers),
                    "Certificate '{}' for round {round} does not meet quorum requirements",
                    certificate.id()
                );

                Ok::<_, Error>(())
            })?;

            Ok::<_, Error>(())
        })?;

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
                .with_context(|| format!("Failed to compute leader for round {round}"))?;

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
