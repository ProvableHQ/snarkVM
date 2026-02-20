// Copyright (c) 2019-2026 Provable Inc.
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

extern crate snarkvm_circuit as circuit;
extern crate snarkvm_console as console;

mod cost;
pub use cost::*;

mod stack;
pub use stack::*;

mod trace;
pub use trace::*;

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
    program::{
        Identifier,
        Literal,
        Locator,
        Plaintext,
        PlaintextType,
        ProgramID,
        Record,
        Request,
        Response,
        Value,
        compute_function_id,
    },
    types::{Field, U16, U64},
};
use snarkvm_algorithms::snark::varuna::VarunaVersion;
use snarkvm_ledger_block::{Deployment, Execution, Fee, Input, Output, Transaction, Transition};
use snarkvm_ledger_store::{FinalizeStorage, FinalizeStore, atomic_batch_scope};
use snarkvm_synthesizer_program::{
    Branch,
    Command,
    FinalizeGlobalState,
    FinalizeOperation,
    Instruction,
    Program,
    StackTrait,
};
use snarkvm_synthesizer_snark::{ProvingKey, UniversalSRS, VerifyingKey};
use snarkvm_utilities::{defer, dev_println};

use aleo_std::prelude::{finish, lap, timer};
use indexmap::IndexMap;
#[cfg(feature = "locktick")]
use locktick::parking_lot::RwLock;
#[cfg(not(feature = "locktick"))]
use parking_lot::RwLock;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
pub struct Process<N: Network> {
    /// The universal SRS.
    universal_srs: UniversalSRS<N>,
    /// The mapping of program IDs to stacks.
    stacks: Arc<RwLock<IndexMap<ProgramID<N>, Arc<Stack<N>>>>>,
    /// The mapping of program IDs to old stacks.
    old_stacks: Arc<RwLock<IndexMap<ProgramID<N>, Option<Arc<Stack<N>>>>>>,
}

impl<N: Network> Process<N> {
    /// Initializes a new process.
    #[inline]
    pub fn setup<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(rng: &mut R) -> Result<Self> {
        let timer = timer!("Process:setup");

        // Initialize the process.
        let mut process =
            Self { universal_srs: UniversalSRS::load()?, stacks: Default::default(), old_stacks: Default::default() };
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
        process.add_stack(stack);

        finish!(timer);
        // Return the process.
        Ok(process)
    }

    /// Adds a new stack to the process.
    /// If the program already exists, then the existing stack is replaced and the original stack is returned.
    /// Note. This method assumes that the provided stack is valid.
    #[inline]
    pub fn add_stack(&mut self, stack: Stack<N>) -> Option<Arc<Stack<N>>> {
        // Get the program ID.
        let program_id = *stack.program_id();
        // Arc the stack first to limit the scope of the write lock.
        let stack = Arc::new(stack);
        // Insert the stack into the process, replacing the existing stack if it exists.
        self.stacks.write().insert(program_id, stack)
    }

    /// Stages a stack to be added to the process.
    /// The new stack is active, while the old stack is retained in `old_stacks`.
    /// The `commit_stacks` method must be called to finalize the addition of the new stack.
    /// The `revert_stacks` method can be called to revert the staged stacks.
    #[inline]
    pub fn stage_stack(&self, stack: Stack<N>) {
        // Get the program ID.
        let program_id = *stack.program_id();
        // Arc the stack first to limit the scope of the write lock.
        let stack = Arc::new(stack);
        // If no entry in `old_stacks` exists for `program_id`, store the old stack.
        // Note: If `old_stack` is `None`, it means that we are adding a new program to the process.
        let old_stack = self.stacks.write().insert(program_id, stack);
        let mut old_stacks = self.old_stacks.write();
        if !old_stacks.contains_key(&program_id) {
            old_stacks.insert(program_id, old_stack);
        }
    }

    /// Commits the staged stacks to the process.
    /// This finalizes the addition of the new stacks and clears the old stacks.
    #[inline]
    pub fn commit_stacks(&self) {
        // Clear the old stacks.
        self.old_stacks.write().clear();
    }

    /// Reverts the staged stacks, restoring the previous state of the process.
    /// This will remove the new stacks and restore the old stacks.
    #[inline]
    pub fn revert_stacks(&self) {
        // Restore the old stacks.
        for (program_id, stack) in self.old_stacks.write().drain(..) {
            // If the stack is `None`, remove the program from the process.
            // Otherwise, insert the old stack back into the process.
            if let Some(stack) = stack {
                self.stacks.write().insert(program_id, stack);
            } else {
                self.stacks.write().shift_remove(&program_id);
            }
        }
    }

    /// Ensure that the types referred to in this program's mappings exist.
    pub fn mapping_types_exist(&self, program: &Program<N>) -> Result<()> {
        for mapping in program.mappings().values() {
            self.plaintext_exists(mapping.key().plaintext_type(), program)?;
            self.plaintext_exists(mapping.value().plaintext_type(), program)?;
        }
        Ok(())
    }

    // If `type_` is a struct or an array containing a struct, ensure the struct type exists.
    fn plaintext_exists(&self, type_: &PlaintextType<N>, program: &Program<N>) -> Result<()> {
        match type_ {
            PlaintextType::Literal(..) => Ok(()),
            PlaintextType::Struct(struct_name) => {
                // Retrieve the struct from the program.
                ensure!(
                    program.get_struct(struct_name).is_ok(),
                    "Struct '{struct_name}' in '{}' is not defined.",
                    program.id()
                );
                Ok(())
            }
            PlaintextType::ExternalStruct(locator) => {
                let stack = self.get_stack(locator.program_id())?;
                ensure!(
                    stack.program().get_struct(locator.resource()).is_ok(),
                    "Struct '{}' in '{}' is not defined.",
                    locator.resource(),
                    stack.program().id(),
                );
                Ok(())
            }
            PlaintextType::Array(array_type) => self.plaintext_exists(array_type.base_element_type(), program),
        }
    }
}

impl<N: Network> Process<N> {
    /// Initializes a new process.
    #[inline]
    pub fn load() -> Result<Self> {
        let timer = timer!("Process::load");

        // Initialize the process.
        let mut process =
            Self { universal_srs: UniversalSRS::load()?, stacks: Default::default(), old_stacks: Default::default() };
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
        process.add_stack(stack);

        finish!(timer, "Process::load");
        // Return the process.
        Ok(process)
    }

    /// Initializes a new process with the V0 credits.aleo verifiying keys.
    #[inline]
    pub fn load_v0() -> Result<Self> {
        let timer = timer!("Process::load_v0");

        // Initialize the process.
        let mut process =
            Self { universal_srs: UniversalSRS::load()?, stacks: Default::default(), old_stacks: Default::default() };
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
            let verifying_key = N::get_credits_v0_verifying_key(function_name.to_string())?;
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
        process.add_stack(stack);

        finish!(timer, "Process::load_v0");
        // Return the process.
        Ok(process)
    }

    /// Initializes a new process without downloading the 'credits.aleo' circuit keys (for web contexts).
    #[inline]
    #[cfg(feature = "wasm")]
    pub fn load_web() -> Result<Self> {
        // Initialize the process.
        let mut process =
            Self { universal_srs: UniversalSRS::load()?, stacks: Default::default(), old_stacks: Default::default() };

        // Initialize the 'credits.aleo' program.
        let program = Program::credits()?;

        // Compute the 'credits.aleo' program stack.
        let stack = Stack::new(&process, &program)?;

        // Add the stack to the process.
        process.add_stack(stack);

        // Return the process.
        Ok(process)
    }

    /// Adds a new program to the process, verifying that it is a valid addition.
    /// If the program exists, then the existing stack is replaced and discarded.
    /// Note. This method should **NOT** be used by the on-chain VM to add new program, use `finalize_deployment` or `load_deployment` instead instead.
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

    /// Adds a new program with the given edition to the process, verifying that it is a valid addition.
    /// If the program exists, then the existing stack is replaced and discarded.
    /// Note. This method should **NOT** be used by the on-chain VM to add new program, use `finalize_deployment` or `load_deployment` instead instead.
    #[inline]
    pub fn add_program_with_edition(&mut self, program: &Program<N>, edition: u16) -> Result<()> {
        // Initialize the 'credits.aleo' program ID.
        let credits_program_id = ProgramID::<N>::from_str("credits.aleo")?;
        // If the program is not 'credits.aleo', compute the program stack, and add it to the process.
        if program.id() != &credits_program_id {
            let stack = Stack::new_raw(self, program, edition)?;
            stack.initialize_and_check(self)?;
            self.add_stack(stack);
        }
        Ok(())
    }

    /// Adds a set of programs and editions, in topological order, to the process, deferring validation of the programs until all programs are added.
    /// If a program exists, then the existing stack is replaced and discarded.
    /// Either all programs are added or none are.
    /// Note. This method should **NOT** be used by the on-chain VM to add new program, use `finalize_deployment` or `load_deployment` instead instead.
    #[inline]
    pub fn add_programs_with_editions(&mut self, programs: &[(Program<N>, u16)]) -> Result<()> {
        // Initialize the 'credits.aleo' program ID.
        let credits_program_id = ProgramID::<N>::from_str("credits.aleo")?;
        // Defer cleanup of the uncommitted stacks.
        defer! {
            self.revert_stacks()
        }
        // Initialize raw stacks for each of the programs, skipping `credits.aleo`.
        for (program, edition) in programs {
            if program.id() != &credits_program_id {
                self.stage_stack(Stack::new_raw(self, program, *edition)?)
            }
        }
        // For each stack, check and initialize it before adding it to the process.
        for (program, _) in programs {
            // Retrieve the stack.
            let stack = self.get_stack(program.id())?;
            // Initialize and check the stack for well-formedness.
            stack.initialize_and_check(self)?;
        }
        // Commit the staged stacks.
        self.commit_stacks();
        Ok(())
    }

    /// Returns the universal SRS.
    #[inline]
    pub const fn universal_srs(&self) -> &UniversalSRS<N> {
        &self.universal_srs
    }

    /// Returns `true` if the process contains the program with the given ID.
    #[inline]
    pub fn contains_program(&self, program_id: &ProgramID<N>) -> bool {
        self.stacks.read().contains_key(program_id)
    }

    /// Returns the program IDs of all programs in the process.
    #[inline]
    pub fn program_ids(&self) -> Vec<ProgramID<N>> {
        self.stacks.read().keys().copied().collect()
    }

    /// Returns the stack for the given program ID.
    #[inline]
    pub fn get_stack(&self, program_id: impl TryInto<ProgramID<N>>) -> Result<Arc<Stack<N>>> {
        // Prepare the program ID.
        let program_id = program_id.try_into().map_err(|_| anyhow!("Invalid program ID"))?;
        // Retrieve the stack.
        let stack = self
            .stacks
            .read()
            .get(&program_id)
            .ok_or_else(|| anyhow!("Program '{program_id}' does not exist"))?
            .clone();
        // Ensure the program ID matches.
        ensure!(stack.program_id() == &program_id, "Expected program '{}', found '{program_id}'", stack.program_id());
        // Return the stack.
        Ok(stack)
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

    /// Removes the given proving key, for the given program ID and function name.
    #[inline]
    pub fn remove_proving_key(&self, program_id: &ProgramID<N>, function_name: &Identifier<N>) -> Result<()> {
        self.get_stack(program_id)?.remove_proving_key(function_name);
        Ok(())
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

    /// Removes the given verifying key, for the given program ID and function name.
    #[inline]
    pub fn remove_verifying_key(&self, program_id: &ProgramID<N>, function_name: &Identifier<N>) -> Result<()> {
        self.get_stack(program_id)?.remove_verifying_key(function_name);
        Ok(())
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
    use snarkvm_ledger_block::Transition;
    use snarkvm_ledger_query::Query;
    use snarkvm_ledger_store::{BlockStore, helpers::memory::BlockMemory};
    use snarkvm_synthesizer_program::Program;

    use aleo_std::StorageMode;
    use std::sync::OnceLock;

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
        let block_store = BlockStore::<CurrentNetwork, BlockMemory<_>>::open(StorageMode::new_test(None)).unwrap();

        // Prepare the assignments from the block store.
        trace.prepare(&snarkvm_ledger_query::Query::from(block_store)).unwrap();

        // Get the locator.
        let locator = format!("{:?}:{function_name:?}", program.id());

        // Return the execution object.
        trace.prove_execution::<CurrentAleo, _>(&locator, VarunaVersion::V1, rng).unwrap()
    }

    pub fn sample_key() -> (Identifier<CurrentNetwork>, ProvingKey<CurrentNetwork>, VerifyingKey<CurrentNetwork>) {
        static INSTANCE: OnceLock<(
            Identifier<CurrentNetwork>,
            ProvingKey<CurrentNetwork>,
            VerifyingKey<CurrentNetwork>,
        )> = OnceLock::new();
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
        static INSTANCE: OnceLock<Execution<CurrentNetwork>> = OnceLock::new();
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
                let block_store =
                    BlockStore::<CurrentNetwork, BlockMemory<_>>::open(StorageMode::new_test(None)).unwrap();

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
                trace.prepare(&Query::from(block_store)).unwrap();
                // Compute the execution.
                trace.prove_execution::<CurrentAleo, _>("testing", VarunaVersion::V1, rng).unwrap()
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
        let mut process = Process::load().unwrap();
        // Add the program to the process.
        process.add_program(program).unwrap();
        // Return the process.
        process
    }
}
