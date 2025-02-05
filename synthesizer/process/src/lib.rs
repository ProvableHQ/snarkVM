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
#![allow(clippy::too_many_arguments)]
// #![warn(clippy::cast_possible_truncation)]
// TODO (howardwu): Update the return type on `execute` after stabilizing the interface.
#![allow(clippy::type_complexity)]

mod cost;
pub use cost::*;

mod stack;
pub use stack::*;

mod trace;
pub use trace::*;

mod traits;
pub use traits::*;

mod authorize;
mod deploy;
mod evaluate;
mod execute;
mod finalize;
mod verify_deployment;
mod verify_execution;
mod verify_fee;

#[cfg(test)]
mod tests;

use console::{
    account::PrivateKey,
    network::prelude::*,
    program::{Identifier, Literal, Locator, Plaintext, ProgramID, Record, Response, Value, compute_function_id},
    types::{Field, U16, U64},
};
use ledger_block::{Deployment, Execution, Fee, Input, Output, Transition};
use ledger_store::{ConsensusStore, FinalizeStorage, FinalizeStore, atomic_batch_scope};
use synthesizer_program::{
    Branch,
    Closure,
    Command,
    Finalize,
    FinalizeGlobalState,
    FinalizeOperation,
    Instruction,
    Program,
    RegistersLoad,
    RegistersStore,
    StackProgram,
};
use synthesizer_snark::{ProvingKey, UniversalSRS, VerifyingKey};

use aleo_std::{
    StorageMode,
    prelude::{finish, lap, timer},
};
use lru::LruCache;
use parking_lot::{Mutex, RwLock};
use std::{collections::HashMap, num::NonZeroUsize, sync::Arc};
use tracing::{debug, warn};

#[cfg(feature = "aleo-cli")]
use colored::Colorize;

#[cfg(not(feature = "rocks"))]
use ledger_store::helpers::memory::ConsensusMemory;
#[cfg(feature = "rocks")]
use ledger_store::helpers::rocksdb::ConsensusDB;

type NumParentsInMemory = usize;

#[derive(Clone)]
pub struct Process<N: Network> {
    /// The universal SRS.
    universal_srs: Arc<UniversalSRS<N>>,
    /// The Stack for credits.aleo
    credits: Option<Arc<Stack<N>>>,
    /// The LRU cache of stacks.
    stacks: Arc<Mutex<LruCache<ProgramID<N>, (Arc<Stack<N>>, NumParentsInMemory)>>>,
    /// The storage.
    #[cfg(feature = "rocks")]
    store: Option<ConsensusStore<N, ConsensusDB<N>>>,
    #[cfg(not(feature = "rocks"))]
    store: Option<ConsensusStore<N, ConsensusMemory<N>>>,
}

impl<N: Network> Process<N> {
    /// Initializes a new process.
    #[inline]
    pub fn setup<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(rng: &mut R) -> Result<Self> {
        let timer = timer!("Process:setup");
        // Initialize the process.
        let mut process = Self::load_no_storage()?;
        lap!(timer, "Initialize process");

        // Initialize the 'credits.aleo' program.
        let program = Program::credits()?;
        lap!(timer, "Load credits program");

        // Compute the 'credits.aleo' program stack.
        let stack = Stack::new(&process, &program)?;
        lap!(timer, "Initialize stack");

        // Synthesize the 'credits.aleo' circuit keys.
        for function_name in program.functions().keys() {
            stack.synthesize_key::<A, _>(function_name, rng)?;
            lap!(timer, "Synthesize circuit keys for {function_name}");
        }
        lap!(timer, "Synthesize credits program keys");

        // Add the 'credits.aleo' stack to the process.
        process.credits = Some(Arc::new(stack));

        finish!(timer);
        // Return the process.
        Ok(process)
    }

    /// Adds a new program to the process.
    /// If you intend to `execute` the program, use `deploy` and `finalize_deployment` instead.
    #[inline]
    pub fn add_program(&mut self, program: &Program<N>) -> Result<()> {
        // Initialize the 'credits.aleo' program ID.
        let credits_program_id = ProgramID::<N>::from_str("credits.aleo")?;
        // If the program is not 'credits.aleo', compute the program stack, and add it to the process.
        if program.id() != &credits_program_id {
            self.add_stack(Arc::new(Stack::new(self, program)?))?;
        }
        Ok(())
    }

    /// Adds a new stack to the LRU cache in the process.
    /// If you intend to `execute` the program, use `deploy` and `finalize_deployment` instead.
    #[inline]
    pub fn add_stack(&self, stack: Arc<Stack<N>>) -> Result<()> {
        // Construct the 'credits.aleo' program ID.
        let credits_id = ProgramID::<N>::from_str("credits.aleo")?;
        // Collect all direct and indirect external stacks.
        let programs_to_add = stack.all_external_stacks();
        // Obtain the lock on the stacks.
        let mut stacks = self.stacks.lock();
        // Determine which stacks still need to be added into the process.
        let programs_to_add = programs_to_add
            .into_iter()
            .chain(std::iter::once((*stack.program_id(), stack))) // add the root stack.
            .unique_by(|(id, _)|*id) // don't add duplicates.
            .filter(|(program_id, _)| {
                *program_id != credits_id // don't add the credits.aleo stack.
                && !stacks.contains(program_id) // don't add stacks present in the cache.
            })
            .collect::<Vec<_>>();
        // Determine the required capacity.
        let remaining_capacity = stacks.cap().get().saturating_sub(stacks.len());
        let mut required_capacity = programs_to_add.len().saturating_sub(remaining_capacity);
        if required_capacity != 0 {
            debug!("Evicting {required_capacity} stacks from the cache.");
        }
        // If the new stacks require more capacity, attempt to remove the least recently used stacks.
        while required_capacity != 0 {
            // To avoid dangling stacks, we only remove stacks which are not imported by any other stack.
            let root_program_ids = stacks
                .iter()
                .rev()
                .filter_map(|(program_id, (_, num_parents))| (*num_parents == 0).then_some(program_id))
                .take(required_capacity)
                .cloned()
                .collect::<Vec<_>>();
            // Evict the old stacks from the cache.
            for program_id in root_program_ids {
                if let Some((removed_stack, _)) = stacks.pop(&program_id) {
                    // Decrement the number of tracked parents of the external stacks.
                    for (import_program_id, _) in
                        removed_stack.program().imports().into_iter().filter(|(id, _)| **id != credits_id)
                    {
                        if let Some((_, num_parents)) = stacks.get_mut(import_program_id) {
                            *num_parents = num_parents.saturating_sub(1);
                        } else {
                            bail!("Could not find expected import program id {} in cache.", import_program_id)
                        }
                    }
                } else {
                    bail!("Could not find expected program id {program_id} in cache.")
                }
                // Lower the required capacity.
                required_capacity = required_capacity.saturating_sub(1);
            }
        }
        // Add new stacks into the cache, from lowest the highest level.
        for (program_id, stack) in programs_to_add {
            // Increment the tracked number of parents of the external stacks.
            for (import_program_id, _) in stack.program().imports().into_iter().filter(|(id, _)| **id != credits_id) {
                if let Some((_, num_parents)) = stacks.get_mut(import_program_id) {
                    *num_parents += 1;
                } else {
                    bail!("Could not find expected import program id {} in cache.", import_program_id)
                }
            }
            // Add the stack itself into the cache.
            stacks.put(program_id, (stack, 0));
        }
        Ok(())
    }

    /// Returns the size of the cache.
    #[inline]
    pub fn num_stacks_in_memory(&self) -> usize {
        self.stacks.lock().len()
    }
}

macro_rules! impl_load_from_store {
    ($store_type:ty) => {
        /// Initializes a new process.
        pub fn load_from_store(store: $store_type) -> Result<Self> {
            let timer = timer!("Process::load_from_storage");

            // Initialize the process.
            let mut process = Self {
                universal_srs: Arc::new(UniversalSRS::load()?),
                credits: None,
                stacks: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(N::MAX_STACKS).unwrap()))),
                store: Some(store),
            };
            lap!(timer, "Initialize process");

            // Initialize the 'credits.aleo' program.
            let program = Program::credits()?;
            lap!(timer, "Load credits program");

            // Compute the 'credits.aleo' program stack.
            let stack = Stack::new(&process, &program)?;
            lap!(timer, "Initialize stack");

            // Synthesize the 'credits.aleo' verifying keys.
            for function_name in program.functions().keys() {
                // Load the verifying key.
                let verifying_key = N::get_credits_verifying_key(function_name.to_string())?;
                // Retrieve the number of public and private variables.
                // Note: This number does *NOT* include the number of constants. This is safe because
                // this program is never deployed, as it is a first-class citizen of the protocol.
                let num_variables = verifying_key.circuit_info.num_public_and_private_variables as u64;
                // Insert the verifying key.
                stack.insert_verifying_key(function_name, VerifyingKey::new(verifying_key.clone(), num_variables))?;
                lap!(timer, "Load verifying key for {function_name}");
            }
            lap!(timer, "Load circuit keys");

            // Add the stack to the process.
            process.credits = Some(Arc::new(stack));

            finish!(timer, "Process::load_from_storage");
            // Return the process.
            Ok(process)
        }
    };
}

impl<N: Network> Process<N> {
    #[cfg(feature = "rocks")]
    impl_load_from_store!(ConsensusStore<N, ConsensusDB<N>>);

    #[cfg(not(feature = "rocks"))]
    impl_load_from_store!(ConsensusStore<N, ConsensusMemory<N>>);

    /// Initializes a new process.
    /// Assumption: this is only called in test code.
    #[inline]
    pub fn load_testing_only() -> Result<Self> {
        Process::load_from_store(ConsensusStore::open(StorageMode::Development(0)).unwrap())
    }

    /// Initializes a new process without creating the 'credits.aleo' program.
    #[inline]
    pub fn load_no_storage() -> Result<Self> {
        // Initialize the process.
        let process = Self {
            universal_srs: Arc::new(UniversalSRS::load()?),
            credits: None,
            stacks: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(N::MAX_STACKS).unwrap()))),
            store: None,
        };

        // Return the process.
        Ok(process)
    }

    /// Initializes a new process without downloading the 'credits.aleo' circuit keys (for web contexts).
    #[inline]
    #[cfg(feature = "wasm")]
    pub fn load_web() -> Result<Self> {
        // Initialize the process.
        let mut process = Self {
            universal_srs: Arc::new(UniversalSRS::load()?),
            credits: None,
            stacks: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(N::MAX_STACKS).unwrap()))),
            store: None,
        };

        // Initialize the 'credits.aleo' program.
        let program = Program::credits()?;

        // Compute the 'credits.aleo' program stack.
        let stack = Stack::new(&process, &program)?;

        // Add the stack to the process.
        process.credits = Some(Arc::new(stack));

        // Return the process.
        Ok(process)
    }

    /// Returns the universal SRS.
    #[inline]
    pub const fn universal_srs(&self) -> &Arc<UniversalSRS<N>> {
        &self.universal_srs
    }

    /// Returns `true` if the process or storage contains the program with the given ID.
    #[inline]
    pub fn contains_program(&self, program_id: &ProgramID<N>) -> bool {
        // Check if the program is in memory.
        if self.contains_program_in_cache(program_id) {
            return true;
        }
        // Retrieve the stores.
        if let Some(store) = self.store.as_ref() {
            let transaction_store = store.transaction_store();
            let deployment_store = transaction_store.deployment_store();
            // Check if the program ID exists in the storage.
            match deployment_store.find_transaction_id_from_program_id(program_id) {
                Ok(Some(_)) => return true,
                Ok(None) => debug!("Program ID {program_id} not found in storage"),
                Err(err) => warn!("Could not retrieve transaction ID for program ID {program_id}: {err}"),
            }
        }

        false
    }

    /// Returns `true` if the process contains the program with the given ID.
    #[inline]
    pub fn contains_program_in_cache(&self, program_id: &ProgramID<N>) -> bool {
        // Check if the program ID is 'credits.aleo'.
        if self.credits.as_ref().map_or(false, |stack| stack.program_id() == program_id) {
            return true;
        }
        // Check if the program ID exists in the cache.
        self.stacks.lock().contains(program_id)
    }

    /// Returns the stack for the given program ID.
    /// Note: stacks are large, so queried stacks should be short-lived to avoid memory leaks.
    #[inline]
    pub fn get_stack(&self, program_id: impl TryInto<ProgramID<N>>) -> Result<Arc<Stack<N>>> {
        // Prepare the program ID.
        let program_id = program_id.try_into().map_err(|_| anyhow!("Invalid program ID"))?;
        // Check if the program is 'credits.aleo'.
        if self.credits.as_ref().map_or(false, |stack| *stack.program_id() == program_id) {
            return self.credits.clone().ok_or_else(|| anyhow!("Failed to get stack for 'credits.aleo'"));
        }
        // Try to retrieve the stack from the LRU cache.
        if let Some((stack, _)) = self.stacks.lock().get(&program_id) {
            // Return the stack.
            return Ok(stack.clone());
        }
        // Otherwise, retrieve the stack from the storage.
        self.load_stack(program_id)
    }

    /// Returns the program for the given program ID.
    #[inline]
    pub fn get_program(&self, program_id: impl TryInto<ProgramID<N>>) -> Result<Program<N>> {
        let stack = self.get_stack(program_id)?;
        Ok(stack.program().clone())
    }

    /// Returns the proving key for the given program ID and function name.
    #[inline]
    pub fn get_proving_key(
        &self,
        program_id: impl TryInto<ProgramID<N>>,
        function_name: impl TryInto<Identifier<N>>,
    ) -> Result<ProvingKey<N>> {
        // Prepare the function name.
        let function_name = function_name.try_into().map_err(|_| anyhow!("Invalid function name"))?;
        // Return the proving key.
        self.get_stack(program_id)?.get_proving_key(&function_name)
    }

    /// Returns the verifying key for the given program ID and function name.
    #[inline]
    pub fn get_verifying_key(
        &self,
        program_id: impl TryInto<ProgramID<N>>,
        function_name: impl TryInto<Identifier<N>>,
    ) -> Result<VerifyingKey<N>> {
        // Prepare the function name.
        let function_name = function_name.try_into().map_err(|_| anyhow!("Invalid function name"))?;
        // Return the verifying key.
        self.get_stack(program_id)?.get_verifying_key(&function_name)
    }

    /// Inserts the given proving key, for the given program ID and function name.
    #[inline]
    pub fn insert_proving_key(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        proving_key: ProvingKey<N>,
    ) -> Result<()> {
        self.get_stack(program_id)?.insert_proving_key(function_name, proving_key)
    }

    /// Inserts the given verifying key, for the given program ID and function name.
    #[inline]
    pub fn insert_verifying_key(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        verifying_key: VerifyingKey<N>,
    ) -> Result<()> {
        self.get_stack(program_id)?.insert_verifying_key(function_name, verifying_key)
    }

    /// Synthesizes the proving and verifying key for the given program ID and function name.
    #[inline]
    pub fn synthesize_key<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        rng: &mut R,
    ) -> Result<()> {
        // Synthesize the proving and verifying key.
        self.get_stack(program_id)?.synthesize_key::<A, R>(function_name, rng)
    }
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use console::{account::PrivateKey, network::MainnetV0, program::Identifier};
    use ledger_block::Transition;
    use ledger_query::Query;
    use ledger_store::{BlockStore, helpers::memory::BlockMemory};
    use synthesizer_program::Program;

    use once_cell::sync::OnceCell;

    type CurrentNetwork = MainnetV0;
    type CurrentAleo = circuit::network::AleoV0;

    /// Returns an execution for the given program and function name.
    pub fn get_execution(
        process: &mut Process<CurrentNetwork>,
        program: &Program<CurrentNetwork>,
        function_name: &Identifier<CurrentNetwork>,
        inputs: impl ExactSizeIterator<Item = impl TryInto<Value<CurrentNetwork>>>,
    ) -> Execution<CurrentNetwork> {
        // Initialize a new rng.
        let rng = &mut TestRng::default();

        // Initialize a private key.
        let private_key = PrivateKey::new(rng).unwrap();

        // Add the program to the process if doesn't yet exist.
        if !process.contains_program(program.id()) {
            process.add_program(program).unwrap();
        }

        // Compute the authorization.
        let authorization =
            process.authorize::<CurrentAleo, _>(&private_key, program.id(), function_name, inputs, rng).unwrap();

        // Execute the program.
        let (_, mut trace) = process.execute::<CurrentAleo, _>(authorization, rng).unwrap();

        // Initialize a new block store.
        let block_store = BlockStore::<CurrentNetwork, BlockMemory<_>>::open(None).unwrap();

        // Prepare the assignments from the block store.
        trace.prepare(ledger_query::Query::from(block_store)).unwrap();

        // Get the locator.
        let locator = format!("{:?}:{function_name:?}", program.id());

        // Return the execution object.
        trace.prove_execution::<CurrentAleo, _>(&locator, rng).unwrap()
    }

    pub fn sample_key() -> (Identifier<CurrentNetwork>, ProvingKey<CurrentNetwork>, VerifyingKey<CurrentNetwork>) {
        static INSTANCE: OnceCell<(
            Identifier<CurrentNetwork>,
            ProvingKey<CurrentNetwork>,
            VerifyingKey<CurrentNetwork>,
        )> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                // Initialize a new program.
                let (string, program) = Program::<CurrentNetwork>::parse(
                    r"
program testing.aleo;

function compute:
    input r0 as u32.private;
    input r1 as u32.public;
    add r0 r1 into r2;
    output r2 as u32.public;",
                )
                .unwrap();
                assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");

                // Declare the function name.
                let function_name = Identifier::from_str("compute").unwrap();

                // Initialize the RNG.
                let rng = &mut TestRng::default();

                // Construct the process.
                let process = sample_process(&program);

                // Synthesize a proving and verifying key.
                process.synthesize_key::<CurrentAleo, _>(program.id(), &function_name, rng).unwrap();

                // Get the proving and verifying key.
                let proving_key = process.get_proving_key(program.id(), function_name).unwrap();
                let verifying_key = process.get_verifying_key(program.id(), function_name).unwrap();

                (function_name, proving_key, verifying_key)
            })
            .clone()
    }

    pub(crate) fn sample_execution() -> Execution<CurrentNetwork> {
        static INSTANCE: OnceCell<Execution<CurrentNetwork>> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                // Initialize a new program.
                let (string, program) = Program::<CurrentNetwork>::parse(
                    r"
program testing.aleo;

function compute:
    input r0 as u32.private;
    input r1 as u32.public;
    add r0 r1 into r2;
    output r2 as u32.public;",
                )
                .unwrap();
                assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");

                // Declare the function name.
                let function_name = Identifier::from_str("compute").unwrap();

                // Initialize the RNG.
                let rng = &mut TestRng::default();
                // Initialize a new caller account.
                let caller_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();

                // Initialize a new block store.
                let block_store = BlockStore::<CurrentNetwork, BlockMemory<_>>::open(None).unwrap();

                // Construct the process.
                let process = sample_process(&program);
                // Authorize the function call.
                let authorization = process
                    .authorize::<CurrentAleo, _>(
                        &caller_private_key,
                        program.id(),
                        function_name,
                        ["5u32", "10u32"].into_iter(),
                        rng,
                    )
                    .unwrap();
                assert_eq!(authorization.len(), 1);
                // Execute the request.
                let (_response, mut trace) = process.execute::<CurrentAleo, _>(authorization, rng).unwrap();
                assert_eq!(trace.transitions().len(), 1);

                // Prepare the trace.
                trace.prepare(Query::from(block_store)).unwrap();
                // Compute the execution.
                trace.prove_execution::<CurrentAleo, _>("testing", rng).unwrap()
            })
            .clone()
    }

    pub fn sample_transition() -> Transition<CurrentNetwork> {
        // Retrieve the execution.
        let mut execution = sample_execution();
        // Ensure the execution is not empty.
        assert!(!execution.is_empty());
        // Return the transition.
        execution.pop().unwrap()
    }

    /// Initializes a new process with the given program.
    pub(crate) fn sample_process(program: &Program<CurrentNetwork>) -> Process<CurrentNetwork> {
        // Construct a new process.
        let mut process = Process::load_testing_only().unwrap();
        // Add the program to the process.
        process.add_program(program).unwrap();
        // Return the process.
        process
    }
}
