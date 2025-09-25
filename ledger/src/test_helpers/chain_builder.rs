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

use crate::{
    Block,
    Ledger,
    Transaction,
    Transmission,
    TransmissionID,
    narwhal::{BatchCertificate, BatchHeader, Subdag},
    puzzle::Solution,
    store::ConsensusStore,
};
use snarkvm_console::{
    account::{Address, PrivateKey},
    network::MainnetV0,
    prelude::*,
};
use snarkvm_synthesizer::vm::VM;

use aleo_std::StorageMode;

use anyhow::{Context, Result};
use indexmap::{IndexMap, IndexSet};
use std::collections::{BTreeMap, HashMap};
use time::OffsetDateTime;

pub type CurrentNetwork = MainnetV0;

#[cfg(not(feature = "rocks"))]
pub type LedgerType<N> = snarkvm_ledger_store::helpers::memory::ConsensusMemory<N>;
#[cfg(feature = "rocks")]
pub type LedgerType<N> = snarkvm_ledger_store::helpers::rocksdb::ConsensusDB<N>;

/// Helper to build chains with custom structures for testing.
pub struct TestChainBuilder {
    /// The keys of all validators.
    private_keys: Vec<PrivateKey<CurrentNetwork>>,
    /// The underlying ledger.
    ledger: Ledger<CurrentNetwork, LedgerType<CurrentNetwork>>,
    /// The round containing the leader certificate for the most recent block we generated.
    last_block_round: u64,
    /// The batch certificates of the last round we generated.
    round_to_certificates: HashMap<u64, IndexMap<usize, BatchCertificate<CurrentNetwork>>>,
    /// The batch certificate of the last leader (if any).
    previous_leader_certificate: Option<BatchCertificate<CurrentNetwork>>,
    /// The last round for each committee member where they created a batch.
    /// Invariant: for any validator i, last_batch[i] <= last_committed_batch[i]
    last_batch_round: HashMap<usize, u64>,
    /// The last batch of a validator that was included in a block.
    last_committed_batch_round: HashMap<usize, u64>,
    /// The start of the test chain.
    genesis_block: Block<CurrentNetwork>,
}

/// Additional options you can pass to the builder when generating a set of blocks.
#[derive(Clone, Default)]
pub struct GenerateBlocksOptions {
    /// Do not include votes to the previous leader certificate
    pub skip_votes: bool,
    /// Do not generate certificates for the specific node indices (to simulate a partition).
    pub skip_nodes: Vec<usize>,
}

/// Additional options you can pass to the builder when generating a single block.
/// Note: As of now, all certificates for this block will have the given timestamp and contain listed transmissions.
#[derive(Clone)]
pub struct GenerateBlockOptions {
    /// Do not include votes to the previous leader certificate
    pub skip_votes: bool,
    /// Do not generate certificates for the specific node indices (to simulate a partition).
    pub skip_nodes: Vec<usize>,
    /// The timestamp for this block.
    pub timestamp: i64,
    /// The transmissions to be included in the block.
    pub solutions: Vec<Solution<CurrentNetwork>>,
    pub transactions: Vec<Transaction<CurrentNetwork>>,
}

impl Default for GenerateBlockOptions {
    fn default() -> Self {
        Self {
            skip_votes: false,
            skip_nodes: Default::default(),
            transactions: Default::default(),
            solutions: Default::default(),
            timestamp: OffsetDateTime::now_utc().unix_timestamp(),
        }
    }
}

impl TestChainBuilder {
    /// Generate a new committee and genesis block.
    pub fn initialize_components(
        committee_size: usize,
        rng: &mut TestRng,
    ) -> Result<(Vec<PrivateKey<CurrentNetwork>>, Block<CurrentNetwork>)> {
        // Sample the genesis private key.
        let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
        // Initialize the store.
        let store = ConsensusStore::<_, LedgerType<_>>::open(StorageMode::new_test(None))
            .with_context(|| "Failed to initialize consensus store")?;
        // Create a genesis block with a seeded RNG to reproduce the same genesis private keys.
        let seed: u64 = rng.r#gen();
        let genesis_rng = &mut TestRng::from_seed(seed);
        let genesis_block = VM::from(store).unwrap().genesis_beacon(&private_key, genesis_rng)?;

        // Extract the private keys from the genesis committee by using the same RNG to sample private keys.
        let genesis_rng = &mut TestRng::from_seed(seed);
        let private_keys = (0..committee_size).map(|_| PrivateKey::new(genesis_rng).unwrap()).collect();

        Ok((private_keys, genesis_block))
    }

    /// Initialize the builder with the default quorum size.
    pub fn new(rng: &mut TestRng) -> Result<Self> {
        Self::new_with_quorum_size(4, rng)
    }

    /// Initialize the builder with the specified quorum size.
    pub fn new_with_quorum_size(num_validators: usize, rng: &mut TestRng) -> Result<Self> {
        let (private_keys, genesis) = Self::initialize_components(num_validators, rng)?;
        Self::from_components(private_keys, genesis)
    }

    /// Initialize the builder with the specified committee and genesis block
    pub fn from_components(
        private_keys: Vec<PrivateKey<CurrentNetwork>>,
        genesis_block: Block<CurrentNetwork>,
    ) -> Result<Self> {
        // Initialize the ledger with the genesis block.
        let ledger = Ledger::<CurrentNetwork, LedgerType<CurrentNetwork>>::load(
            genesis_block.clone(),
            StorageMode::new_test(None),
        )
        .with_context(|| "Failed to set up ledger for test chain")?;

        ensure!(ledger.genesis_block == genesis_block);

        Self::from_genesis(private_keys, genesis_block)
    }

    /// Initialize the builder with the specified committee and gensis block
    pub fn from_genesis(
        private_keys: Vec<PrivateKey<CurrentNetwork>>,
        genesis_block: Block<CurrentNetwork>,
    ) -> Result<Self> {
        // Initialize the ledger with the genesis block.
        let ledger = Ledger::<CurrentNetwork, LedgerType<CurrentNetwork>>::load(
            genesis_block.clone(),
            StorageMode::new_test(None),
        )
        .with_context(|| "Failed to set up ledger for test chain")?;

        Ok(Self {
            private_keys,
            ledger,

            genesis_block,
            last_batch_round: Default::default(),
            last_committed_batch_round: Default::default(),
            last_block_round: 0,
            round_to_certificates: Default::default(),
            previous_leader_certificate: Default::default(),
        })
    }

    /// Create multiple blocks, with fully-connected DAGs.
    pub fn generate_blocks(&mut self, num_blocks: usize, rng: &mut TestRng) -> Result<Vec<Block<CurrentNetwork>>> {
        self.generate_blocks_with_opts(num_blocks, GenerateBlocksOptions::default(), rng)
    }

    /// Create multiple blocks, with additional parameters.
    pub fn generate_blocks_with_opts(
        &mut self,
        num_blocks: usize,
        options: GenerateBlocksOptions,
        rng: &mut TestRng,
    ) -> Result<Vec<Block<CurrentNetwork>>> {
        assert!(num_blocks > 0, "Need to build at least one block");

        let options = GenerateBlockOptions {
            skip_votes: options.skip_votes,
            skip_nodes: options.skip_nodes,
            ..Default::default()
        };

        let mut result = vec![];
        for _ in 0..num_blocks {
            let block = self.generate_block_with_opts(options.clone(), rng)?;
            result.push(block);
        }

        Ok(result)
    }

    /// Create a new block, with a fully-connected DAG.
    ///
    /// This will "fill in" any gaps left in earlier rounds from non participating nodes.
    pub fn generate_block(&mut self, rng: &mut TestRng) -> Result<Block<CurrentNetwork>> {
        self.generate_block_with_opts(GenerateBlockOptions::default(), rng)
    }

    /// Same as `generate_block` but with additional options/parameters.
    pub fn generate_block_with_opts(
        &mut self,
        options: GenerateBlockOptions,
        rng: &mut TestRng,
    ) -> Result<Block<CurrentNetwork>> {
        assert!(
            options.skip_nodes.len() * 3 < self.private_keys.len(),
            "Cannot mark more than f nodes as unavailable/skipped"
        );

        let next_block_round = self.last_block_round + 2;

        // SubDAGs can be at most GC rounds long.
        // Batches from genesis round cannot be included in any block that isn't genesis
        let mut round =
            next_block_round.checked_sub(BatchHeader::<CurrentNetwork>::MAX_GC_ROUNDS as u64).unwrap_or(1).max(1);

        let mut transmissions = IndexMap::default();

        for txn in options.transactions {
            let txn_id = txn.id();
            let transmission = Transmission::from(txn);
            let transmission_id = TransmissionID::Transaction(txn_id, transmission.to_checksum().unwrap().unwrap());

            transmissions.insert(transmission_id, transmission);
        }

        for solution in options.solutions {
            let transmission = Transmission::from(solution);
            let transmission_id = TransmissionID::Solution(solution.id(), transmission.to_checksum().unwrap().unwrap());

            transmissions.insert(transmission_id, transmission);
        }

        let transmission_ids: IndexSet<_> = transmissions.keys().copied().collect();

        // =======================================
        // Create certificates for the new block.
        // =======================================
        loop {
            let mut created_anchor = false;

            let previous_certificate_ids = if round == 1 {
                IndexSet::default()
            } else {
                self.round_to_certificates
                    .get(&(round - 1))
                    .unwrap()
                    .iter()
                    .filter_map(|(_, cert)| {
                        // If votes are skipped, remove previous leader cert from the set.
                        let skip = if let Some(leader) = &self.previous_leader_certificate {
                            options.skip_votes && leader.id() == cert.id()
                        } else {
                            false
                        };

                        if skip { None } else { Some(cert.id()) }
                    })
                    .collect()
            };

            let committee = self.ledger.get_committee_lookback_for_round(round).unwrap().unwrap_or_else(|| {
                panic!("No committee for round {round}");
            });

            for (key1_idx, private_key_1) in self.private_keys.iter().enumerate() {
                if options.skip_nodes.contains(&key1_idx) {
                    continue;
                }
                // Don't recreate batches that already exist.
                if self.last_batch_round.get(&key1_idx).unwrap_or(&0) >= &round {
                    continue;
                }

                let batch_header = BatchHeader::new(
                    private_key_1,
                    round,
                    options.timestamp,
                    committee.id(),
                    transmission_ids.clone(),
                    previous_certificate_ids.clone(),
                    rng,
                )
                .unwrap();

                // Add signatures for the batch header.
                let signatures = self
                    .private_keys
                    .iter()
                    .enumerate()
                    .filter(|&(key2_idx, _)| key1_idx != key2_idx)
                    .map(|(_, private_key_2)| private_key_2.sign(&[batch_header.batch_id()], rng).unwrap())
                    .collect();

                // Update the round at which this validator last created a batch.
                self.last_batch_round.insert(key1_idx, round);

                // Insert certificate into the round_to_certificates mapping.
                self.round_to_certificates
                    .entry(round)
                    .or_default()
                    .insert(key1_idx, BatchCertificate::from(batch_header, signatures).unwrap());

                // Check if this batch was an anchor.
                if round % 2 == 0 {
                    let leader = committee.get_leader(round).unwrap();
                    if leader == Address::try_from(private_key_1).unwrap() {
                        created_anchor = true;
                    }
                }
            }

            // Anchor was confirmed by more than a third of the validators.
            if created_anchor && round % 2 == 0 && self.last_block_round < round {
                self.last_block_round = round;
                break;
            }

            round += 1;
        }

        // ==============================================================
        // Build a subdag from the new certificates and create the block.
        // ==============================================================
        let commit_round = round;

        let leader_committee = self.ledger.get_committee_lookback_for_round(round).unwrap().unwrap();
        let leader = leader_committee.get_leader(commit_round).unwrap();
        let (leader_idx, leader_certificate) =
            self.round_to_certificates.get(&commit_round).unwrap().iter().find(|(_, c)| c.author() == leader).unwrap();
        let leader_idx = *leader_idx;
        let leader_certificate = leader_certificate.clone();

        // Construct the subdag for the new block.
        let mut subdag_map = BTreeMap::new();

        // Figure out what the earliest round for the subDAG could be.
        let start_round = if commit_round < BatchHeader::<CurrentNetwork>::MAX_GC_ROUNDS as u64 {
            1
        } else {
            commit_round - BatchHeader::<CurrentNetwork>::MAX_GC_ROUNDS as u64 + 2
        };

        for round in start_round..commit_round {
            let mut to_insert = IndexSet::new();
            for idx in 0..self.private_keys.len() {
                // Some of the batches we in previous rounds might not be new,
                // and already included in a previous block.
                let cround = self.last_committed_batch_round.entry(idx).or_default();
                // Batch already included in another block
                if *cround >= round {
                    continue;
                }

                if let Some(cert) = self.round_to_certificates.entry(round).or_default().get(&idx) {
                    to_insert.insert(cert.clone());
                    *cround = round;
                }
            }
            if !to_insert.is_empty() {
                subdag_map.insert(round, to_insert);
            }
        }

        // Add the leader certificate.
        // (special case, because it is the only cert included from the commit round)
        subdag_map.insert(commit_round, [leader_certificate.clone()].into());
        self.last_committed_batch_round.insert(leader_idx, commit_round);

        // Construct the block.
        let subdag = Subdag::from(subdag_map).unwrap();
        let block = self.ledger.prepare_advance_to_next_quorum_block(subdag, transmissions, rng)?;
        self.ledger.check_next_block(&block, rng).with_context(|| "Failed to (internally) check generated block")?;

        // Update the ledger state.
        self.ledger
            .advance_to_next_block(&block)
            .with_context(|| "Failed to (internally) advance to generated block")?;
        self.previous_leader_certificate = Some(leader_certificate.clone());

        Ok(block)
    }

    /// Return the genesis block associated with the test chain
    pub fn genesis_block(&self) -> &Block<CurrentNetwork> {
        &self.genesis_block
    }

    /// Returns the private keys of the genesis committee of this test chain
    pub fn private_keys(&self) -> &[PrivateKey<CurrentNetwork>] {
        &self.private_keys
    }

    /// Returns the private keys of the genesis committee of this test chain
    pub fn validator_key(&self, index: usize) -> &PrivateKey<CurrentNetwork> {
        &self.private_keys[index]
    }

    /// Returns the address of the specified validator.
    pub fn validator_address(&self, index: usize) -> Address<CurrentNetwork> {
        Address::try_from(*self.validator_key(index)).unwrap()
    }

    /// Create a test ledger with this builder's genesis block.
    pub fn instantiate_ledger(&self) -> Ledger<CurrentNetwork, LedgerType<CurrentNetwork>> {
        Ledger::load(self.genesis_block().clone(), StorageMode::new_test(None)).unwrap()
    }
}
