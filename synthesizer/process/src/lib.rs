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
use ledger_block::{Deployment, Execution, Fee, Input, Transition};
use ledger_store::{FinalizeStorage, FinalizeStore, atomic_batch_scope};
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
use tracing::debug;

use aleo_std::{
    StorageMode,
    prelude::{finish, lap, timer},
};
use ledger_store::{ConsensusStore, TransactionStorage, TransactionStore};
use lru::LruCache;
use parking_lot::{Mutex, RwLock};
use std::{collections::HashMap, num::NonZeroUsize, sync::Arc};

#[cfg(feature = "aleo-cli")]
use colored::Colorize;

#[cfg(not(feature = "rocks"))]
use ledger_store::helpers::memory::ConsensusMemory;
#[cfg(feature = "rocks")]
use ledger_store::helpers::rocksdb::ConsensusDB;

#[cfg(feature = "rocks")]
#[derive(Clone)]
pub struct Process<N: Network> {
    /// The universal SRS.
    universal_srs: Arc<UniversalSRS<N>>,
    /// The Stack for credits.aleo
    credits: Option<Arc<Stack<N>>>,
    /// The mapping of program IDs to stacks.
    stacks: Arc<Mutex<LruCache<ProgramID<N>, Arc<Stack<N>>>>>,
    /// The storage.
    store: Option<ConsensusStore<N, ConsensusDB<N>>>,
}

#[cfg(not(feature = "rocks"))]
#[derive(Clone)]
pub struct Process<N: Network> {
    /// The universal SRS.
    universal_srs: Arc<UniversalSRS<N>>,
    /// The Stack for credits.aleo
    credits: Option<Arc<Stack<N>>>,
    /// The mapping of program IDs to stacks.
    stacks: Arc<Mutex<LruCache<ProgramID<N>, Arc<Stack<N>>>>>,
    /// The storage.
    store: Option<ConsensusStore<N, ConsensusMemory<N>>>,
}

impl<N: Network> Process<N> {
    /// Initializes a new process.
    #[inline]
    pub fn setup<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(rng: &mut R) -> Result<Self> {
        let timer = timer!("Process:setup");
        // Initialize the process.
        let mut process = Self {
            universal_srs: Arc::new(UniversalSRS::load()?),
            credits: None,
            stacks: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(N::MAX_STACKS).unwrap()))),
            store: None,
        };
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
            self.add_stack(Stack::new(self, program)?);
        }
        Ok(())
    }

    /// Adds a new stack to the process.
    /// If you intend to `execute` the program, use `deploy` and `finalize_deployment` instead.
    #[inline]
    pub fn add_stack(&self, stack: Stack<N>) {
        // Add the stack to the process.
        self.stacks.lock().put(*stack.program_id(), Arc::new(stack));
    }
}

impl<N: Network> Process<N> {
    /// Initializes a new process.
    /// Assumption: this is only called in test code.
    #[inline]
    pub fn load_testing_only() -> Result<Self> {
        Process::load_from_storage(Some(aleo_std::StorageMode::Development(0)))
    }

    /// Initializes a new process.
    #[inline]
    pub fn load_from_storage(storage_mode: Option<StorageMode>) -> Result<Self> {
        let timer = timer!("Process::load_from_storage");

        debug!("Opening storage");
        let storage_mode = storage_mode.clone().ok_or_else(|| anyhow!("Failed to get storage mode"))?;
        // try to lazy load the stack
        #[cfg(feature = "rocks")]
        let store = ConsensusStore::<N, ConsensusDB<N>>::open(storage_mode);
        #[cfg(not(feature = "rocks"))]
        let store = ConsensusStore::<N, ConsensusMemory<N>>::open(storage_mode);

        let store = match store {
            Ok(store) => store,
            Err(e) => bail!("Failed to load ledger (run 'snarkos clean' and try again)\n\n{e}\n"),
        };
        debug!("Opened storage");

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

    /// Returns `true` if the process contains the program with the given ID.
    #[inline]
    pub fn contains_program(&self, program_id: &ProgramID<N>) -> bool {
        if self.credits.as_ref().map_or(false, |stack| stack.program_id() == program_id) {
            return true;
        }
        self.stacks.lock().contains(program_id)
    }

    /// Loads the Stack and imported Stacks for the given program ID into memory.
    #[inline]
    fn load_stack(&self, program_id: impl TryInto<ProgramID<N>>) -> Result<()> {
        let program_id = program_id.try_into().map_err(|_| anyhow!("Invalid program ID"))?;
        debug!("Lazy loading stack for {program_id}");
        // Retrieve the stores.
        let store = self.store.as_ref().ok_or_else(|| anyhow!("Failed to get store"))?;
        let transaction_store = store.transaction_store();
        let deployment_store = transaction_store.deployment_store();
        // Retrieve the transaction ID.
        let transaction_id = deployment_store
            .find_transaction_id_from_program_id(&program_id)
            .map_err(|e| anyhow!("Program ID not found in storage: {e}"))?
            .ok_or_else(|| anyhow!("Program ID not found in storage"))?;
        // Collect the deployment and imports.
        let deployments = load_deployment_and_imports(self, transaction_store, transaction_id)?;
        // Load the deployment into memory.
        for deployment in deployments {
            self.load_deployment(&deployment.1)?;
        }
        debug!("Loaded stack for {program_id}");
        Ok(())
    }

    /// Returns the stack for the given program ID.
    #[inline]
    pub fn get_stack(&self, program_id: impl TryInto<ProgramID<N>>) -> Result<Arc<Stack<N>>> {
        // Prepare the program ID.
        let program_id = program_id.try_into().map_err(|_| anyhow!("Invalid program ID"))?;
        // Check if the program is 'credits.aleo'.
        if program_id == ProgramID::<N>::from_str("credits.aleo")? {
            return self.credits.clone().ok_or_else(|| anyhow!("Failed to get stack for 'credits.aleo'"));
        }
        // Maybe lazy load the stack
        if !self.contains_program(&program_id) {
            self.load_stack(program_id)?;
        }
        // Retrieve the stack.
        let mut lru_cache = self.stacks.lock();
        let stack = lru_cache.get(&program_id).ok_or_else(|| anyhow!("Failed to load stack"))?;
        // Ensure the program ID matches.
        ensure!(stack.program_id() == &program_id, "Expected program '{}', found '{program_id}'", stack.program_id());
        // Return the stack.
        Ok(stack.clone())
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
            deployments.extend_from_slice(&load_deployment_and_imports(process, transaction_store, transaction_id)?);
        }
    }

    // Once all the imports have been included, add the parent deployment.
    deployments.push((*program_id, deployment));

    Ok(deployments)
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
