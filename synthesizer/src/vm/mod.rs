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

mod helpers;
pub use helpers::*;

mod authorize;
mod deploy;
mod execute;
mod finalize;
mod verify;

#[cfg(test)]
mod tests;

use crate::{Restrictions, cast_mut_ref, cast_ref, convert, process};
use console::{
    account::{Address, PrivateKey},
    network::prelude::*,
    program::{Argument, Identifier, Literal, Locator, Plaintext, ProgramID, ProgramOwner, Record, Value},
    types::{Field, Group, U64},
};
use ledger_block::{
    Block,
    ConfirmedTransaction,
    Deployment,
    Execution,
    Fee,
    Header,
    Output,
    Ratifications,
    Ratify,
    Rejected,
    Solutions,
    Transaction,
    Transactions,
};
use ledger_committee::Committee;
use ledger_narwhal_data::Data;
use ledger_puzzle::Puzzle;
use ledger_query::{Query, QueryTrait};
use ledger_store::{
    BlockStore,
    ConsensusStorage,
    ConsensusStore,
    FinalizeMode,
    FinalizeStore,
    TransactionStorage,
    TransactionStore,
    TransitionStore,
    atomic_finalize,
};
use synthesizer_process::{Authorization, Process, Trace, deployment_cost, execution_cost_v1, execution_cost_v2};
use synthesizer_program::{FinalizeGlobalState, FinalizeOperation, FinalizeStoreTrait, Program};
use utilities::try_vm_runtime;

use aleo_std::prelude::{finish, lap, timer};
use indexmap::{IndexMap, IndexSet};
use itertools::Either;
use lru::LruCache;
use parking_lot::{Mutex, RwLock};
use rand::{SeedableRng, rngs::StdRng};
use std::{collections::HashSet, num::NonZeroUsize, sync::Arc};

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

#[derive(Clone)]
pub struct VM<N: Network, C: ConsensusStorage<N>> {
    /// The process.
    process: Arc<RwLock<Process<N>>>,
    /// The puzzle.
    puzzle: Puzzle<N>,
    /// The VM store.
    store: ConsensusStore<N, C>,
    /// A cache containing the list of recent partially-verified transactions.
    partially_verified_transactions: Arc<RwLock<LruCache<N::TransactionID, N::TransmissionChecksum>>>,
    /// The restrictions list.
    restrictions: Restrictions<N>,
    /// The lock to guarantee atomicity over calls to speculate and finalize.
    atomic_lock: Arc<Mutex<()>>,
    /// The lock for ensuring there is no concurrency when advancing blocks.
    block_lock: Arc<Mutex<()>>,
}

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// Initializes the VM from storage.
    #[inline]
    pub fn from(store: ConsensusStore<N, C>) -> Result<Self> {
        // Initialize a new process.
        println!("Initializing new process");
        let mut process = Process::load()?;
        println!("Finished initializing new process");

        // Initialize the store for 'credits.aleo'.
        let credits = Program::<N>::credits()?;
        for mapping in credits.mappings().values() {
            // Ensure that all mappings are initialized.
            if !store.finalize_store().contains_mapping_confirmed(credits.id(), mapping.name())? {
                // Initialize the mappings for 'credits.aleo'.
                store.finalize_store().initialize_mapping(*credits.id(), *mapping.name())?;
            }
        }

        // A helper function to retrieve all the deployments.
        fn load_deployment_and_imports<N: Network, T: TransactionStorage<N>>(
            process: &Process<N>,
            transaction_store: &TransactionStore<N, T>,
            transaction_id: N::TransactionID,
        ) -> Result<Vec<(ProgramID<N>, Deployment<N>)>> {
            // Retrieve the deployment from the transaction ID.
            let deployment = match transaction_store.get_deployment(&transaction_id)? {
                Some(deployment) => deployment,
                None => bail!("Deployment transaction '{transaction_id}' is not found in storage."),
            };

            // Fetch the program from the deployment.
            let program = deployment.program();
            let program_id = program.id();

            // Return early if the program is already loaded.
            if process.contains_program(program_id) {
                return Ok(vec![]);
            }

            // Prepare a vector for the deployments.
            let mut deployments = vec![];

            // Iterate through the program imports.
            for import_program_id in program.imports().keys() {
                // Add the imports to the process if does not exist yet.
                if !process.contains_program(import_program_id) {
                    // Fetch the deployment transaction ID.
                    let Some(transaction_id) =
                        transaction_store.deployment_store().find_transaction_id_from_program_id(import_program_id)?
                    else {
                        bail!("Transaction ID for '{program_id}' is not found in storage.");
                    };

                    // Add the deployment and its imports found recursively.
                    deployments.extend_from_slice(&load_deployment_and_imports(
                        process,
                        transaction_store,
                        transaction_id,
                    )?);
                }
            }

            // Once all the imports have been included, add the parent deployment.
            deployments.push((*program_id, deployment));

            Ok(deployments)
        }

        // Retrieve the transaction store.
        let transaction_store = store.transaction_store();
        // Retrieve the list of deployment transaction IDs.
        let deployment_ids = transaction_store.deployment_transaction_ids().collect::<Vec<_>>();
        // Load the deployments from the store.
        for (i, chunk) in deployment_ids.chunks(256).enumerate() {
            debug!(
                "Loading deployments {}-{} (of {})...",
                i * 256,
                ((i + 1) * 256).min(deployment_ids.len()),
                deployment_ids.len()
            );
            let deployments = cfg_iter!(chunk)
                .map(|transaction_id| {
                    // Load the deployment and its imports.
                    load_deployment_and_imports(&process, transaction_store, **transaction_id)
                })
                .collect::<Result<Vec<_>>>()?;

            for (program_id, deployment) in deployments.iter().flatten() {
                // Load the deployment if it does not exist in the process yet.
                println!("Loading: {program_id}");
                if !process.contains_program(program_id) {
                    process.load_deployment(deployment)?;
                }
            }
        }

        // Return the new VM.
        Ok(Self {
            process: Arc::new(RwLock::new(process)),
            puzzle: Self::new_puzzle()?,
            store,
            partially_verified_transactions: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(Transactions::<N>::MAX_TRANSACTIONS).unwrap(),
            ))),
            restrictions: Restrictions::load()?,
            atomic_lock: Arc::new(Mutex::new(())),
            block_lock: Arc::new(Mutex::new(())),
        })
    }

    /// Returns `true` if a program with the given program ID exists.
    #[inline]
    pub fn contains_program(&self, program_id: &ProgramID<N>) -> bool {
        self.process.read().contains_program(program_id)
    }

    /// Returns the process.
    #[inline]
    pub fn process(&self) -> Arc<RwLock<Process<N>>> {
        self.process.clone()
    }

    /// Returns the puzzle.
    #[inline]
    pub const fn puzzle(&self) -> &Puzzle<N> {
        &self.puzzle
    }

    /// Returns the partially-verified transactions.
    #[inline]
    pub fn partially_verified_transactions(&self) -> Arc<RwLock<LruCache<N::TransactionID, N::TransmissionChecksum>>> {
        self.partially_verified_transactions.clone()
    }

    /// Returns the restrictions.
    #[inline]
    pub const fn restrictions(&self) -> &Restrictions<N> {
        &self.restrictions
    }
}

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// Returns the finalize store.
    #[inline]
    pub fn finalize_store(&self) -> &FinalizeStore<N, C::FinalizeStorage> {
        self.store.finalize_store()
    }

    /// Returns the block store.
    #[inline]
    pub fn block_store(&self) -> &BlockStore<N, C::BlockStorage> {
        self.store.block_store()
    }

    /// Returns the transaction store.
    #[inline]
    pub fn transaction_store(&self) -> &TransactionStore<N, C::TransactionStorage> {
        self.store.transaction_store()
    }

    /// Returns the transition store.
    #[inline]
    pub fn transition_store(&self) -> &TransitionStore<N, C::TransitionStorage> {
        self.store.transition_store()
    }
}

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// Returns a new instance of the puzzle.
    pub fn new_puzzle() -> Result<Puzzle<N>> {
        // Initialize a new instance of the puzzle.
        macro_rules! logic {
            ($network:path, $aleo:path) => {{
                let puzzle = Puzzle::new::<ledger_puzzle_epoch::SynthesisPuzzle<$network, $aleo>>();
                Ok(cast_ref!(puzzle as Puzzle<N>).clone())
            }};
        }
        // Initialize the puzzle.
        convert!(logic)
    }
}

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// Returns a new genesis block for a beacon chain.
    pub fn genesis_beacon<R: Rng + CryptoRng>(&self, private_key: &PrivateKey<N>, rng: &mut R) -> Result<Block<N>> {
        let private_keys = [*private_key, PrivateKey::new(rng)?, PrivateKey::new(rng)?, PrivateKey::new(rng)?];

        // Construct the committee members.
        let members = indexmap::indexmap! {
            Address::try_from(private_keys[0])? => (ledger_committee::MIN_VALIDATOR_STAKE, true, 0u8),
            Address::try_from(private_keys[1])? => (ledger_committee::MIN_VALIDATOR_STAKE, true, 0u8),
            Address::try_from(private_keys[2])? => (ledger_committee::MIN_VALIDATOR_STAKE, true, 0u8),
            Address::try_from(private_keys[3])? => (ledger_committee::MIN_VALIDATOR_STAKE, true, 0u8),
        };
        // Construct the committee.
        let committee = Committee::<N>::new_genesis(members)?;

        // Compute the remaining supply.
        let remaining_supply = N::STARTING_SUPPLY - (ledger_committee::MIN_VALIDATOR_STAKE * 4);
        // Construct the public balances.
        let public_balances = indexmap::indexmap! {
            Address::try_from(private_keys[0])? => remaining_supply / 4,
            Address::try_from(private_keys[1])? => remaining_supply / 4,
            Address::try_from(private_keys[2])? => remaining_supply / 4,
            Address::try_from(private_keys[3])? => remaining_supply / 4,
        };
        // Construct the bonded balances.
        let bonded_balances = committee
            .members()
            .iter()
            .map(|(address, (amount, _, _))| (*address, (*address, *address, *amount)))
            .collect();
        // Return the genesis block.
        self.genesis_quorum(private_key, committee, public_balances, bonded_balances, rng)
    }

    /// Returns a new genesis block for a quorum chain.
    pub fn genesis_quorum<R: Rng + CryptoRng>(
        &self,
        private_key: &PrivateKey<N>,
        committee: Committee<N>,
        public_balances: IndexMap<Address<N>, u64>,
        bonded_balances: IndexMap<Address<N>, (Address<N>, Address<N>, u64)>,
        rng: &mut R,
    ) -> Result<Block<N>> {
        // Retrieve the total bonded balance.
        let total_bonded_amount = bonded_balances
            .values()
            .try_fold(0u64, |acc, (_, _, x)| acc.checked_add(*x).ok_or(anyhow!("Invalid bonded amount")))?;
        // Compute the account supply.
        let account_supply = public_balances
            .values()
            .try_fold(0u64, |acc, x| acc.checked_add(*x).ok_or(anyhow!("Invalid account supply")))?;
        // Compute the total supply.
        let total_supply =
            total_bonded_amount.checked_add(account_supply).ok_or_else(|| anyhow!("Invalid total supply"))?;
        // Ensure the total supply matches.
        ensure!(
            total_supply == N::STARTING_SUPPLY,
            "Invalid total supply. Found {total_supply}, expected {}",
            N::STARTING_SUPPLY
        );

        // Prepare the caller.
        let caller = Address::try_from(private_key)?;
        // Prepare the locator.
        let locator = ("credits.aleo", "transfer_public_to_private");
        // Prepare the amount for each call to the function.
        let amount = public_balances
            .get(&caller)
            .ok_or_else(|| anyhow!("Missing public balance for {caller}"))?
            .saturating_div(Block::<N>::NUM_GENESIS_TRANSACTIONS.saturating_mul(2) as u64);
        // Prepare the function inputs.
        let inputs = [caller.to_string(), format!("{amount}_u64")];

        // Prepare the ratifications.
        let ratifications =
            vec![Ratify::Genesis(Box::new(committee), Box::new(public_balances), Box::new(bonded_balances))];
        // Prepare the solutions.
        let solutions = Solutions::<N>::from(None); // The genesis block does not require solutions.
        // Prepare the aborted solution IDs.
        let aborted_solution_ids = vec![];
        // Prepare the transactions.
        let transactions = (0..Block::<N>::NUM_GENESIS_TRANSACTIONS)
            .map(|_| self.execute(private_key, locator, inputs.iter(), None, 0, None, rng))
            .collect::<Result<Vec<_>, _>>()?;

        // Construct the finalize state.
        let state = FinalizeGlobalState::new_genesis::<N>()?;
        // Speculate on the ratifications, solutions, and transactions.
        let (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations) =
            self.speculate(state, 0, None, ratifications, &solutions, transactions.iter(), rng)?;
        ensure!(
            aborted_transaction_ids.is_empty(),
            "Failed to initialize a genesis block - found aborted transaction IDs"
        );

        // Prepare the block header.
        let header = Header::genesis(&ratifications, &transactions, ratified_finalize_operations)?;
        // Prepare the previous block hash.
        let previous_hash = N::BlockHash::default();

        // Construct the block.
        let block = Block::new_beacon(
            private_key,
            previous_hash,
            header,
            ratifications,
            solutions,
            aborted_solution_ids,
            transactions,
            aborted_transaction_ids,
            rng,
        )?;
        // Ensure the block is valid genesis block.
        match block.is_genesis() {
            true => Ok(block),
            false => bail!("Failed to initialize a genesis block"),
        }
    }

    /// Adds the given block into the VM.
    #[inline]
    pub fn add_next_block(&self, block: &Block<N>) -> Result<()> {
        // Acquire the block lock, which is needed to ensure this function is not called concurrently.
        // Note: This lock must be held for the entire scope of this function.
        let _block_lock = self.block_lock.lock();

        // Construct the finalize state.
        let state = FinalizeGlobalState::new::<N>(
            block.round(),
            block.height(),
            block.cumulative_weight(),
            block.cumulative_proof_target(),
            block.previous_hash(),
        )?;

        // Pause the atomic writes, so that both the insertion and finalization belong to a single batch.
        #[cfg(feature = "rocks")]
        self.block_store().pause_atomic_writes()?;

        // First, insert the block.
        if let Err(insert_error) = self.block_store().insert(block) {
            if cfg!(feature = "rocks") {
                // Clear all pending atomic operations so that unpausing the atomic writes
                // doesn't execute any of the queued storage operations.
                self.block_store().abort_atomic();
                // Disable the atomic batch override.
                // Note: This call is guaranteed to succeed (without error), because `DISCARD_BATCH == true`.
                self.block_store().unpause_atomic_writes::<true>()?;
            }

            return Err(insert_error);
        };

        // Next, finalize the transactions.
        match self.finalize(state, block.ratifications(), block.solutions(), block.transactions()) {
            Ok(_ratified_finalize_operations) => {
                // Unpause the atomic writes, executing the ones queued from block insertion and finalization.
                #[cfg(feature = "rocks")]
                self.block_store().unpause_atomic_writes::<false>()?;
                Ok(())
            }
            Err(finalize_error) => {
                if cfg!(feature = "rocks") {
                    // Clear all pending atomic operations so that unpausing the atomic writes
                    // doesn't execute any of the queued storage operations.
                    self.block_store().abort_atomic();
                    self.finalize_store().abort_atomic();
                    // Disable the atomic batch override.
                    // Note: This call is guaranteed to succeed (without error), because `DISCARD_BATCH == true`.
                    self.block_store().unpause_atomic_writes::<true>()?;
                    // Rollback the Merkle tree.
                    self.block_store().remove_last_n_from_tree_only(1).inspect_err(|_| {
                        // Log the finalize error.
                        error!("Failed to finalize block {} - {finalize_error}", block.height());
                    })?;
                } else {
                    // Rollback the block.
                    self.block_store().remove_last_n(1).inspect_err(|_| {
                        // Log the finalize error.
                        error!("Failed to finalize block {} - {finalize_error}", block.height());
                    })?;
                }
                // Return the finalize error.
                Err(finalize_error)
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use console::{
        account::{Address, ViewKey},
        network::MainnetV0,
        program::Value,
        types::Field,
    };
    use ledger_block::{Block, Header, Metadata, Transition};
    use ledger_store::helpers::memory::ConsensusMemory;
    #[cfg(feature = "rocks")]
    use ledger_store::helpers::rocksdb::ConsensusDB;
    use synthesizer_program::Program;

    use indexmap::IndexMap;
    use once_cell::sync::OnceCell;
    #[cfg(feature = "rocks")]
    use std::path::Path;

    pub(crate) type CurrentNetwork = MainnetV0;

    /// Samples a new finalize state.
    pub(crate) fn sample_finalize_state(block_height: u32) -> FinalizeGlobalState {
        FinalizeGlobalState::from(block_height as u64, block_height, [0u8; 32])
    }

    pub(crate) fn sample_vm() -> VM<CurrentNetwork, ConsensusMemory<CurrentNetwork>> {
        // Initialize a new VM.
        VM::from(ConsensusStore::open(None).unwrap()).unwrap()
    }

    #[cfg(feature = "rocks")]
    pub(crate) fn sample_vm_rocks(path: &Path) -> VM<CurrentNetwork, ConsensusDB<CurrentNetwork>> {
        // Initialize a new VM.
        VM::from(ConsensusStore::open(path.to_owned()).unwrap()).unwrap()
    }

    pub(crate) fn sample_genesis_private_key(rng: &mut TestRng) -> PrivateKey<CurrentNetwork> {
        static INSTANCE: OnceCell<PrivateKey<CurrentNetwork>> = OnceCell::new();
        *INSTANCE.get_or_init(|| {
            // Initialize a new caller.
            PrivateKey::<CurrentNetwork>::new(rng).unwrap()
        })
    }

    pub(crate) fn sample_genesis_block(rng: &mut TestRng) -> Block<CurrentNetwork> {
        static INSTANCE: OnceCell<Block<CurrentNetwork>> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                // Initialize the VM.
                let vm = crate::vm::test_helpers::sample_vm();
                // Initialize a new caller.
                let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
                // Return the block.
                vm.genesis_beacon(&caller_private_key, rng).unwrap()
            })
            .clone()
    }

    pub(crate) fn sample_vm_with_genesis_block(
        rng: &mut TestRng,
    ) -> VM<CurrentNetwork, ConsensusMemory<CurrentNetwork>> {
        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();
        // Return the VM.
        vm
    }

    pub(crate) fn sample_program() -> Program<CurrentNetwork> {
        static INSTANCE: OnceCell<Program<CurrentNetwork>> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                // Initialize a new program.
                Program::<CurrentNetwork>::from_str(
                    r"
program testing.aleo;

struct message:
    amount as u128;

mapping account:
    key as address.public;
    value as u64.public;

record token:
    owner as address.private;
    amount as u64.private;

function initialize:
    input r0 as address.private;
    input r1 as u64.private;
    cast r0 r1 into r2 as token.record;
    output r2 as token.record;

function compute:
    input r0 as message.private;
    input r1 as message.public;
    input r2 as message.private;
    input r3 as token.record;
    add r0.amount r1.amount into r4;
    cast r3.owner r3.amount into r5 as token.record;
    output r4 as u128.public;
    output r5 as token.record;",
                )
                .unwrap()
            })
            .clone()
    }

    pub(crate) fn sample_deployment_transaction(rng: &mut TestRng) -> Transaction<CurrentNetwork> {
        static INSTANCE: OnceCell<Transaction<CurrentNetwork>> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                // Initialize the program.
                let program = sample_program();

                // Initialize a new caller.
                let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
                let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();

                // Initialize the genesis block.
                let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

                // Fetch the unspent records.
                let records =
                    genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
                trace!("Unspent Records:\n{:#?}", records);

                // Prepare the fee.
                let credits = Some(records.values().next().unwrap().decrypt(&caller_view_key).unwrap());

                // Initialize the VM.
                let vm = sample_vm();
                // Update the VM.
                vm.add_next_block(&genesis).unwrap();

                // Deploy.
                let transaction = vm.deploy(&caller_private_key, &program, credits, 10, None, rng).unwrap();
                // Verify.
                vm.check_transaction(&transaction, None, rng).unwrap();
                // Return the transaction.
                transaction
            })
            .clone()
    }

    pub(crate) fn sample_execution_transaction_without_fee(rng: &mut TestRng) -> Transaction<CurrentNetwork> {
        static INSTANCE: OnceCell<Transaction<CurrentNetwork>> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                // Initialize a new caller.
                let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
                let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();

                // Initialize the genesis block.
                let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

                // Fetch the unspent records.
                let records =
                    genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
                trace!("Unspent Records:\n{:#?}", records);

                // Select a record to spend.
                let record = records.values().next().unwrap().decrypt(&caller_view_key).unwrap();

                // Initialize the VM.
                let vm = sample_vm();
                // Update the VM.
                vm.add_next_block(&genesis).unwrap();

                // Prepare the inputs.
                let inputs =
                    [Value::<CurrentNetwork>::Record(record), Value::<CurrentNetwork>::from_str("1u64").unwrap()]
                        .into_iter();

                // Authorize.
                let authorization = vm.authorize(&caller_private_key, "credits.aleo", "split", inputs, rng).unwrap();
                assert_eq!(authorization.len(), 1);

                // Construct the execute transaction.
                let transaction = vm.execute_authorization(authorization, None, None, rng).unwrap();
                // Verify.
                vm.check_transaction(&transaction, None, rng).unwrap();
                // Return the transaction.
                transaction
            })
            .clone()
    }

    pub(crate) fn sample_execution_transaction_with_private_fee(rng: &mut TestRng) -> Transaction<CurrentNetwork> {
        static INSTANCE: OnceCell<Transaction<CurrentNetwork>> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                // Initialize a new caller.
                let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
                let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();
                let address = Address::try_from(&caller_private_key).unwrap();

                // Initialize the genesis block.
                let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

                // Fetch the unspent records.
                let records =
                    genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
                trace!("Unspent Records:\n{:#?}", records);

                // Select a record to spend.
                let record = Some(records.values().next().unwrap().decrypt(&caller_view_key).unwrap());

                // Initialize the VM.
                let vm = sample_vm();
                // Update the VM.
                vm.add_next_block(&genesis).unwrap();

                // Prepare the inputs.
                let inputs = [
                    Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
                    Value::<CurrentNetwork>::from_str("1u64").unwrap(),
                ]
                .into_iter();

                // Execute.
                let transaction = vm
                    .execute(&caller_private_key, ("credits.aleo", "transfer_public"), inputs, record, 0, None, rng)
                    .unwrap();
                // Verify.
                vm.check_transaction(&transaction, None, rng).unwrap();
                // Return the transaction.
                transaction
            })
            .clone()
    }

    pub(crate) fn sample_execution_transaction_with_public_fee(rng: &mut TestRng) -> Transaction<CurrentNetwork> {
        static INSTANCE: OnceCell<Transaction<CurrentNetwork>> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                // Initialize a new caller.
                let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
                let address = Address::try_from(&caller_private_key).unwrap();

                // Initialize the genesis block.
                let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

                // Initialize the VM.
                let vm = sample_vm();
                // Update the VM.
                vm.add_next_block(&genesis).unwrap();

                // Prepare the inputs.
                let inputs = [
                    Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
                    Value::<CurrentNetwork>::from_str("1u64").unwrap(),
                ]
                .into_iter();

                // Execute.
                let transaction_without_fee = vm
                    .execute(&caller_private_key, ("credits.aleo", "transfer_public"), inputs, None, 0, None, rng)
                    .unwrap();
                let execution = transaction_without_fee.execution().unwrap().clone();

                // Authorize the fee.
                let authorization = vm
                    .authorize_fee_public(
                        &caller_private_key,
                        10_000_000,
                        100,
                        execution.to_execution_id().unwrap(),
                        rng,
                    )
                    .unwrap();
                // Compute the fee.
                let fee = vm.execute_fee_authorization(authorization, None, rng).unwrap();

                // Construct the transaction.
                let transaction = Transaction::from_execution(execution, Some(fee)).unwrap();
                // Verify.
                vm.check_transaction(&transaction, None, rng).unwrap();
                // Return the transaction.
                transaction
            })
            .clone()
    }

    #[cfg(feature = "test")]
    pub(crate) fn create_new_transaction_with_different_fee(
        rng: &mut TestRng,
        transaction: Transaction<CurrentNetwork>,
        fee: u64,
    ) -> Transaction<CurrentNetwork> {
        // Initialize a new caller.
        let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

        // Initialize the VM.
        let vm = sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Get Execution
        let execution = transaction.execution().unwrap().clone();

        // Authorize the fee.
        let authorization =
            vm.authorize_fee_public(&caller_private_key, fee, 100, execution.to_execution_id().unwrap(), rng).unwrap();
        // Compute the fee.
        let fee = vm.execute_fee_authorization(authorization, None, rng).unwrap();

        // Construct the transaction.
        Transaction::from_execution(execution, Some(fee)).unwrap()
    }

    pub fn sample_next_block<R: Rng + CryptoRng>(
        vm: &VM<MainnetV0, ConsensusMemory<MainnetV0>>,
        private_key: &PrivateKey<MainnetV0>,
        transactions: &[Transaction<MainnetV0>],
        rng: &mut R,
    ) -> Result<Block<MainnetV0>> {
        // Get the most recent block.
        let block_hash = vm.block_store().get_block_hash(vm.block_store().max_height().unwrap()).unwrap().unwrap();
        let previous_block = vm.block_store().get_block(&block_hash).unwrap().unwrap();

        // Construct the new block header.
        let time_since_last_block = MainnetV0::BLOCK_TIME as i64;
        let (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations) = vm.speculate(
            sample_finalize_state(1),
            time_since_last_block,
            None,
            vec![],
            &None.into(),
            transactions.iter(),
            rng,
        )?;

        // Construct the metadata associated with the block.
        let metadata = Metadata::new(
            MainnetV0::ID,
            previous_block.round() + 1,
            previous_block.height() + 1,
            0,
            0,
            MainnetV0::GENESIS_COINBASE_TARGET,
            MainnetV0::GENESIS_PROOF_TARGET,
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
}
