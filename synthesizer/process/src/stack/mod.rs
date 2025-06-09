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

mod authorization;
pub use authorization::*;

mod call;
pub use call::*;

mod finalize_registers;
pub use finalize_registers::*;

mod finalize_types;
pub use finalize_types::*;

mod register_types;
pub use register_types::*;

mod registers;
pub use registers::*;

mod authorize;
mod deploy;
mod evaluate;
mod execute;
mod helpers;

use crate::{CallMetrics, Process, Trace, constructor_cost_in_microcredits, cost_in_microcredits_v2, traits::*};
use console::{
    account::{Address, PrivateKey},
    network::prelude::*,
    program::{
        Argument,
        Entry,
        EntryType,
        FinalizeType,
        Future,
        Identifier,
        Literal,
        Locator,
        Owner as RecordOwner,
        Plaintext,
        PlaintextType,
        ProgramID,
        Record,
        RecordType,
        RegisterType,
        Request,
        Response,
        Value,
        ValueType,
    },
    types::{Field, Group, U8, U16},
};
use ledger_block::{Deployment, Transaction, Transition};
use synthesizer_program::{CallOperator, Closure, Constructor, Function, Instruction, Operand, Program, traits::*};
use synthesizer_snark::{Certificate, ProvingKey, UniversalSRS, VerifyingKey};

use aleo_std::prelude::{finish, lap, timer};
use indexmap::IndexMap;
#[cfg(feature = "locktick")]
use locktick::parking_lot::RwLock;
#[cfg(not(feature = "locktick"))]
use parking_lot::RwLock;
use std::sync::{Arc, Weak};

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

pub type Assignments<N> = Arc<RwLock<Vec<(circuit::Assignment<<N as Environment>::Field>, CallMetrics<N>)>>>;

/// The call stack is used to track the current state of the program execution.
#[derive(Clone)]
pub enum CallStack<N: Network> {
    /// The `Authorize` call stack allows a client to authorize an `Execute` transaction.
    Authorize(Vec<Request<N>>, PrivateKey<N>, Authorization<N>),
    /// The `Synthesize` call stack allows a client to synthesize a function circuit before a `Deploy` transaction.
    Synthesize(Vec<Request<N>>, PrivateKey<N>, Authorization<N>),
    /// The `CheckDeployment` call stack allows a validator to validate a `Deploy` transaction's function circuit.
    CheckDeployment(Vec<Request<N>>, PrivateKey<N>, Assignments<N>, Option<u64>, Option<u64>),
    /// The `Evaluate` call stack allows a client to evaluate a function.
    Evaluate(Authorization<N>),
    /// The `Execute` call stack allows a client to prove function calls in an `Execute` transaction.
    Execute(Authorization<N>, Arc<RwLock<Trace<N>>>),
    /// The `PackageRun` call stack is deprecated.
    PackageRun(Vec<Request<N>>, PrivateKey<N>, Assignments<N>),
}

impl<N: Network> CallStack<N> {
    /// Initializes a call stack as `Self::Evaluate`.
    pub fn evaluate(authorization: Authorization<N>) -> Result<Self> {
        Ok(CallStack::Evaluate(authorization))
    }

    /// Initializes a call stack as `Self::Execute`.
    pub fn execute(authorization: Authorization<N>, trace: Arc<RwLock<Trace<N>>>) -> Result<Self> {
        Ok(CallStack::Execute(authorization, trace))
    }
}

impl<N: Network> CallStack<N> {
    /// Returns a new and independent replica of the call stack.
    pub fn replicate(&self) -> Self {
        match self {
            CallStack::Authorize(requests, private_key, authorization) => {
                CallStack::Authorize(requests.clone(), *private_key, authorization.replicate())
            }
            CallStack::Synthesize(requests, private_key, authorization) => {
                CallStack::Synthesize(requests.clone(), *private_key, authorization.replicate())
            }
            CallStack::CheckDeployment(requests, private_key, assignments, constraint_limit, variable_limit) => {
                CallStack::CheckDeployment(
                    requests.clone(),
                    *private_key,
                    Arc::new(RwLock::new(assignments.read().clone())),
                    *constraint_limit,
                    *variable_limit,
                )
            }
            CallStack::Evaluate(authorization) => CallStack::Evaluate(authorization.replicate()),
            CallStack::Execute(authorization, trace) => {
                CallStack::Execute(authorization.replicate(), Arc::new(RwLock::new(trace.read().clone())))
            }
            CallStack::PackageRun(requests, private_key, assignments) => {
                CallStack::PackageRun(requests.clone(), *private_key, Arc::new(RwLock::new(assignments.read().clone())))
            }
        }
    }

    /// Pushes the request to the stack.
    pub fn push(&mut self, request: Request<N>) -> Result<()> {
        match self {
            CallStack::Authorize(requests, ..)
            | CallStack::Synthesize(requests, ..)
            | CallStack::CheckDeployment(requests, ..)
            | CallStack::PackageRun(requests, ..) => {
                // Check that the number of requests does not exceed the maximum.
                ensure!(
                    requests.len() < Transaction::<N>::MAX_TRANSITIONS,
                    "The number of requests in the authorization must be less than '{}'.",
                    Transaction::<N>::MAX_TRANSITIONS
                );
                // Push the request to the stack.
                requests.push(request)
            }
            CallStack::Evaluate(authorization) => authorization.push(request)?,
            CallStack::Execute(authorization, ..) => authorization.push(request)?,
        }
        Ok(())
    }

    /// Pops the request from the stack.
    pub fn pop(&mut self) -> Result<Request<N>> {
        match self {
            CallStack::Authorize(requests, ..)
            | CallStack::Synthesize(requests, ..)
            | CallStack::CheckDeployment(requests, ..)
            | CallStack::PackageRun(requests, ..) => {
                requests.pop().ok_or_else(|| anyhow!("No more requests on the stack"))
            }
            CallStack::Evaluate(authorization) => authorization.next(),
            CallStack::Execute(authorization, ..) => authorization.next(),
        }
    }

    /// Peeks at the next request from the stack.
    pub fn peek(&mut self) -> Result<Request<N>> {
        match self {
            CallStack::Authorize(requests, ..)
            | CallStack::Synthesize(requests, ..)
            | CallStack::CheckDeployment(requests, ..)
            | CallStack::PackageRun(requests, ..) => {
                requests.last().cloned().ok_or_else(|| anyhow!("No more requests on the stack"))
            }
            CallStack::Evaluate(authorization) => authorization.peek_next(),
            CallStack::Execute(authorization, ..) => authorization.peek_next(),
        }
    }
}

#[derive(Clone)]
pub struct Stack<N: Network> {
    /// The program (record types, structs, functions).
    program: Program<N>,
    /// A reference to the global stack map.
    stacks: Weak<RwLock<IndexMap<ProgramID<N>, Arc<Stack<N>>>>>,
    /// The register types for the program constructor, if it exists.
    constructor_types: Option<FinalizeTypes<N>>,
    /// The mapping of closure and function names to their register types.
    register_types: IndexMap<Identifier<N>, RegisterTypes<N>>,
    /// The mapping of finalize names to their register types.
    finalize_types: IndexMap<Identifier<N>, FinalizeTypes<N>>,
    /// The universal SRS.
    universal_srs: UniversalSRS<N>,
    /// The mapping of function name to proving key.
    proving_keys: Arc<RwLock<IndexMap<Identifier<N>, ProvingKey<N>>>>,
    /// The mapping of function name to verifying key.
    verifying_keys: Arc<RwLock<IndexMap<Identifier<N>, VerifyingKey<N>>>>,
    /// The program address.
    program_address: Address<N>,
    /// The program checksum.
    program_checksum: [U8<N>; 32],
    /// The mapping of function names to their call graph checksum.
    /// These are required from ConsensusVersion::V8 onwards to avoid malleability due to program upgrades.
    call_graph_checksums: Option<IndexMap<Identifier<N>, Field<N>>>,
    /// The program edition.
    program_edition: U16<N>,
}

impl<N: Network> Stack<N> {
    /// Initializes a new stack, if it does not already exist, given the process and the program.
    #[inline]
    pub fn new(process: &Process<N>, program: &Program<N>) -> Result<Self> {
        // Retrieve the program ID.
        let program_id = program.id();
        // Ensure the program contains functions.
        ensure!(!program.functions().is_empty(), "No functions present in the deployment for program '{program_id}'");
        // If the program exists in the process, check that the upgrade is valid.
        if process.contains_program(program_id) {
            Self::check_upgrade_is_valid(process, program)?;
        }

        // Serialize the program into bytes.
        let program_bytes = program.to_bytes_le()?;
        // Ensure the program deserializes from bytes correctly.
        ensure!(program == &Program::from_bytes_le(&program_bytes)?, "Program byte serialization failed");

        // Serialize the program into string.
        let program_string = program.to_string();
        // Ensure the program deserializes from a string correctly.
        ensure!(program == &Program::from_str(&program_string)?, "Program string serialization failed");

        // Return the stack.
        Stack::initialize(process, program)
    }
}

impl<N: Network> StackKeys<N> for Stack<N> {
    /// Returns `true` if the proving key for the given function name exists.
    #[inline]
    fn contains_proving_key(&self, function_name: &Identifier<N>) -> bool {
        self.proving_keys.read().contains_key(function_name)
    }

    /// Returns the proving key for the given function name.
    #[inline]
    fn get_proving_key(&self, function_name: &Identifier<N>) -> Result<ProvingKey<N>> {
        // If the program is 'credits.aleo', try to load the proving key, if it does not exist.
        self.try_insert_credits_function_proving_key(function_name)?;
        // Return the proving key, if it exists.
        match self.proving_keys.read().get(function_name) {
            Some(pk) => Ok(pk.clone()),
            None => bail!("Proving key not found for: {}/{}", self.program.id(), function_name),
        }
    }

    /// Inserts the given proving key for the given function name.
    #[inline]
    fn insert_proving_key(&self, function_name: &Identifier<N>, proving_key: ProvingKey<N>) -> Result<()> {
        // Ensure the function name exists in the program.
        ensure!(
            self.program.contains_function(function_name),
            "Function '{function_name}' does not exist in program '{}'.",
            self.program.id()
        );
        // Insert the proving key.
        self.proving_keys.write().insert(*function_name, proving_key);
        Ok(())
    }

    /// Removes the proving key for the given function name.
    #[inline]
    fn remove_proving_key(&self, function_name: &Identifier<N>) {
        self.proving_keys.write().shift_remove(function_name);
    }

    /// Returns `true` if the verifying key for the given function name exists.
    #[inline]
    fn contains_verifying_key(&self, function_name: &Identifier<N>) -> bool {
        self.verifying_keys.read().contains_key(function_name)
    }

    /// Returns the verifying key for the given function name.
    #[inline]
    fn get_verifying_key(&self, function_name: &Identifier<N>) -> Result<VerifyingKey<N>> {
        // Return the verifying key, if it exists.
        match self.verifying_keys.read().get(function_name) {
            Some(vk) => Ok(vk.clone()),
            None => bail!("Verifying key not found for: {}/{}", self.program.id(), function_name),
        }
    }

    /// Inserts the given verifying key for the given function name.
    #[inline]
    fn insert_verifying_key(&self, function_name: &Identifier<N>, verifying_key: VerifyingKey<N>) -> Result<()> {
        // Ensure the function name exists in the program.
        ensure!(
            self.program.contains_function(function_name),
            "Function '{function_name}' does not exist in program '{}'.",
            self.program.id()
        );
        // Insert the verifying key.
        self.verifying_keys.write().insert(*function_name, verifying_key);
        Ok(())
    }

    /// Removes the verifying key for the given function name.
    #[inline]
    fn remove_verifying_key(&self, function_name: &Identifier<N>) {
        self.verifying_keys.write().shift_remove(function_name);
    }
}

impl<N: Network> StackProgram<N> for Stack<N> {
    /// Returns the program.
    #[inline]
    fn program(&self) -> &Program<N> {
        &self.program
    }

    /// Returns the program ID.
    #[inline]
    fn program_id(&self) -> &ProgramID<N> {
        self.program.id()
    }

    /// Returns the program address.
    #[inline]
    fn program_address(&self) -> &Address<N> {
        &self.program_address
    }

    /// Returns the program checksum.
    #[inline]
    fn program_checksum(&self) -> &[U8<N>; 32] {
        &self.program_checksum
    }

    /// Returns the call graph checksum for a function.
    /// The call graph checksum must exist from ConsensusVersion::V8 onwards.
    #[inline]
    fn call_graph_checksum(&self, function_name: &Identifier<N>) -> Result<Option<Field<N>>> {
        if let Some(call_graph_checksums) = &self.call_graph_checksums {
            Ok(Some(
                *call_graph_checksums
                    .get(function_name)
                    .ok_or(anyhow!("Call graph checksum missing for '{}'", function_name))?,
            ))
        } else {
            Ok(None)
        }
    }

    /// Returns the program edition.
    #[inline]
    fn program_edition(&self) -> &U16<N> {
        &self.program_edition
    }

    /// Returns the external stack for the given program ID.
    ///
    /// Attention - this function is used to check the existence of the external program.
    /// Developers should explicitly handle the error case so as to not default to the main program.
    #[inline]
    fn get_external_stack(&self, program_id: &ProgramID<N>) -> Result<Arc<Stack<N>>> {
        // Check that the program ID is not itself.
        ensure!(
            program_id != self.program.id(),
            "Attempted to get the main program '{program_id}' as an external program."
        );
        // Check that the program ID is imported by the program.
        ensure!(self.program.contains_import(program_id), "External program '{program_id}' is not imported.");
        // Upgrade the weak reference to the process-level stack map and retrieve the external stack.
        self.stacks
            .upgrade()
            .ok_or_else(|| anyhow!("Process-level stack map does not exist"))?
            .read()
            .get(program_id)
            .cloned()
            .ok_or_else(|| anyhow!("External stack for '{program_id}' does not exist"))
    }

    /// Returns the function with the given function name.
    #[inline]
    fn get_function(&self, function_name: &Identifier<N>) -> Result<Function<N>> {
        self.program.get_function(function_name)
    }

    /// Returns a reference to the function with the given function name.
    #[inline]
    fn get_function_ref(&self, function_name: &Identifier<N>) -> Result<&Function<N>> {
        self.program.get_function_ref(function_name)
    }

    /// Returns the expected number of calls for the given function name.
    #[inline]
    fn get_number_of_calls(&self, function_name: &Identifier<N>) -> Result<usize> {
        // Initialize the base number of calls.
        let mut num_calls = 1;
        // Initialize a queue of functions to check.
        let mut queue = vec![(StackRef::Internal(self), *function_name)];
        // Iterate over the queue.
        while let Some((stack_ref, function_name)) = queue.pop() {
            // Ensure that the number of calls does not exceed the maximum.
            // Note that one transition is reserved for the fee.
            ensure!(
                num_calls < Transaction::<N>::MAX_TRANSITIONS,
                "Number of calls must be less than '{}'",
                Transaction::<N>::MAX_TRANSITIONS
            );
            // Determine the number of calls for the function.
            for instruction in stack_ref.get_function_ref(&function_name)?.instructions() {
                if let Instruction::Call(call) = instruction {
                    // Determine if this is a function call.
                    if call.is_function_call(&*stack_ref)? {
                        // Increment by the number of calls.
                        num_calls += 1;
                        // Add the function to the queue.
                        match call.operator() {
                            CallOperator::Locator(locator) => {
                                queue.push((
                                    StackRef::External(stack_ref.get_external_stack(locator.program_id())?),
                                    *locator.resource(),
                                ));
                            }
                            CallOperator::Resource(resource) => {
                                queue.push((stack_ref.clone(), *resource));
                            }
                        }
                    }
                }
            }
        }
        // Return the number of calls.
        Ok(num_calls)
    }

    /// Returns the call graph checksum for the given function name.
    #[inline]
    fn compute_call_graph_checksum(&self, function_name: &Identifier<N>) -> Result<Field<N>> {
        // Initialize a queue of functions to check.
        let mut queue = vec![(StackRef::Internal(self), *function_name)];
        // Initialize a container for the program checksums.
        let mut program_checksums = Vec::new();

        // Iterate over the queue.
        while let Some((stack_ref, function_name)) = queue.pop() {
            // Save the program checksum for the current stack reference.
            program_checksums.push(*stack_ref.program_checksum());

            // Determine the Stacks of calls for the function.
            for instruction in stack_ref.get_function_ref(&function_name)?.instructions() {
                if let Instruction::Call(call) = instruction {
                    // Determine if this is a function call.
                    if call.is_function_call(&*stack_ref)? {
                        // Add the function to the queue.
                        match call.operator() {
                            CallOperator::Locator(locator) => {
                                queue.push((
                                    StackRef::External(stack_ref.get_external_stack(locator.program_id())?),
                                    *locator.resource(),
                                ));
                            }
                            CallOperator::Resource(resource) => {
                                queue.push((stack_ref.clone(), *resource));
                            }
                        }
                    }
                }
            }
        }

        // Collect the program checksums as bits.
        let program_checksum_bits = program_checksums
            .into_iter()
            .flat_map(|program_checksum| program_checksum.into_iter().flat_map(|b| b.to_bits_le())) //*b.deref()))
            .collect::<Vec<bool>>();
        // Hash the program checksums to produce a call graph hash.
        let call_graph_hash = N::hash_sha3_256(&program_checksum_bits)?;
        // Convert the hash of call graph checksums to a field element.
        let num_bits = Field::<N>::size_in_data_bits();
        let call_graph_checksum = Field::<N>::from_bits_le(&call_graph_hash[..num_bits])?;
        // Return the call graph checksum.
        Ok(call_graph_checksum)
    }

    /// Returns a value for the given value type.
    fn sample_value<R: Rng + CryptoRng>(
        &self,
        burner_address: &Address<N>,
        value_type: &ValueType<N>,
        rng: &mut R,
    ) -> Result<Value<N>> {
        match value_type {
            ValueType::Constant(plaintext_type)
            | ValueType::Public(plaintext_type)
            | ValueType::Private(plaintext_type) => Ok(Value::Plaintext(self.sample_plaintext(plaintext_type, rng)?)),
            ValueType::Record(record_name) => {
                Ok(Value::Record(self.sample_record(burner_address, record_name, Group::rand(rng), rng)?))
            }
            ValueType::ExternalRecord(locator) => {
                // Retrieve the external stack.
                let stack = self.get_external_stack(locator.program_id())?;
                // Sample the output.
                Ok(Value::Record(stack.sample_record(burner_address, locator.resource(), Group::rand(rng), rng)?))
            }
            ValueType::Future(locator) => Ok(Value::Future(self.sample_future(locator, rng)?)),
        }
    }

    /// Returns a record for the given record name, with the given burner address and nonce.
    fn sample_record<R: Rng + CryptoRng>(
        &self,
        burner_address: &Address<N>,
        record_name: &Identifier<N>,
        nonce: Group<N>,
        rng: &mut R,
    ) -> Result<Record<N, Plaintext<N>>> {
        // Sample a record.
        let record = self.sample_record_internal(burner_address, record_name, nonce, 0, rng)?;
        // Ensure the record matches the value type.
        self.matches_record(&record, record_name)?;
        // Return the record.
        Ok(record)
    }

    /// Returns a record for the given record name, deriving the nonce from tvk and index.
    fn sample_record_using_tvk<R: Rng + CryptoRng>(
        &self,
        burner_address: &Address<N>,
        record_name: &Identifier<N>,
        tvk: Field<N>,
        index: Field<N>,
        rng: &mut R,
    ) -> Result<Record<N, Plaintext<N>>> {
        // Compute the randomizer.
        let randomizer = N::hash_to_scalar_psd2(&[tvk, index])?;
        // Construct the record nonce from that randomizer.
        let record_nonce = N::g_scalar_multiply(&randomizer);
        // Sample the record with that nonce.
        self.sample_record(burner_address, record_name, record_nonce, rng)
    }
}

impl<N: Network> StackProgramTypes<N> for Stack<N> {
    /// Returns the constructor types for the program.
    #[inline]
    fn get_constructor_types(&self) -> Result<&FinalizeTypes<N>> {
        self.constructor_types.as_ref().ok_or_else(|| anyhow!("Constructor types do not exist"))
    }

    /// Returns the register types for the given closure or function name.
    #[inline]
    fn get_register_types(&self, name: &Identifier<N>) -> Result<&RegisterTypes<N>> {
        // Retrieve the register types.
        self.register_types.get(name).ok_or_else(|| anyhow!("Register types for '{name}' do not exist"))
    }

    /// Returns the register types for the given finalize name.
    #[inline]
    fn get_finalize_types(&self, name: &Identifier<N>) -> Result<&FinalizeTypes<N>> {
        // Retrieve the finalize types.
        self.finalize_types.get(name).ok_or_else(|| anyhow!("Finalize types for '{name}' do not exist"))
    }
}

impl<N: Network> Stack<N> {
    /// Inserts the proving key if the program ID is 'credits.aleo'.
    fn try_insert_credits_function_proving_key(&self, function_name: &Identifier<N>) -> Result<()> {
        // If the program is 'credits.aleo' and it does not exist yet, load the proving key directly.
        if self.program_id() == &ProgramID::from_str("credits.aleo")?
            && !self.proving_keys.read().contains_key(function_name)
        {
            // Load the 'credits.aleo' function proving key.
            let proving_key = N::get_credits_proving_key(function_name.to_string())?;
            // Insert the 'credits.aleo' function proving key.
            self.insert_proving_key(function_name, ProvingKey::new(proving_key.clone()))?;
        }
        Ok(())
    }
}

impl<N: Network> PartialEq for Stack<N> {
    fn eq(&self, other: &Self) -> bool {
        self.program == other.program
            && self.register_types == other.register_types
            && self.finalize_types == other.finalize_types
    }
}

impl<N: Network> Eq for Stack<N> {}

// A helper enum to avoid cloning stacks.
#[derive(Clone)]
pub(crate) enum StackRef<'a, N: Network> {
    // Self's stack.
    Internal(&'a Stack<N>),
    // An external stack.
    External(Arc<Stack<N>>),
}

impl<N: Network> Deref for StackRef<'_, N> {
    type Target = Stack<N>;

    fn deref(&self) -> &Self::Target {
        match self {
            StackRef::Internal(stack) => stack,
            StackRef::External(stack) => stack,
        }
    }
}
