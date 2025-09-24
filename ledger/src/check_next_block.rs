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

use snarkvm_utilities::ensure_equals;

use crate::narwhal::BatchHeader;

use anyhow::{Context, bail};

/// Wrapper for a block that has a valid subDAG, but where the block header,
/// solutions, and transmissions have not been verified yet.
///
/// This type is created by `Ledger::check_block_subdag` and consumed by `Ledger::check_block_content`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PendingBlock<N: Network>(Block<N>);

impl<N: Network> Deref for PendingBlock<N> {
    type Target = Block<N>;

    fn deref(&self) -> &Block<N> {
        &self.0
    }
}

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Checks that the subDAG in a given block is valid, but does not fully verify the block.
    ///
    /// # Arguments
    /// * `block` - The block to check.
    /// * `pending_block` - A sequence of blocks between the block to check and the current height of the ledger.
    ///
    /// # Notes
    /// * This does *not* check that the header of the block is correct or execute/verify any of the transmissions contained within it.
    ///
    /// * In most cases, you want to use [`Self::check_next_block`] instead to perform a full verification.
    ///
    /// * This will reject any blocks with a height <= the current height and any blocks with a height >= the current height + GC.
    ///   For the former, a valid block already exists and,
    ///   for the latter, the comittte is still unknown.
    pub fn check_block_subdag(&self, block: Block<N>, pending_blocks: &[PendingBlock<N>]) -> Result<PendingBlock<N>> {
        self.check_block_subdag_inner(&block, pending_blocks)?;
        Ok(PendingBlock(block))
    }

    fn check_block_subdag_inner(&self, block: &Block<N>, pending_blocks: &[PendingBlock<N>]) -> Result<()> {
        // First check that the heights and hashes of the pending block sequence and of the new block are correct.
        // The hash checks should be redundant, but we perform them out of extra caution.
        let mut expected_height = self.latest_height() + 1;

        for block in pending_blocks.iter().map(|b| b.deref()).chain([block]) {
            ensure_equals!(block.height(), expected_height, "Block has invalid height");

            if self.contains_block_hash(&block.hash())? {
                bail!("Hash '{}' for block at height {} already exists in the ledger", block.height(), block.hash())
            }

            expected_height += 1;
        }

        // Ensure the certificates in the block subdag have met quorum requirements.
        self.check_block_subdag_quorum(block)?;

        // Determine if the block subdag is correctly constructed and is not a combination of multiple subdags.
        self.check_block_subdag_atomicity(block)?;

        // Ensure that all leaves of the subdag point to valid batches in other subdags/blocks.
        self.check_block_subdag_leaves(block, pending_blocks)?;

        Ok(())
    }

    /// Checks the given block is a valid next block with regard to the current state/height of the Ledger.
    pub fn check_next_block<R: CryptoRng + Rng>(&self, block: &Block<N>, rng: &mut R) -> Result<()> {
        self.check_block_subdag_inner(block, &[])?;
        self.check_block_content_inner(block, rng)?;

        Ok(())
    }

    /// Takes a pending block and performs the remaining checks to full verify it.
    ///
    /// # Arguments
    /// This takes a [`PendingBlock`] as input, which is the output of a successful call to [`Self::check_block_subdag`].
    /// The latter already verified the block's DAG and certificate signatures.
    ///
    /// # Return Value
    /// This returns a [`Block`] on success representing the fully verified block.
    pub fn check_block_content<R: CryptoRng + Rng>(&self, block: PendingBlock<N>, rng: &mut R) -> Result<Block<N>> {
        self.check_block_content_inner(&block.0, rng)?;
        Ok(block.0)
    }

    fn check_block_content_inner<R: CryptoRng + Rng>(&self, block: &Block<N>, rng: &mut R) -> Result<()> {
        let latest_block = self.latest_block();

        // Ensure solution IDs are unique
        self.check_block_solution_ids(block, &[])?;

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
    fn check_block_subdag_leaves(&self, block: &Block<N>, previous_blocks: &[PendingBlock<N>]) -> Result<()> {
        // Check if the block has a subdag.
        let Authority::Quorum(subdag) = block.authority() else {
            return Ok(());
        };

        let previous_certs: HashSet<_> = previous_blocks
            .iter()
            .filter_map(|block| match block.authority() {
                Authority::Quorum(subdag) => Some(subdag.certificate_ids()),
                Authority::Beacon(_) => None,
            })
            .flatten()
            .collect();

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
            if !previous_certs.contains(prev_id) && !self.vm.block_store().contains_block_for_certificate(prev_id)? {
                bail!(
                    "Batch(es) in the block point(s) to a certificate {prev_id} in round {prev_round} that is not associated with a previous block"
                )
            }

            Ok(())
        })
    }

    /// Ensure solutions IDs are unique and did not already appear in previous blocks.
    /// Called by [`Self::check_block_subdag_inner`]
    fn check_block_solution_ids(&self, block: &Block<N>, pending_blocks: &[PendingBlock<N>]) -> Result<()> {
        let mut pending_ids = HashSet::new();

        for block in pending_blocks.iter().map(|b| b.deref()).chain([block]) {
            for solution_id in block.solutions().solution_ids() {
                if !pending_ids.insert(solution_id) || self.contains_solution_id(solution_id)? {
                    bail!("Solution ID {solution_id} already exists in the ledger");
                }
            }
        }

        Ok(())
    }

    /// Check that the certificates in the block subdag have met quorum requirements.
    ///
    /// Called by [`Self::check_block_subdag_inner`]
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
    ///
    /// Called by [`Self::check_block_subdag_inner`]
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

#[cfg(test)]
mod tests {
    use crate::{
        check_next_block::PendingBlock,
        test_helpers::{sample_genesis_block, sample_test_env},
    };

    use console::prelude::*;

    #[test]
    fn test_check_block_subdag_inner_duplicate_hash() {
        let mut rng = TestRng::default();
        let test_env = sample_test_env(&mut rng);
        let ledger = test_env.ledger;

        // Get the existing genesis block
        let genesis_block = ledger.latest_block().clone();

        // Try to check a block with the same hash as genesis (already exists)
        let result = ledger.check_block_subdag_inner(&genesis_block, &[]);
        assert!(result.is_err(), "check_block_subdag_inner should fail for duplicate block hash");

        // The error could be either about duplicate hash or invalid height
        let error_message = result.unwrap_err().to_string();
        assert!(
            error_message.contains("already exists in the ledger")
                || error_message.contains("Block has invalid height"),
        );
    }

    #[test]
    fn test_check_block_subdag_with_empty_pending_blocks() {
        let mut rng = TestRng::default();
        let test_env = sample_test_env(&mut rng);
        let ledger = test_env.ledger;

        let genesis_block = ledger.latest_block().clone();

        // Test all individual functions with empty pending blocks.
        //TODO(kaimast): test with blocks other than the genesis.

        let result1 = ledger.check_block_solution_ids(&genesis_block, &[]);
        assert!(result1.is_ok(), "check_block_solution_ids should succeed with empty pending blocks");

        let result2 = ledger.check_block_subdag_quorum(&genesis_block);
        assert!(result2.is_ok(), "check_block_subdag_quorum should succeed");

        let result3 = ledger.check_block_subdag_atomicity(&genesis_block);
        assert!(result3.is_ok(), "check_block_subdag_atomicity should succeed");

        let result4 = ledger.check_block_subdag_leaves(&genesis_block, &[]);
        assert!(result4.is_ok(), "check_block_subdag_leaves should succeed with empty pending blocks");
    }

    #[test]
    fn test_pending_block_properties() {
        let mut rng = TestRng::default();

        //TODO(kaimast): test with blocks other than the genesis.
        let genesis_block = sample_genesis_block(&mut rng);
        let pending_block = PendingBlock(genesis_block.clone());

        // Test all properties accessible through Deref
        assert_eq!(pending_block.height(), genesis_block.height());
        assert_eq!(pending_block.hash(), genesis_block.hash());
        assert_eq!(pending_block.round(), genesis_block.round());
        assert_eq!(pending_block.timestamp(), genesis_block.timestamp());
        assert_eq!(pending_block.previous_hash(), genesis_block.previous_hash());

        // Test Clone and PartialEq
        let pending_block2 = pending_block.clone();
        assert_eq!(pending_block, pending_block2, "PendingBlock should be cloneable and equal");
    }
}
