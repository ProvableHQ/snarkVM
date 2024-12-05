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
        let mut process = Process::load()?;

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
        program::{Entry, Value},
        types::Field,
    };
    use ledger_block::{Block, Header, Metadata, Transition};
    use ledger_store::helpers::memory::ConsensusMemory;
    #[cfg(feature = "rocks")]
    use ledger_store::helpers::rocksdb::ConsensusDB;
    use ledger_test_helpers::{large_transaction_program, small_transaction_program};
    use synthesizer_program::Program;

    use indexmap::IndexMap;
    use once_cell::sync::OnceCell;
    #[cfg(feature = "rocks")]
    use std::path::Path;
    use synthesizer_snark::VerifyingKey;

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

    #[test]
    fn test_multiple_deployments_and_multiple_executions() {
        let rng = &mut TestRng::default();

        // Initialize a new caller.
        let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
        let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();

        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

        // Fetch the unspent records.
        let records = genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
        trace!("Unspent Records:\n{:#?}", records);

        // Select a record to spend.
        let record = records.values().next().unwrap().decrypt(&caller_view_key).unwrap();

        // Initialize the VM.
        let vm = sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Split once.
        let transaction = vm
            .execute(
                &caller_private_key,
                ("credits.aleo", "split"),
                [Value::Record(record), Value::from_str("1000000000u64").unwrap()].iter(), // 1000 credits
                None,
                0,
                None,
                rng,
            )
            .unwrap();
        let records = transaction.records().collect_vec();
        let first_record = records[0].1.clone().decrypt(&caller_view_key).unwrap();
        let second_record = records[1].1.clone().decrypt(&caller_view_key).unwrap();
        let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng).unwrap();
        vm.add_next_block(&block).unwrap();

        // Split again.
        let mut transactions = Vec::new();
        let transaction = vm
            .execute(
                &caller_private_key,
                ("credits.aleo", "split"),
                [Value::Record(first_record), Value::from_str("100000000u64").unwrap()].iter(), // 100 credits
                None,
                0,
                None,
                rng,
            )
            .unwrap();
        let records = transaction.records().collect_vec();
        let first_record = records[0].1.clone().decrypt(&caller_view_key).unwrap();
        let third_record = records[1].1.clone().decrypt(&caller_view_key).unwrap();
        transactions.push(transaction);
        // Split again.
        let transaction = vm
            .execute(
                &caller_private_key,
                ("credits.aleo", "split"),
                [Value::Record(second_record), Value::from_str("100000000u64").unwrap()].iter(), // 100 credits
                None,
                0,
                None,
                rng,
            )
            .unwrap();
        let records = transaction.records().collect_vec();
        let second_record = records[0].1.clone().decrypt(&caller_view_key).unwrap();
        let fourth_record = records[1].1.clone().decrypt(&caller_view_key).unwrap();
        transactions.push(transaction);
        // Add the split transactions to a block and update the VM.
        let fee_block = sample_next_block(&vm, &caller_private_key, &transactions, rng).unwrap();
        vm.add_next_block(&fee_block).unwrap();

        // Deploy the programs.
        let first_program = r"
program test_program_1.aleo;
mapping map_0:
    key as field.public;
    value as field.public;
function init:
    async init into r0;
    output r0 as test_program_1.aleo/init.future;
finalize init:
    set 0field into map_0[0field];
function getter:
    async getter into r0;
    output r0 as test_program_1.aleo/getter.future;
finalize getter:
    get map_0[0field] into r0;
        ";
        let second_program = r"
program test_program_2.aleo;
mapping map_0:
    key as field.public;
    value as field.public;
function init:
    async init into r0;
    output r0 as test_program_2.aleo/init.future;
finalize init:
    set 0field into map_0[0field];
function getter:
    async getter into r0;
    output r0 as test_program_2.aleo/getter.future;
finalize getter:
    get map_0[0field] into r0;
        ";
        let first_deployment = vm
            .deploy(&caller_private_key, &Program::from_str(first_program).unwrap(), Some(first_record), 1, None, rng)
            .unwrap();
        let second_deployment = vm
            .deploy(&caller_private_key, &Program::from_str(second_program).unwrap(), Some(second_record), 1, None, rng)
            .unwrap();
        let deployment_block =
            sample_next_block(&vm, &caller_private_key, &[first_deployment, second_deployment], rng).unwrap();
        vm.add_next_block(&deployment_block).unwrap();

        // Execute the programs.
        let first_execution = vm
            .execute(
                &caller_private_key,
                ("test_program_1.aleo", "init"),
                Vec::<Value<MainnetV0>>::new().iter(),
                Some(third_record),
                1,
                None,
                rng,
            )
            .unwrap();
        let second_execution = vm
            .execute(
                &caller_private_key,
                ("test_program_2.aleo", "init"),
                Vec::<Value<MainnetV0>>::new().iter(),
                Some(fourth_record),
                1,
                None,
                rng,
            )
            .unwrap();
        let execution_block =
            sample_next_block(&vm, &caller_private_key, &[first_execution, second_execution], rng).unwrap();
        vm.add_next_block(&execution_block).unwrap();
    }

    #[test]
    fn test_load_deployments_with_imports() {
        // NOTE: This seed was chosen for the CI's RNG to ensure that the test passes.
        let rng = &mut TestRng::fixed(123456789);

        // Initialize a new caller.
        let caller_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
        let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = vm.genesis_beacon(&caller_private_key, rng).unwrap();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Fetch the unspent records.
        let records = genesis.transitions().cloned().flat_map(Transition::into_records).collect::<Vec<(_, _)>>();
        trace!("Unspent Records:\n{:#?}", records);
        let record_0 = records[0].1.decrypt(&caller_view_key).unwrap();
        let record_1 = records[1].1.decrypt(&caller_view_key).unwrap();
        let record_2 = records[2].1.decrypt(&caller_view_key).unwrap();
        let record_3 = records[3].1.decrypt(&caller_view_key).unwrap();

        // Create the deployment for the first program.
        let program_1 = r"
program first_program.aleo;

function c:
    input r0 as u8.private;
    input r1 as u8.private;
    add r0 r1 into r2;
    output r2 as u8.private;
        ";
        let deployment_1 = vm
            .deploy(&caller_private_key, &Program::from_str(program_1).unwrap(), Some(record_0), 0, None, rng)
            .unwrap();

        // Deploy the first program.
        let deployment_block = sample_next_block(&vm, &caller_private_key, &[deployment_1.clone()], rng).unwrap();
        vm.add_next_block(&deployment_block).unwrap();

        // Create the deployment for the second program.
        let program_2 = r"
import first_program.aleo;

program second_program.aleo;

function b:
    input r0 as u8.private;
    input r1 as u8.private;
    call first_program.aleo/c r0 r1 into r2;
    output r2 as u8.private;
        ";
        let deployment_2 = vm
            .deploy(&caller_private_key, &Program::from_str(program_2).unwrap(), Some(record_1), 0, None, rng)
            .unwrap();

        // Deploy the second program.
        let deployment_block = sample_next_block(&vm, &caller_private_key, &[deployment_2.clone()], rng).unwrap();
        vm.add_next_block(&deployment_block).unwrap();

        // Create the deployment for the third program.
        let program_3 = r"
import second_program.aleo;

program third_program.aleo;

function a:
    input r0 as u8.private;
    input r1 as u8.private;
    call second_program.aleo/b r0 r1 into r2;
    output r2 as u8.private;
        ";
        let deployment_3 = vm
            .deploy(&caller_private_key, &Program::from_str(program_3).unwrap(), Some(record_2), 0, None, rng)
            .unwrap();

        // Create the deployment for the fourth program.
        let program_4 = r"
import second_program.aleo;
import first_program.aleo;

program fourth_program.aleo;

function a:
    input r0 as u8.private;
    input r1 as u8.private;
    call second_program.aleo/b r0 r1 into r2;
    output r2 as u8.private;
        ";
        let deployment_4 = vm
            .deploy(&caller_private_key, &Program::from_str(program_4).unwrap(), Some(record_3), 0, None, rng)
            .unwrap();

        // Deploy the third and fourth program together.
        let deployment_block =
            sample_next_block(&vm, &caller_private_key, &[deployment_3.clone(), deployment_4.clone()], rng).unwrap();
        vm.add_next_block(&deployment_block).unwrap();

        // Sanity check the ordering of the deployment transaction IDs from storage.
        {
            let deployment_transaction_ids =
                vm.transaction_store().deployment_transaction_ids().map(|id| *id).collect::<Vec<_>>();
            // This assert check is here to ensure that we are properly loading imports even though any order will work for `VM::from`.
            // Note: `deployment_transaction_ids` is sorted lexicographically by transaction ID, so the order may change if we update internal methods.
            assert_eq!(
                deployment_transaction_ids,
                vec![deployment_4.id(), deployment_3.id(), deployment_2.id(), deployment_1.id()],
                "Update me if serialization has changed"
            );
        }

        // Enforce that the VM can load properly with the imports.
        assert!(VM::from(vm.store.clone()).is_ok());
    }

    #[test]
    fn test_multiple_external_calls() {
        let rng = &mut TestRng::default();

        // Initialize a new caller.
        let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
        let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();
        let address = Address::try_from(&caller_private_key).unwrap();

        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

        // Fetch the unspent records.
        let records =
            genesis.transitions().cloned().flat_map(Transition::into_records).take(3).collect::<IndexMap<_, _>>();
        trace!("Unspent Records:\n{:#?}", records);
        let record_0 = records.values().next().unwrap().decrypt(&caller_view_key).unwrap();
        let record_1 = records.values().nth(1).unwrap().decrypt(&caller_view_key).unwrap();
        let record_2 = records.values().nth(2).unwrap().decrypt(&caller_view_key).unwrap();

        // Initialize the VM.
        let vm = sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Deploy the program.
        let program = Program::from_str(
            r"
import credits.aleo;

program test_multiple_external_calls.aleo;

function multitransfer:
    input r0 as credits.aleo/credits.record;
    input r1 as address.private;
    input r2 as u64.private;
    call credits.aleo/transfer_private r0 r1 r2 into r3 r4;
    call credits.aleo/transfer_private r4 r1 r2 into r5 r6;
    output r4 as credits.aleo/credits.record;
    output r5 as credits.aleo/credits.record;
    output r6 as credits.aleo/credits.record;
    ",
        )
        .unwrap();
        let deployment = vm.deploy(&caller_private_key, &program, Some(record_0), 1, None, rng).unwrap();
        vm.add_next_block(&sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap()).unwrap();

        // Execute the programs.
        let inputs = [
            Value::<MainnetV0>::Record(record_1),
            Value::<MainnetV0>::from_str(&address.to_string()).unwrap(),
            Value::<MainnetV0>::from_str("10u64").unwrap(),
        ];
        let execution = vm
            .execute(
                &caller_private_key,
                ("test_multiple_external_calls.aleo", "multitransfer"),
                inputs.into_iter(),
                Some(record_2),
                1,
                None,
                rng,
            )
            .unwrap();
        vm.add_next_block(&sample_next_block(&vm, &caller_private_key, &[execution], rng).unwrap()).unwrap();
    }

    #[test]
    fn test_nested_deployment_with_assert() {
        let rng = &mut TestRng::default();

        // Initialize a private key.
        let private_key = sample_genesis_private_key(rng);

        // Initialize the genesis block.
        let genesis = sample_genesis_block(rng);

        // Initialize the VM.
        let vm = sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Deploy the base program.
        let program = Program::from_str(
            r"
program child_program.aleo;

function check:
    input r0 as field.private;
    assert.eq r0 123456789123456789123456789123456789123456789123456789field;
        ",
        )
        .unwrap();

        let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
        assert!(vm.check_transaction(&deployment, None, rng).is_ok());
        vm.add_next_block(&sample_next_block(&vm, &private_key, &[deployment], rng).unwrap()).unwrap();

        // Check that program is deployed.
        assert!(vm.contains_program(&ProgramID::from_str("child_program.aleo").unwrap()));

        // Deploy the program that calls the program from the previous layer.
        let program = Program::from_str(
            r"
import child_program.aleo;

program parent_program.aleo;

function check:
    input r0 as field.private;
    call child_program.aleo/check r0;
        ",
        )
        .unwrap();

        let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
        assert!(vm.check_transaction(&deployment, None, rng).is_ok());
        vm.add_next_block(&sample_next_block(&vm, &private_key, &[deployment], rng).unwrap()).unwrap();

        // Check that program is deployed.
        assert!(vm.contains_program(&ProgramID::from_str("parent_program.aleo").unwrap()));
    }

    #[test]
    fn test_deployment_with_external_records() {
        let rng = &mut TestRng::default();

        // Initialize a private key.
        let private_key = sample_genesis_private_key(rng);

        // Initialize the genesis block.
        let genesis = sample_genesis_block(rng);

        // Initialize the VM.
        let vm = sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Deploy the program.
        let program = Program::from_str(
            r"
import credits.aleo;
program test_program.aleo;

function transfer:
    input r0 as credits.aleo/credits.record;
    input r1 as u64.private;
    input r2 as u64.private;
    input r3 as [address; 10u32].private;
    call credits.aleo/transfer_private r0 r3[0u32] r1 into r4 r5;
    call credits.aleo/transfer_private r5 r3[0u32] r2 into r6 r7;
",
        )
        .unwrap();

        let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
        assert!(vm.check_transaction(&deployment, None, rng).is_ok());
        vm.add_next_block(&sample_next_block(&vm, &private_key, &[deployment], rng).unwrap()).unwrap();

        // Check that program is deployed.
        assert!(vm.contains_program(&ProgramID::from_str("test_program.aleo").unwrap()));
    }

    #[test]
    fn test_internal_fee_calls_are_invalid() {
        let rng = &mut TestRng::default();

        // Initialize a private key.
        let private_key = sample_genesis_private_key(rng);
        let view_key = ViewKey::try_from(&private_key).unwrap();

        // Initialize the genesis block.
        let genesis = sample_genesis_block(rng);

        // Initialize the VM.
        let vm = sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Fetch the unspent records.
        let records =
            genesis.transitions().cloned().flat_map(Transition::into_records).take(3).collect::<IndexMap<_, _>>();
        trace!("Unspent Records:\n{:#?}", records);
        let record_0 = records.values().next().unwrap().decrypt(&view_key).unwrap();

        // Deploy the program.
        let program = Program::from_str(
            r"
import credits.aleo;
program test_program.aleo;

function call_fee_public:
    input r0 as u64.private;
    input r1 as u64.private;
    input r2 as field.private;
    call credits.aleo/fee_public r0 r1 r2 into r3;
    async call_fee_public r3 into r4;
    output r4 as test_program.aleo/call_fee_public.future;

finalize call_fee_public:
    input r0 as credits.aleo/fee_public.future;
    await r0;
    
function call_fee_private:
    input r0 as credits.aleo/credits.record;
    input r1 as u64.private;
    input r2 as u64.private;
    input r3 as field.private;
    call credits.aleo/fee_private r0 r1 r2 r3 into r4;
    output r4 as credits.aleo/credits.record;
",
        )
        .unwrap();

        let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
        assert!(vm.check_transaction(&deployment, None, rng).is_ok());
        vm.add_next_block(&sample_next_block(&vm, &private_key, &[deployment], rng).unwrap()).unwrap();

        // Execute the programs.
        let internal_base_fee_amount: u64 = rng.gen_range(1..1000);
        let internal_priority_fee_amount: u64 = rng.gen_range(1..1000);

        // Ensure that the transaction that calls `fee_public` internally cannot be generated.
        let inputs = [
            Value::<MainnetV0>::from_str(&format!("{}u64", internal_base_fee_amount)).unwrap(),
            Value::<MainnetV0>::from_str(&format!("{}u64", internal_priority_fee_amount)).unwrap(),
            Value::<MainnetV0>::from_str("1field").unwrap(),
        ];
        assert!(
            vm.execute(&private_key, ("test_program.aleo", "call_fee_public"), inputs.into_iter(), None, 0, None, rng)
                .is_err()
        );

        // Ensure that the transaction that calls `fee_private` internally cannot be generated.
        let inputs = [
            Value::<MainnetV0>::Record(record_0),
            Value::<MainnetV0>::from_str(&format!("{}u64", internal_base_fee_amount)).unwrap(),
            Value::<MainnetV0>::from_str(&format!("{}u64", internal_priority_fee_amount)).unwrap(),
            Value::<MainnetV0>::from_str("1field").unwrap(),
        ];
        assert!(
            vm.execute(&private_key, ("test_program.aleo", "call_fee_private"), inputs.into_iter(), None, 0, None, rng)
                .is_err()
        );
    }

    #[test]
    fn test_deployment_synthesis_overload() {
        let rng = &mut TestRng::default();

        // Initialize a private key.
        let private_key = sample_genesis_private_key(rng);

        // Initialize the genesis block.
        let genesis = sample_genesis_block(rng);

        // Initialize the VM.
        let vm = sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Deploy the base program.
        let program = Program::from_str(
            r"
program synthesis_overload.aleo;

function do:
    input r0 as [[u128; 32u32]; 2u32].private;
    hash.sha3_256 r0 into r1 as field;
    output r1 as field.public;",
        )
        .unwrap();

        // Create the deployment transaction.
        let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

        // Verify the deployment transaction. It should fail because there are too many constraints.
        assert!(vm.check_transaction(&deployment, None, rng).is_err());
    }

    #[test]
    fn test_deployment_num_constant_overload() {
        let rng = &mut TestRng::default();

        // Initialize a private key.
        let private_key = sample_genesis_private_key(rng);

        // Initialize the genesis block.
        let genesis = sample_genesis_block(rng);

        // Initialize the VM.
        let vm = sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Deploy the base program.
        let program = Program::from_str(
            r"
program synthesis_num_constants.aleo;
function do:
    cast 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 into r0 as [u32; 32u32];
    cast r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 into r1 as [[u32; 32u32]; 32u32];
    cast r1 r1 r1 r1 r1 into r2 as [[[u32; 32u32]; 32u32]; 5u32];
    hash.bhp1024 r2 into r3 as u32;
    output r3 as u32.private;
function do2:
    cast 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 0u32 into r0 as [u32; 32u32];
    cast r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 r0 into r1 as [[u32; 32u32]; 32u32];
    cast r1 r1 r1 r1 r1 into r2 as [[[u32; 32u32]; 32u32]; 5u32];
    hash.bhp1024 r2 into r3 as u32;
    output r3 as u32.private;",
        )
            .unwrap();

        // Create the deployment transaction.
        let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

        // Verify the deployment transaction. It should fail because there are too many constants.
        let check_tx_res = vm.check_transaction(&deployment, None, rng);
        assert!(check_tx_res.is_err());
    }

    #[test]
    fn test_deployment_synthesis_overreport() {
        let rng = &mut TestRng::default();

        // Initialize a private key.
        let private_key = sample_genesis_private_key(rng);

        // Initialize the genesis block.
        let genesis = sample_genesis_block(rng);

        // Initialize the VM.
        let vm = sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Deploy the base program.
        let program = Program::from_str(
            r"
program synthesis_overreport.aleo;

function do:
    input r0 as u32.private;
    add r0 r0 into r1;
    output r1 as u32.public;",
        )
        .unwrap();

        // Create the deployment transaction.
        let transaction = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

        // Destructure the deployment transaction.
        let Transaction::Deploy(_, program_owner, deployment, fee) = transaction else {
            panic!("Expected a deployment transaction");
        };

        // Increase the number of constraints in the verifying keys.
        let mut vks_with_overreport = Vec::with_capacity(deployment.verifying_keys().len());
        for (id, (vk, cert)) in deployment.verifying_keys() {
            let mut vk_deref = vk.deref().clone();
            vk_deref.circuit_info.num_constraints += 1;
            let vk = VerifyingKey::new(Arc::new(vk_deref), vk.num_variables());
            vks_with_overreport.push((*id, (vk, cert.clone())));
        }

        // Each additional constraint costs 25 microcredits, so we need to increase the fee by 25 microcredits.
        let required_fee = *fee.base_amount().unwrap() + 25;
        // Authorize a new fee.
        let fee_authorization = vm
            .authorize_fee_public(&private_key, required_fee, 0, deployment.as_ref().to_deployment_id().unwrap(), rng)
            .unwrap();
        // Compute the fee.
        let fee = vm.execute_fee_authorization(fee_authorization, None, rng).unwrap();

        // Create a new deployment transaction with the overreported verifying keys.
        let adjusted_deployment =
            Deployment::new(deployment.edition(), deployment.program().clone(), vks_with_overreport).unwrap();
        let adjusted_transaction = Transaction::from_deployment(program_owner, adjusted_deployment, fee).unwrap();

        // Verify the deployment transaction. It should error when certificate checking for constraint count mismatch.
        let res = vm.check_transaction(&adjusted_transaction, None, rng);
        assert!(res.is_err());
    }

    #[test]
    fn test_deployment_synthesis_underreport() {
        let rng = &mut TestRng::default();

        // Initialize a private key.
        let private_key = sample_genesis_private_key(rng);
        let address = Address::try_from(&private_key).unwrap();

        // Initialize the genesis block.
        let genesis = sample_genesis_block(rng);

        // Initialize the VM.
        let vm = sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Deploy the base program.
        let program = Program::from_str(
            r"
program synthesis_underreport.aleo;

function do:
    input r0 as u32.private;
    add r0 r0 into r1;
    output r1 as u32.public;",
        )
        .unwrap();

        // Create the deployment transaction.
        let transaction = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

        // Destructure the deployment transaction.
        let Transaction::Deploy(txid, program_owner, deployment, fee) = transaction else {
            panic!("Expected a deployment transaction");
        };

        // Decrease the number of constraints in the verifying keys.
        let mut vks_with_underreport = Vec::with_capacity(deployment.verifying_keys().len());
        for (id, (vk, cert)) in deployment.verifying_keys() {
            let mut vk_deref = vk.deref().clone();
            vk_deref.circuit_info.num_constraints -= 2;
            let vk = VerifyingKey::new(Arc::new(vk_deref), vk.num_variables());
            vks_with_underreport.push((*id, (vk, cert.clone())));
        }

        // Create a new deployment transaction with the underreported verifying keys.
        let adjusted_deployment =
            Deployment::new(deployment.edition(), deployment.program().clone(), vks_with_underreport).unwrap();
        let adjusted_transaction = Transaction::Deploy(txid, program_owner, Box::new(adjusted_deployment), fee);

        // Verify the deployment transaction. It should error when enforcing the first constraint over the vk limit.
        let result = vm.check_transaction(&adjusted_transaction, None, rng);
        assert!(result.is_err());

        // Create a standard transaction
        // Prepare the inputs.
        let inputs = [
            Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
            Value::<CurrentNetwork>::from_str("1u64").unwrap(),
        ]
        .into_iter();

        // Execute.
        let transaction =
            vm.execute(&private_key, ("credits.aleo", "transfer_public"), inputs, None, 0, None, rng).unwrap();

        // Check that the deployment transaction will be aborted if injected into a block.
        let block = sample_next_block(&vm, &private_key, &[transaction, adjusted_transaction.clone()], rng).unwrap();

        // Check that the block aborts the deployment transaction.
        assert_eq!(block.aborted_transaction_ids(), &vec![adjusted_transaction.id()]);

        // Update the VM.
        vm.add_next_block(&block).unwrap();
    }

    #[test]
    fn test_deployment_variable_underreport() {
        let rng = &mut TestRng::default();

        // Initialize a private key.
        let private_key = sample_genesis_private_key(rng);
        let address = Address::try_from(&private_key).unwrap();

        // Initialize the genesis block.
        let genesis = sample_genesis_block(rng);

        // Initialize the VM.
        let vm = sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Deploy the base program.
        let program = Program::from_str(
            r"
program synthesis_underreport.aleo;
function do:
    input r0 as u32.private;
    add r0 r0 into r1;
    output r1 as u32.public;",
        )
        .unwrap();

        // Create the deployment transaction.
        let transaction = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

        // Destructure the deployment transaction.
        let Transaction::Deploy(txid, program_owner, deployment, fee) = transaction else {
            panic!("Expected a deployment transaction");
        };

        // Decrease the number of reported variables in the verifying keys.
        let mut vks_with_underreport = Vec::with_capacity(deployment.verifying_keys().len());
        for (id, (vk, cert)) in deployment.verifying_keys() {
            let vk = VerifyingKey::new(Arc::new(vk.deref().clone()), vk.num_variables() - 2);
            vks_with_underreport.push((*id, (vk.clone(), cert.clone())));
        }

        // Create a new deployment transaction with the underreported verifying keys.
        let adjusted_deployment =
            Deployment::new(deployment.edition(), deployment.program().clone(), vks_with_underreport).unwrap();
        let adjusted_transaction = Transaction::Deploy(txid, program_owner, Box::new(adjusted_deployment), fee);

        // Verify the deployment transaction. It should error when synthesizing the first variable over the vk limit.
        let result = vm.check_transaction(&adjusted_transaction, None, rng);
        assert!(result.is_err());

        // Create a standard transaction
        // Prepare the inputs.
        let inputs = [
            Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
            Value::<CurrentNetwork>::from_str("1u64").unwrap(),
        ]
        .into_iter();

        // Execute.
        let transaction =
            vm.execute(&private_key, ("credits.aleo", "transfer_public"), inputs, None, 0, None, rng).unwrap();

        // Check that the deployment transaction will be aborted if injected into a block.
        let block = sample_next_block(&vm, &private_key, &[transaction, adjusted_transaction.clone()], rng).unwrap();

        // Check that the block aborts the deployment transaction.
        assert_eq!(block.aborted_transaction_ids(), &vec![adjusted_transaction.id()]);

        // Update the VM.
        vm.add_next_block(&block).unwrap();
    }

    #[test]
    #[ignore]
    fn test_deployment_memory_overload() {
        const NUM_DEPLOYMENTS: usize = 32;

        let rng = &mut TestRng::default();

        // Initialize a private key.
        let private_key = sample_genesis_private_key(rng);

        // Initialize a view key.
        let view_key = ViewKey::try_from(&private_key).unwrap();

        // Initialize the genesis block.
        let genesis = sample_genesis_block(rng);

        // Initialize the VM.
        let vm = sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Deploy the base program.
        let program = Program::from_str(
            r"
program program_layer_0.aleo;

mapping m:
    key as u8.public;
    value as u32.public;

function do:
    input r0 as u32.public;
    async do r0 into r1;
    output r1 as program_layer_0.aleo/do.future;

finalize do:
    input r0 as u32.public;
    set r0 into m[0u8];",
        )
        .unwrap();

        let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
        vm.add_next_block(&sample_next_block(&vm, &private_key, &[deployment], rng).unwrap()).unwrap();

        // For each layer, deploy a program that calls the program from the previous layer.
        for i in 1..NUM_DEPLOYMENTS {
            let mut program_string = String::new();
            // Add the import statements.
            for j in 0..i {
                program_string.push_str(&format!("import program_layer_{}.aleo;\n", j));
            }
            // Add the program body.
            program_string.push_str(&format!(
                "program program_layer_{i}.aleo;

mapping m:
    key as u8.public;
    value as u32.public;

function do:
    input r0 as u32.public;
    call program_layer_{prev}.aleo/do r0 into r1;
    async do r0 r1 into r2;
    output r2 as program_layer_{i}.aleo/do.future;

finalize do:
    input r0 as u32.public;
    input r1 as program_layer_{prev}.aleo/do.future;
    await r1;
    set r0 into m[0u8];",
                prev = i - 1
            ));
            // Construct the program.
            let program = Program::from_str(&program_string).unwrap();

            // Deploy the program.
            let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

            vm.add_next_block(&sample_next_block(&vm, &private_key, &[deployment], rng).unwrap()).unwrap();
        }

        // Fetch the unspent records.
        let records = genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
        trace!("Unspent Records:\n{:#?}", records);

        // Select a record to spend.
        let record = Some(records.values().next().unwrap().decrypt(&view_key).unwrap());

        // Prepare the inputs.
        let inputs = [Value::<CurrentNetwork>::from_str("1u32").unwrap()].into_iter();

        // Execute.
        let transaction =
            vm.execute(&private_key, ("program_layer_30.aleo", "do"), inputs, record, 0, None, rng).unwrap();

        // Verify.
        vm.check_transaction(&transaction, None, rng).unwrap();
    }

    #[test]
    fn test_transfer_public_from_user() {
        let rng = &mut TestRng::default();

        // Initialize a new caller.
        let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
        let caller_address = Address::try_from(&caller_private_key).unwrap();

        // Initialize a recipient.
        let recipient_private_key = PrivateKey::new(rng).unwrap();
        let recipient_address = Address::try_from(&recipient_private_key).unwrap();

        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();

        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Check the balance of the caller.
        let credits_program_id = ProgramID::from_str("credits.aleo").unwrap();
        let account_mapping_name = Identifier::from_str("account").unwrap();
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(caller_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 182_499_999_894_244, "Update me if the initial balance changes.");

        // Transfer credits from the caller to the recipient.
        let transaction = vm
            .execute(
                &caller_private_key,
                ("credits.aleo", "transfer_public"),
                [Value::from_str(&format!("{recipient_address}")).unwrap(), Value::from_str("1u64").unwrap()].iter(),
                None,
                0,
                None,
                rng,
            )
            .unwrap();

        // Verify the transaction.
        vm.check_transaction(&transaction, None, rng).unwrap();

        // Add the transaction to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng).unwrap();

        // Update the VM.
        vm.add_next_block(&block).unwrap();

        // Check the balance of the caller.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(caller_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 182_499_999_843_183, "Update me if the initial balance changes.");

        // Check the balance of the recipient.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(recipient_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 1, "Update me if the test amount changes.");
    }

    #[test]
    fn test_transfer_public_as_signer_from_user() {
        let rng = &mut TestRng::default();

        // Initialize a new caller.
        let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
        let caller_address = Address::try_from(&caller_private_key).unwrap();

        // Initialize a recipient.
        let recipient_private_key = PrivateKey::new(rng).unwrap();
        let recipient_address = Address::try_from(&recipient_private_key).unwrap();

        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();

        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Check the balance of the caller.
        let credits_program_id = ProgramID::from_str("credits.aleo").unwrap();
        let account_mapping_name = Identifier::from_str("account").unwrap();
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(caller_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 182_499_999_894_244, "Update me if the initial balance changes.");

        // Transfer credits from the caller to the recipient.
        let transaction = vm
            .execute(
                &caller_private_key,
                ("credits.aleo", "transfer_public_as_signer"),
                [Value::from_str(&format!("{recipient_address}")).unwrap(), Value::from_str("1u64").unwrap()].iter(),
                None,
                0,
                None,
                rng,
            )
            .unwrap();

        // Verify the transaction.
        vm.check_transaction(&transaction, None, rng).unwrap();

        // Add the transaction to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng).unwrap();

        // Update the VM.
        vm.add_next_block(&block).unwrap();

        // Check the balance of the caller.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(caller_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 182_499_999_843_163, "Update me if the initial balance changes.");

        // Check the balance of the recipient.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(recipient_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 1, "Update me if the test amount changes.");
    }

    #[test]
    fn transfer_public_from_program() {
        let rng = &mut TestRng::default();

        // Initialize a new caller.
        let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
        let caller_address = Address::try_from(&caller_private_key).unwrap();

        // Initialize a recipient.
        let recipient_private_key = PrivateKey::new(rng).unwrap();
        let recipient_address = Address::try_from(&recipient_private_key).unwrap();

        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();

        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Check the balance of the caller.
        let credits_program_id = ProgramID::from_str("credits.aleo").unwrap();
        let account_mapping_name = Identifier::from_str("account").unwrap();
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(caller_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 182_499_999_894_244, "Update me if the initial balance changes.");

        // Initialize a wrapper program, importing `credits.aleo` and calling `transfer_public`.
        let program = Program::from_str(
            r"
import credits.aleo;
program credits_wrapper.aleo;

function transfer_public:
    input r0 as address.public;
    input r1 as u64.public;
    call credits.aleo/transfer_public r0 r1 into r2;
    async transfer_public r2 into r3;
    output r3 as credits_wrapper.aleo/transfer_public.future;

finalize transfer_public:
    input r0 as credits.aleo/transfer_public.future;
    await r0;
        ",
        )
        .unwrap();

        // Get the address of the wrapper program.
        let wrapper_program_id = ProgramID::from_str("credits_wrapper.aleo").unwrap();
        let wrapper_program_address = wrapper_program_id.to_address().unwrap();

        // Deploy the wrapper program.
        let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();

        // Add the deployment to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();

        // Update the VM.
        vm.add_next_block(&block).unwrap();

        // Transfer credits from the caller to the `credits_wrapper` program.
        let transaction = vm
            .execute(
                &caller_private_key,
                ("credits.aleo", "transfer_public"),
                [Value::from_str(&format!("{wrapper_program_address}")).unwrap(), Value::from_str("1u64").unwrap()]
                    .iter(),
                None,
                0,
                None,
                rng,
            )
            .unwrap();

        // Verify the transaction.
        vm.check_transaction(&transaction, None, rng).unwrap();

        // Add the transaction to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng).unwrap();

        // Update the VM.
        vm.add_next_block(&block).unwrap();

        // Check the balance of the caller.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(caller_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 182_499_996_914_808, "Update me if the initial balance changes.");

        // Check the balance of the `credits_wrapper` program.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(wrapper_program_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 1, "Update me if the test amount changes.");

        // Transfer credits from the `credits_wrapper` program to the recipient.
        let transaction = vm
            .execute(
                &caller_private_key,
                ("credits_wrapper.aleo", "transfer_public"),
                [Value::from_str(&format!("{recipient_address}")).unwrap(), Value::from_str("1u64").unwrap()].iter(),
                None,
                0,
                None,
                rng,
            )
            .unwrap();

        // Verify the transaction.
        vm.check_transaction(&transaction, None, rng).unwrap();

        // Add the transaction to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng).unwrap();

        // Update the VM.
        vm.add_next_block(&block).unwrap();

        // Check the balance of the caller.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(caller_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 182_499_996_862_283, "Update me if the initial balance changes.");

        // Check the balance of the `credits_wrapper` program.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(wrapper_program_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 0);

        // Check the balance of the recipient.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(recipient_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 1, "Update me if the test amount changes.");
    }

    #[test]
    fn transfer_public_as_signer_from_program() {
        let rng = &mut TestRng::default();

        // Initialize a new caller.
        let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
        let caller_address = Address::try_from(&caller_private_key).unwrap();

        // Initialize a recipient.
        let recipient_private_key = PrivateKey::new(rng).unwrap();
        let recipient_address = Address::try_from(&recipient_private_key).unwrap();

        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();

        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Check the balance of the caller.
        let credits_program_id = ProgramID::from_str("credits.aleo").unwrap();
        let account_mapping_name = Identifier::from_str("account").unwrap();
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(caller_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 182_499_999_894_244, "Update me if the initial balance changes.");

        // Initialize a wrapper program, importing `credits.aleo` and calling `transfer_public`.
        let program = Program::from_str(
            r"
import credits.aleo;
program credits_wrapper.aleo;

function transfer_public_as_signer:
    input r0 as address.public;
    input r1 as u64.public;
    call credits.aleo/transfer_public_as_signer r0 r1 into r2;
    async transfer_public_as_signer r2 into r3;
    output r3 as credits_wrapper.aleo/transfer_public_as_signer.future;

finalize transfer_public_as_signer:
    input r0 as credits.aleo/transfer_public_as_signer.future;
    await r0;
        ",
        )
        .unwrap();

        // Get the address of the wrapper program.
        let wrapper_program_id = ProgramID::from_str("credits_wrapper.aleo").unwrap();
        let wrapper_program_address = wrapper_program_id.to_address().unwrap();

        // Deploy the wrapper program.
        let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();

        // Add the deployment to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();

        // Update the VM.
        vm.add_next_block(&block).unwrap();

        // Transfer credits from the signer using `credits_wrapper` program.
        let transaction = vm
            .execute(
                &caller_private_key,
                ("credits_wrapper.aleo", "transfer_public_as_signer"),
                [Value::from_str(&format!("{recipient_address}")).unwrap(), Value::from_str("1u64").unwrap()].iter(),
                None,
                0,
                None,
                rng,
            )
            .unwrap();

        // Verify the transaction.
        vm.check_transaction(&transaction, None, rng).unwrap();

        // Add the transaction to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng).unwrap();

        // Update the VM.
        vm.add_next_block(&block).unwrap();

        // Check the balance of the caller.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(caller_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 182_499_996_821_793, "Update me if the initial balance changes.");

        // Check the `credits_wrapper` program does not have any balance.
        let balance = vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(wrapper_program_address)),
            )
            .unwrap();
        assert!(balance.is_none());

        // Check the balance of the recipient.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(recipient_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 1, "Update me if the test amount changes.");
    }

    #[test]
    fn test_transfer_public_to_private_from_program() {
        let rng = &mut TestRng::default();

        // Initialize a new caller.
        let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
        let caller_address = Address::try_from(&caller_private_key).unwrap();

        // Initialize a recipient.
        let recipient_private_key = PrivateKey::new(rng).unwrap();
        let recipient_address = Address::try_from(&recipient_private_key).unwrap();

        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();

        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Check the balance of the caller.
        let credits_program_id = ProgramID::from_str("credits.aleo").unwrap();
        let account_mapping_name = Identifier::from_str("account").unwrap();
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(caller_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 182_499_999_894_244, "Update me if the initial balance changes.");

        // Check that the recipient does not have a public balance.
        let balance = vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(recipient_address)),
            )
            .unwrap();
        assert!(balance.is_none());

        // Initialize a wrapper program, importing `credits.aleo` and calling `transfer_public_as_signer` then `transfer_public_to_private`.
        let program = Program::from_str(
            r"
import credits.aleo;

program credits_wrapper.aleo;

function transfer_public_to_private:
    input r0 as address.private;
    input r1 as u64.public;
    call credits.aleo/transfer_public_as_signer credits_wrapper.aleo r1 into r2;
    call credits.aleo/transfer_public_to_private r0 r1 into r3 r4;
    async transfer_public_to_private r2 r4 into r5;
    output r3 as credits.aleo/credits.record;
    output r5 as credits_wrapper.aleo/transfer_public_to_private.future;

finalize transfer_public_to_private:
    input r0 as credits.aleo/transfer_public_as_signer.future;
    input r1 as credits.aleo/transfer_public_to_private.future;
    contains credits.aleo/account[credits_wrapper.aleo] into r2;
    assert.eq r2 false;
    await r0;
    get credits.aleo/account[credits_wrapper.aleo] into r3;
    assert.eq r3 r0[2u32];
    await r1;
        ",
        )
        .unwrap();

        // Get the address of the wrapper program.
        let wrapper_program_id = ProgramID::from_str("credits_wrapper.aleo").unwrap();

        // Deploy the wrapper program.
        let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();

        // Add the deployment to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();

        // Update the VM.
        vm.add_next_block(&block).unwrap();

        // Call the wrapper program to transfer credits from the caller to the recipient.
        let transaction = vm
            .execute(
                &caller_private_key,
                ("credits_wrapper.aleo", "transfer_public_to_private"),
                [Value::from_str(&format!("{recipient_address}")).unwrap(), Value::from_str("1u64").unwrap()].iter(),
                None,
                0,
                None,
                rng,
            )
            .unwrap();

        // Verify the transaction.
        vm.check_transaction(&transaction, None, rng).unwrap();

        // Add the transaction to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[transaction.clone()], rng).unwrap();

        // Update the VM.
        vm.add_next_block(&block).unwrap();

        // Check the balance of the caller.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(caller_address)),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };

        assert_eq!(balance, 182_499_996_071_881, "Update me if the initial balance changes.");

        // Check that the `credits_wrapper` program has a balance of 0.
        let balance = match vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(wrapper_program_id.to_address().unwrap())),
            )
            .unwrap()
        {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) => *balance,
            _ => panic!("Expected a valid balance"),
        };
        assert_eq!(balance, 0);

        // Check that the recipient does not have a public balance.
        let balance = vm
            .finalize_store()
            .get_value_confirmed(
                credits_program_id,
                account_mapping_name,
                &Plaintext::from(Literal::Address(recipient_address)),
            )
            .unwrap();
        assert!(balance.is_none());

        // Get the output record from the transaction and check that it is well-formed.
        let records = transaction.records().collect_vec();
        assert_eq!(records.len(), 1);
        let (commitment, record) = records[0];
        let record = record.decrypt(&ViewKey::try_from(&recipient_private_key).unwrap()).unwrap();
        assert_eq!(**record.owner(), recipient_address);
        let data = record.data();
        assert_eq!(data.len(), 1);
        match data.get(&Identifier::from_str("microcredits").unwrap()) {
            Some(Entry::<CurrentNetwork, _>::Private(Plaintext::Literal(Literal::U64(value), _))) => {
                assert_eq!(**value, 1)
            }
            _ => panic!("Incorrect record."),
        }

        // Check that the record exists in the VM.
        assert!(vm.transition_store().get_record(commitment).unwrap().is_some());

        // Check that the serial number of the record does not exist in the VM.
        assert!(
            !vm.transition_store()
                .contains_serial_number(
                    &Record::<CurrentNetwork, Plaintext<CurrentNetwork>>::serial_number(
                        recipient_private_key,
                        *commitment
                    )
                    .unwrap()
                )
                .unwrap()
        );
    }

    #[test]
    fn test_large_transaction_is_aborted() {
        let rng = &mut TestRng::default();

        // Initialize a new caller.
        let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();

        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Deploy a program that produces small transactions.
        let program = small_transaction_program();

        // Deploy the program.
        let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();

        // Add the deployment to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();

        // Update the VM.
        vm.add_next_block(&block).unwrap();

        // Deploy a program that produces large transactions.
        let program = large_transaction_program();

        // Deploy the program.
        let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();

        // Add the deployment to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();

        // Update the VM.
        vm.add_next_block(&block).unwrap();

        // Call the program to produce the small transaction.
        let transaction = vm
            .execute(
                &caller_private_key,
                ("testing_small.aleo", "small_transaction"),
                Vec::<Value<CurrentNetwork>>::new().iter(),
                None,
                0,
                None,
                rng,
            )
            .unwrap();

        // Verify the transaction.
        vm.check_transaction(&transaction, None, rng).unwrap();

        // Add the transaction to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng).unwrap();

        // Check that the transaction was accepted.
        assert_eq!(block.transactions().num_accepted(), 1);

        // Update the VM.
        vm.add_next_block(&block).unwrap();

        // Call the program to produce a large transaction.
        let transaction = vm
            .execute(
                &caller_private_key,
                ("testing_large.aleo", "large_transaction"),
                Vec::<Value<CurrentNetwork>>::new().iter(),
                None,
                0,
                None,
                rng,
            )
            .unwrap();

        // Verify that the transaction is invalid.
        assert!(vm.check_transaction(&transaction, None, rng).is_err());

        // Add the transaction to a block and update the VM.
        let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng).unwrap();

        // Check that the transaction was aborted.
        assert_eq!(block.aborted_transaction_ids().len(), 1);

        // Update the VM.
        vm.add_next_block(&block).unwrap();
    }

    #[test]
    fn test_vm_puzzle() {
        // Attention: This test is used to ensure that the VM has performed downcasting correctly for
        // the puzzle, and that the underlying traits in the puzzle are working correctly. Please
        // *do not delete* this test as it is a critical safety check for the integrity of the
        // instantiation of the puzzle in the VM.

        let rng = &mut TestRng::default();

        // Initialize the VM.
        let vm = sample_vm();

        // Ensure this call succeeds.
        vm.puzzle.prove(rng.gen(), rng.gen(), rng.gen(), None).unwrap();
    }

    #[cfg(feature = "rocks")]
    #[test]
    fn test_atomic_unpause_on_error() {
        let rng = &mut TestRng::default();

        // Initialize a genesis private key..
        let genesis_private_key = sample_genesis_private_key(rng);

        // Initialize the genesis block.
        let genesis = sample_genesis_block(rng);

        // Initialize a VM and sample 2 blocks using it.
        let vm = sample_vm();
        vm.add_next_block(&genesis).unwrap();
        let block1 = sample_next_block(&vm, &genesis_private_key, &[], rng).unwrap();
        vm.add_next_block(&block1).unwrap();
        let block2 = sample_next_block(&vm, &genesis_private_key, &[], rng).unwrap();

        // Create a new, rocks-based VM shadowing the 1st one.
        let tempdir = tempfile::tempdir().unwrap();
        let vm = sample_vm_rocks(tempdir.path());
        vm.add_next_block(&genesis).unwrap();
        // This time, however, try to insert the 2nd block first, which fails due to height.
        assert!(vm.add_next_block(&block2).is_err());

        // It should still be possible to insert the 1st block afterwards.
        vm.add_next_block(&block1).unwrap();
    }
}
