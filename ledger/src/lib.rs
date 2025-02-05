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

#![forbid(unsafe_code)]
#![warn(clippy::cast_possible_truncation)]

#[macro_use]
extern crate tracing;

pub use ledger_authority as authority;
pub use ledger_block as block;
pub use ledger_committee as committee;
pub use ledger_narwhal as narwhal;
pub use ledger_puzzle as puzzle;
pub use ledger_query as query;
pub use ledger_store as store;

pub use crate::block::*;

#[cfg(feature = "test-helpers")]
pub use ledger_test_helpers;

mod helpers;
pub use helpers::*;

mod advance;
mod check_next_block;
mod check_transaction_basic;
mod contains;
mod find;
mod get;
mod iterators;

#[cfg(test)]
mod tests;

use console::{
    account::{Address, GraphKey, PrivateKey, ViewKey},
    network::prelude::*,
    program::{Ciphertext, Entry, Identifier, Literal, Plaintext, ProgramID, Record, StatePath, Value},
    types::{Field, Group},
};
use ledger_authority::Authority;
use ledger_committee::Committee;
use ledger_narwhal::{BatchCertificate, Subdag, Transmission, TransmissionID};
use ledger_puzzle::{Puzzle, PuzzleSolutions, Solution, SolutionID};
use ledger_query::Query;
use ledger_store::{ConsensusStorage, ConsensusStore};
use synthesizer::{
    program::{FinalizeGlobalState, Program},
    vm::VM,
};

use aleo_std::{
    StorageMode,
    prelude::{finish, lap, timer},
};
use anyhow::Result;
use core::ops::Range;
use indexmap::IndexMap;
use parking_lot::RwLock;
use rand::{prelude::IteratorRandom, rngs::OsRng};
use std::{borrow::Cow, sync::Arc};
use time::OffsetDateTime;

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

#[cfg(not(feature = "rocks"))]
use ledger_store::helpers::memory::ConsensusMemory;
#[cfg(feature = "rocks")]
use ledger_store::helpers::rocksdb::ConsensusDB;

pub type RecordMap<N> = IndexMap<Field<N>, Record<N, Plaintext<N>>>;

#[derive(Copy, Clone, Debug)]
pub enum RecordsFilter<N: Network> {
    /// Returns all records associated with the account.
    All,
    /// Returns only records associated with the account that are **spent** with the graph key.
    Spent,
    /// Returns only records associated with the account that are **not spent** with the graph key.
    Unspent,
    /// Returns all records associated with the account that are **spent** with the given private key.
    SlowSpent(PrivateKey<N>),
    /// Returns all records associated with the account that are **not spent** with the given private key.
    SlowUnspent(PrivateKey<N>),
}

#[derive(Clone)]
pub struct Ledger<N: Network, C: ConsensusStorage<N>> {
    /// The VM state.
    vm: VM<N, C>,
    /// The genesis block.
    genesis_block: Block<N>,
    /// The current epoch hash.
    current_epoch_hash: Arc<RwLock<Option<N::BlockHash>>>,
    /// The current committee.
    current_committee: Arc<RwLock<Option<Committee<N>>>>,
    /// The current block.
    current_block: Arc<RwLock<Block<N>>>,
}

macro_rules! impl_load {
    ($store_type:ty) => {
        impl<N: Network> Ledger<N, $store_type> {
            /// Loads the ledger from storage.
            pub fn load(genesis_block: Block<N>, storage_mode: StorageMode) -> Result<Self> {
                let timer = timer!("Ledger::load");

                // Retrieve the genesis hash.
                let genesis_hash = genesis_block.hash();
                // Initialize the ledger.
                let ledger = Self::load_unchecked(genesis_block, storage_mode)?;

                // Ensure the ledger contains the correct genesis block.
                if !ledger.contains_block_hash(&genesis_hash)? {
                    bail!("Incorrect genesis block (run 'snarkos clean' and try again)")
                }

                // Spot check the integrity of `NUM_BLOCKS` random blocks upon bootup.
                const NUM_BLOCKS: usize = 10;
                // Retrieve the latest height.
                let latest_height = ledger.current_block.read().height();
                debug_assert_eq!(
                    latest_height,
                    ledger.vm.block_store().max_height().unwrap(),
                    "Mismatch in latest height"
                );
                // Sample random block heights.
                let block_heights: Vec<u32> =
                    (0..=latest_height).choose_multiple(&mut OsRng, (latest_height as usize).min(NUM_BLOCKS));
                cfg_into_iter!(block_heights).try_for_each(|height| {
                    ledger.get_block(height)?;
                    Ok::<_, Error>(())
                })?;
                lap!(timer, "Check existence of {NUM_BLOCKS} random blocks");

                finish!(timer);
                Ok(ledger)
            }

            /// Loads the ledger from storage, without performing integrity checks.
            pub fn load_unchecked(genesis_block: Block<N>, storage_mode: StorageMode) -> Result<Self> {
                let timer = timer!("Ledger::load_unchecked");

                info!("Loading the ledger from storage...");
                // Initialize the consensus store.
                let store = match ConsensusStore::<N, _>::open(storage_mode) {
                    Ok(store) => store,
                    Err(e) => bail!("Failed to load ledger (run 'snarkos clean' and try again)\n\n{e}\n"),
                };
                lap!(timer, "Load consensus store");

                // Initialize a new VM.
                let vm = VM::from(store)?;
                lap!(timer, "Initialize a new VM");

                // Retrieve the current committee.
                let current_committee = vm.finalize_store().committee_store().current_committee().ok();

                // Initialize the ledger.
                let mut ledger = Self {
                    vm,
                    genesis_block: genesis_block.clone(),
                    current_epoch_hash: Default::default(),
                    current_committee: Arc::new(RwLock::new(current_committee)),
                    current_block: Arc::new(RwLock::new(genesis_block.clone())),
                };

                // If the block store is empty, initialize the genesis block.
                if ledger.vm.block_store().max_height().is_none() {
                    // Add the genesis block.
                    ledger.advance_to_next_block(&genesis_block)?;
                }
                lap!(timer, "Initialize genesis");

                // Retrieve the latest height.
                let latest_height = ledger
                    .vm
                    .block_store()
                    .max_height()
                    .ok_or_else(|| anyhow!("Failed to load blocks from the ledger"))?;
                // Fetch the latest block.
                let block = ledger
                    .get_block(latest_height)
                    .map_err(|_| anyhow!("Failed to load block {latest_height} from the ledger"))?;

                // Set the current block.
                ledger.current_block = Arc::new(RwLock::new(block));
                // Set the current committee (and ensures the latest committee exists).
                ledger.current_committee = Arc::new(RwLock::new(Some(ledger.latest_committee()?)));
                // Set the current epoch hash.
                ledger.current_epoch_hash = Arc::new(RwLock::new(Some(ledger.get_epoch_hash(latest_height)?)));

                finish!(timer, "Initialize ledger");
                Ok(ledger)
            }
        }
    };
}

#[cfg(feature = "rocks")]
impl_load!(ConsensusDB<N>);
#[cfg(not(feature = "rocks"))]
impl_load!(ConsensusMemory<N>);

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Returns the VM.
    pub const fn vm(&self) -> &VM<N, C> {
        &self.vm
    }

    /// Returns the puzzle.
    pub const fn puzzle(&self) -> &Puzzle<N> {
        self.vm.puzzle()
    }

    /// Returns the latest committee.
    pub fn latest_committee(&self) -> Result<Committee<N>> {
        match self.current_committee.read().as_ref() {
            Some(committee) => Ok(committee.clone()),
            None => self.vm.finalize_store().committee_store().current_committee(),
        }
    }

    /// Returns the latest state root.
    pub fn latest_state_root(&self) -> N::StateRoot {
        self.vm.block_store().current_state_root()
    }

    /// Returns the latest epoch number.
    pub fn latest_epoch_number(&self) -> u32 {
        self.current_block.read().height() / N::NUM_BLOCKS_PER_EPOCH
    }

    /// Returns the latest epoch hash.
    pub fn latest_epoch_hash(&self) -> Result<N::BlockHash> {
        match self.current_epoch_hash.read().as_ref() {
            Some(epoch_hash) => Ok(*epoch_hash),
            None => self.get_epoch_hash(self.latest_height()),
        }
    }

    /// Returns the latest block.
    pub fn latest_block(&self) -> Block<N> {
        self.current_block.read().clone()
    }

    /// Returns the latest round number.
    pub fn latest_round(&self) -> u64 {
        self.current_block.read().round()
    }

    /// Returns the latest block height.
    pub fn latest_height(&self) -> u32 {
        self.current_block.read().height()
    }

    /// Returns the latest block hash.
    pub fn latest_hash(&self) -> N::BlockHash {
        self.current_block.read().hash()
    }

    /// Returns the latest block header.
    pub fn latest_header(&self) -> Header<N> {
        *self.current_block.read().header()
    }

    /// Returns the latest block cumulative weight.
    pub fn latest_cumulative_weight(&self) -> u128 {
        self.current_block.read().cumulative_weight()
    }

    /// Returns the latest block cumulative proof target.
    pub fn latest_cumulative_proof_target(&self) -> u128 {
        self.current_block.read().cumulative_proof_target()
    }

    /// Returns the latest block solutions root.
    pub fn latest_solutions_root(&self) -> Field<N> {
        self.current_block.read().header().solutions_root()
    }

    /// Returns the latest block coinbase target.
    pub fn latest_coinbase_target(&self) -> u64 {
        self.current_block.read().coinbase_target()
    }

    /// Returns the latest block proof target.
    pub fn latest_proof_target(&self) -> u64 {
        self.current_block.read().proof_target()
    }

    /// Returns the last coinbase target.
    pub fn last_coinbase_target(&self) -> u64 {
        self.current_block.read().last_coinbase_target()
    }

    /// Returns the last coinbase timestamp.
    pub fn last_coinbase_timestamp(&self) -> i64 {
        self.current_block.read().last_coinbase_timestamp()
    }

    /// Returns the latest block timestamp.
    pub fn latest_timestamp(&self) -> i64 {
        self.current_block.read().timestamp()
    }

    /// Returns the latest block transactions.
    pub fn latest_transactions(&self) -> Transactions<N> {
        self.current_block.read().transactions().clone()
    }
}

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Returns the unspent `credits.aleo` records.
    pub fn find_unspent_credits_records(&self, view_key: &ViewKey<N>) -> Result<RecordMap<N>> {
        let microcredits = Identifier::from_str("microcredits")?;
        Ok(self
            .find_records(view_key, RecordsFilter::Unspent)?
            .filter(|(_, record)| {
                // TODO (raychu86): Find cleaner approach and check that the record is associated with the `credits.aleo` program
                match record.data().get(&microcredits) {
                    Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => !amount.is_zero(),
                    _ => false,
                }
            })
            .collect::<IndexMap<_, _>>())
    }

    /// Creates a deploy transaction.
    ///
    /// The `priority_fee_in_microcredits` is an additional fee **on top** of the deployment fee.
    pub fn create_deploy<R: Rng + CryptoRng>(
        &self,
        private_key: &PrivateKey<N>,
        program: &Program<N>,
        priority_fee_in_microcredits: u64,
        query: Option<Query<N, C::BlockStorage>>,
        rng: &mut R,
    ) -> Result<Transaction<N>> {
        // Fetch the unspent records.
        let records = self.find_unspent_credits_records(&ViewKey::try_from(private_key)?)?;
        ensure!(!records.len().is_zero(), "The Aleo account has no records to spend.");
        let mut records = records.values();

        // Prepare the fee record.
        let fee_record = Some(records.next().unwrap().clone());

        // Create a new deploy transaction.
        self.vm.deploy(private_key, program, fee_record, priority_fee_in_microcredits, query, rng)
    }

    /// Creates a transfer transaction.
    ///
    /// The `priority_fee_in_microcredits` is an additional fee **on top** of the execution fee.
    pub fn create_transfer<R: Rng + CryptoRng>(
        &self,
        private_key: &PrivateKey<N>,
        to: Address<N>,
        amount_in_microcredits: u64,
        priority_fee_in_microcredits: u64,
        query: Option<Query<N, C::BlockStorage>>,
        rng: &mut R,
    ) -> Result<Transaction<N>> {
        // Fetch the unspent records.
        let records = self.find_unspent_credits_records(&ViewKey::try_from(private_key)?)?;
        ensure!(!records.len().is_zero(), "The Aleo account has no records to spend.");
        let mut records = records.values();

        // Prepare the inputs.
        let inputs = [
            Value::Record(records.next().unwrap().clone()),
            Value::from_str(&format!("{to}"))?,
            Value::from_str(&format!("{amount_in_microcredits}u64"))?,
        ];

        // Prepare the fee.
        let fee_record = Some(records.next().unwrap().clone());

        // Create a new execute transaction.
        self.vm.execute(
            private_key,
            ("credits.aleo", "transfer_private"),
            inputs.iter(),
            fee_record,
            priority_fee_in_microcredits,
            query,
            rng,
        )
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use crate::Ledger;
    use aleo_std::StorageMode;
    use console::{
        account::{Address, PrivateKey, ViewKey},
        network::MainnetV0,
        prelude::*,
    };
    use ledger_block::Block;
    use ledger_store::ConsensusStore;
    use snarkvm_circuit::network::AleoV0;
    use synthesizer::vm::VM;

    pub(crate) type CurrentNetwork = MainnetV0;
    pub(crate) type CurrentAleo = AleoV0;

    #[cfg(not(feature = "rocks"))]
    pub(crate) type CurrentLedger =
        Ledger<CurrentNetwork, ledger_store::helpers::memory::ConsensusMemory<CurrentNetwork>>;
    #[cfg(feature = "rocks")]
    pub(crate) type CurrentLedger = Ledger<CurrentNetwork, ledger_store::helpers::rocksdb::ConsensusDB<CurrentNetwork>>;

    #[cfg(not(feature = "rocks"))]
    pub(crate) type CurrentConsensusStore =
        ConsensusStore<CurrentNetwork, ledger_store::helpers::memory::ConsensusMemory<CurrentNetwork>>;
    #[cfg(feature = "rocks")]
    pub(crate) type CurrentConsensusStore =
        ConsensusStore<CurrentNetwork, ledger_store::helpers::rocksdb::ConsensusDB<CurrentNetwork>>;

    #[allow(dead_code)]
    pub(crate) struct TestEnv {
        pub ledger: CurrentLedger,
        pub private_key: PrivateKey<CurrentNetwork>,
        pub view_key: ViewKey<CurrentNetwork>,
        pub address: Address<CurrentNetwork>,
    }

    pub(crate) fn sample_test_env(rng: &mut (impl Rng + CryptoRng)) -> TestEnv {
        // Sample the genesis private key.
        let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
        let view_key = ViewKey::try_from(&private_key).unwrap();
        let address = Address::try_from(&private_key).unwrap();
        // Sample the ledger.
        let ledger = sample_ledger(private_key, rng);
        // Return the test environment.
        TestEnv { ledger, private_key, view_key, address }
    }

    pub(crate) fn sample_genesis_block() -> Block<CurrentNetwork> {
        Block::<CurrentNetwork>::from_bytes_le(CurrentNetwork::genesis_bytes()).unwrap()
    }

    pub(crate) fn sample_ledger(
        private_key: PrivateKey<CurrentNetwork>,
        rng: &mut (impl Rng + CryptoRng),
    ) -> CurrentLedger {
        // Initialize the store.
        let store = CurrentConsensusStore::open(None).unwrap();
        // Create a genesis block.
        let genesis = VM::from(store).unwrap().genesis_beacon(&private_key, rng).unwrap();
        // Initialize the ledger with the genesis block.
        let ledger = CurrentLedger::load(genesis.clone(), StorageMode::Production).unwrap();
        // Ensure the genesis block is correct.
        assert_eq!(genesis, ledger.get_block(0).unwrap());
        // Return the ledger.
        ledger
    }
}
