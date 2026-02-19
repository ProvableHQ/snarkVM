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

use super::*;
use console::program::{FinalizeType, Future, Register};
use snarkvm_synthesizer_error::{FinalizeError, IndexedFinalizeError};
use snarkvm_synthesizer_program::{Await, FinalizeRegistersState, Operand, RegistersTrait};
use snarkvm_utilities::try_vm_runtime;

use std::collections::HashSet;

/// A helper macro to bail with an `IndexedFinalizeError` for a given index, command, and anyhow message.
#[macro_export]
macro_rules! indexed_finalize_bail {
    // With locator + index + command
    ($locator:expr, $index:expr, $command:expr, $($arg:tt)*) => {{
        return Err(IndexedFinalizeError::new(
            $locator.to_string(),
            Some(($index, $command.to_string())),
            FinalizeError::Anyhow(anyhow!($($arg)*)),
        ));
    }};
    // With locator only (no command)
    ($locator:expr, $($arg:tt)*) => {{
        return Err(IndexedFinalizeError::new(
            $locator.to_string(),
            None,
            FinalizeError::Anyhow(anyhow!($($arg)*)),
        ));
    }};
}

pub trait IntoIndexedFinalize<T> {
    fn into_indexed(
        self,
        locator: impl Into<String>,
        command: Option<(usize, impl Into<String>)>,
    ) -> Result<T, IndexedFinalizeError>;
}

impl<T> IntoIndexedFinalize<T> for Result<T, Error> {
    fn into_indexed(
        self,
        locator: impl Into<String>,
        command: Option<(usize, impl Into<String>)>,
    ) -> Result<T, IndexedFinalizeError> {
        self.map_err(|e| {
            IndexedFinalizeError::new(
                locator.into(),
                command.map(|(index, cmd)| (index, cmd.into())),
                FinalizeError::Anyhow(e),
            )
        })
    }
}

impl<N: Network> Process<N> {
    /// Finalizes the deployment and fee.
    /// This method assumes the given deployment **is valid**.
    /// This method should **only** be called by `VM::finalize()`.
    #[inline]
    pub fn finalize_deployment<P: FinalizeStorage<N>>(
        &self,
        state: FinalizeGlobalState,
        store: &FinalizeStore<N, P>,
        deployment: &Deployment<N>,
        fee: &Fee<N>,
    ) -> Result<(Stack<N>, Vec<FinalizeOperation<N>>), IndexedFinalizeError> {
        let timer = timer!("Process::finalize_deployment");

        // Fetch the program ID.
        let program_id_str = &deployment.program().id().to_string();

        // Compute the program stack.
        let mut stack = Stack::new(self, deployment.program()).into_indexed(program_id_str, None)?;
        lap!(timer, "Compute the stack");

        // Set the program owner.
        // Note: The program owner is only enforced to be `Some` after `ConsensusVersion::V9`
        // and is `None` for all programs deployed before the `V9` migration.
        stack.set_program_owner(deployment.program_owner());

        // Insert the verifying keys.
        for (function_name, (verifying_key, _)) in deployment.verifying_keys() {
            stack.insert_verifying_key(function_name, verifying_key.clone()).into_indexed(program_id_str, None)?;
        }
        lap!(timer, "Insert the verifying keys");

        // Determine which mappings must be initialized.
        let mappings = match deployment.edition().is_zero() {
            true => deployment.program().mappings().values().collect::<Vec<_>>(),
            false => {
                // Get the existing stack.
                let existing_stack = self.get_stack(deployment.program_id()).into_indexed(program_id_str, None)?;
                // Get the existing mappings.
                let existing_mappings = existing_stack.program().mappings();
                // Determine and return the new mappings
                let mut new_mappings = Vec::new();
                for mapping in deployment.program().mappings().values() {
                    if !existing_mappings.contains_key(mapping.name()) {
                        new_mappings.push(mapping);
                    }
                }
                new_mappings
            }
        };
        lap!(timer, "Retrieve the mappings to initialize");

        // Initialize the mappings, and store their finalize operations.
        atomic_batch_scope!(store, {
            // Initialize a list for the finalize operations.
            let mut finalize_operations = Vec::with_capacity(deployment.program().mappings().len());

            /* Finalize the fee. */

            // Retrieve the fee stack.
            let fee_stack = self.get_stack(fee.program_id()).into_indexed(program_id_str, None)?;
            // Finalize the fee transition.
            finalize_operations.extend(finalize_fee_transition(state, store, &fee_stack, fee)?);
            lap!(timer, "Finalize transition for '{}/{}'", fee.program_id(), fee.function_name());

            /* Finalize the deployment. */

            // Retrieve the program ID.
            let program_id = deployment.program_id();
            // Iterate over the mappings that must be initialized.
            for mapping in mappings {
                // Initialize the mapping.
                finalize_operations
                    .push(store.initialize_mapping(*program_id, *mapping.name()).into_indexed(program_id_str, None)?);
            }
            lap!(timer, "Initialize the program mappings");

            // If the program has a constructor, execute it and extend the finalize operations.
            // This must happen after the mappings are initialized as the constructor may depend on them.
            if deployment.program().contains_constructor() {
                let operations = finalize_constructor(state, store, &stack, N::TransitionID::default())?;
                finalize_operations.extend(operations);
                lap!(timer, "Execute the constructor");
            }

            finish!(timer, "Finished finalizing the deployment");
            // Return the stack and finalize operations.
            Ok((stack, finalize_operations))
        })
    }

    /// Finalizes the execution and fee.
    /// This method assumes the given execution **is valid**.
    /// This method should **only** be called by `VM::finalize()`.
    #[inline]
    pub fn finalize_execution<P: FinalizeStorage<N>>(
        &self,
        state: FinalizeGlobalState,
        store: &FinalizeStore<N, P>,
        execution: &Execution<N>,
        fee: Option<&Fee<N>>,
    ) -> Result<Vec<FinalizeOperation<N>>, IndexedFinalizeError> {
        let timer = timer!("Program::finalize_execution");

        // Ensure the execution contains transitions.
        if execution.is_empty() {
            indexed_finalize_bail!("None", "There are no transitions in the execution");
        }

        // Ensure the number of transitions matches the program function.
        // Retrieve the root transition (without popping it).
        let transition = execution.peek().into_indexed("None", None)?;
        // Fetch the transition locator.
        let transition_locator = &format!("{}/{}", transition.program_id(), transition.function_name());
        // Retrieve the stack.
        let stack = self.get_stack(transition.program_id()).into_indexed(transition_locator, None)?;
        // Ensure the number of calls matches the number of transitions.
        let number_of_calls =
            stack.get_number_of_calls(transition.function_name()).into_indexed(transition_locator, None)?;
        if !number_of_calls == execution.len() {
            indexed_finalize_bail!(
                transition_locator,
                "The number of transitions in the execution is incorrect. Expected {number_of_calls}, but found {}",
                execution.len()
            );
        }
        lap!(timer, "Verify the number of transitions");

        // Construct the call graph.
        let consensus_version = N::CONSENSUS_VERSION(state.block_height()).into_indexed(transition_locator, None)?;
        let call_graph = match (ConsensusVersion::V1..=ConsensusVersion::V2).contains(&consensus_version) {
            true => self.construct_call_graph(execution).into_indexed(transition_locator, None)?,
            // If the height is greater than or equal to `ConsensusVersion::V3`, then provide an empty call graph, as it is no longer used during finalization.
            false => HashMap::new(),
        };

        atomic_batch_scope!(store, {
            // Finalize the root transition.
            // Note that this will result in all the remaining transitions being finalized, since the number
            // of calls matches the number of transitions.
            let mut finalize_operations = finalize_transition(state, store, &stack, transition, call_graph)?;

            /* Finalize the fee. */

            if let Some(fee) = fee {
                // Fetch the fee locator.
                let fee_locator = format!("{}/{}", fee.program_id(), fee.function_name());
                // Retrieve the fee stack.
                let fee_stack = self.get_stack(fee.program_id()).into_indexed(fee_locator, None)?;
                // Finalize the fee transition.
                finalize_operations.extend(finalize_fee_transition(state, store, &fee_stack, fee)?);
                lap!(timer, "Finalize transition for '{fee_locator}'");
            }

            finish!(timer);
            // Return the finalize operations.
            Ok(finalize_operations)
        })
    }

    /// Finalizes the fee.
    /// This method assumes the given fee **is valid**.
    /// This method should **only** be called by `VM::finalize()`.
    #[inline]
    pub fn finalize_fee<P: FinalizeStorage<N>>(
        &self,
        state: FinalizeGlobalState,
        store: &FinalizeStore<N, P>,
        fee: &Fee<N>,
    ) -> Result<Vec<FinalizeOperation<N>>, IndexedFinalizeError> {
        let timer = timer!("Program::finalize_fee");

        let locator = format!("{}/{}", fee.program_id(), fee.function_name());
        atomic_batch_scope!(store, {
            // Retrieve the stack.
            let stack = self.get_stack(fee.program_id()).into_indexed(locator, None)?;
            // Finalize the fee transition.
            let result = finalize_fee_transition(state, store, &stack, fee);
            finish!(timer, "Finalize transition for '{locator}'");
            // Return the result.
            result
        })
    }
}

/// Finalizes the given fee transition.
fn finalize_fee_transition<N: Network, P: FinalizeStorage<N>>(
    state: FinalizeGlobalState,
    store: &FinalizeStore<N, P>,
    stack: &Arc<Stack<N>>,
    fee: &Fee<N>,
) -> Result<Vec<FinalizeOperation<N>>, IndexedFinalizeError> {
    // Fetch the fee locator.
    let locator = &format!("{}/{}", fee.program_id(), fee.function_name());
    // Construct the call graph.
    let consensus_version = N::CONSENSUS_VERSION(state.block_height()).into_indexed(locator, None::<(_, String)>)?;
    let call_graph = match (ConsensusVersion::V1..=ConsensusVersion::V2).contains(&consensus_version) {
        true => HashMap::from([(*fee.transition_id(), Vec::new())]),
        // If the height is greater than or equal to `ConsensusVersion::V3`, then provide an empty call graph, as it is no longer used during finalization.
        false => HashMap::new(),
    };

    // Finalize the transition.
    finalize_transition(state, store, stack, fee, call_graph)
}

/// Finalizes the constructor.
fn finalize_constructor<N: Network, P: FinalizeStorage<N>>(
    state: FinalizeGlobalState,
    store: &FinalizeStore<N, P>,
    stack: &Stack<N>,
    transition_id: N::TransitionID,
) -> Result<Vec<FinalizeOperation<N>>, IndexedFinalizeError> {
    // Retrieve the program ID.
    let program_id = stack.program_id();
    dev_println!("Finalizing constructor for {}...", stack.program_id());

    // Initialize a list for finalize operations.
    let mut finalize_operations = Vec::new();

    // Initialize a nonce for the constructor registers.
    // Currently, this nonce is set to zero for every constructor.
    let nonce = 0;

    // Initialize the locator for the constructor.
    let locator = &format!("{program_id}/constructor");

    // Get the constructor logic. If the program does not have a constructor, return early.
    let Some(constructor) = stack.program().constructor() else {
        return Ok(finalize_operations);
    };

    // Get the constructor types.
    let constructor_types = match stack.get_constructor_types() {
        Ok(types) => types.clone(),
        Err(error) => indexed_finalize_bail!(locator, "Failed to get constructor types - {error}"),
    };

    // Initialize the finalize registers.
    let mut registers = FinalizeRegisters::new(state, transition_id, *program_id.name(), constructor_types, nonce);

    // Initialize a counter for the commands.
    let mut counter = 0;

    // Evaluate the commands.
    while counter < constructor.commands().len() {
        // Retrieve the command.
        let command = &constructor.commands()[counter];
        // Finalize the command.
        match &command {
            Command::Await(_) => {
                indexed_finalize_bail!(locator, counter, command, "Cannot `await` a Future in a constructor")
            }
            _ => finalize_command_except_await(
                locator,
                store,
                stack,
                &mut registers,
                constructor.positions(),
                command,
                &mut counter,
                &mut finalize_operations,
            )?,
        };
    }

    // Return the finalize operations.
    Ok(finalize_operations)
}

/// Finalizes the given transition.
fn finalize_transition<N: Network, P: FinalizeStorage<N>>(
    state: FinalizeGlobalState,
    store: &FinalizeStore<N, P>,
    stack: &Arc<Stack<N>>,
    transition: &Transition<N>,
    call_graph: HashMap<N::TransitionID, Vec<N::TransitionID>>,
) -> Result<Vec<FinalizeOperation<N>>, IndexedFinalizeError> {
    // Retrieve the program ID.
    let program_id = transition.program_id();
    // Retrieve the function name.
    let function_name = transition.function_name();
    // Construct the locator.
    let transition_locator = &format!("{program_id}/{function_name}");

    dev_println!("Finalizing transition for {transition_locator}...");
    debug_assert_eq!(stack.program_id(), transition.program_id());

    // If the last output of the transition is a future, retrieve and finalize it. Otherwise, there are no operations to finalize.
    let future = match transition.outputs().last().and_then(|output| output.future()) {
        Some(future) => future,
        _ => return Ok(Vec::new()),
    };

    // Check that the program ID and function name of the transition match those in the future.
    if future.program_id() != program_id || future.function_name() != function_name {
        indexed_finalize_bail!(
            transition_locator,
            "The program ID and function name of the future do not match the transition"
        );
    }

    // Initialize a list for finalize operations.
    let mut finalize_operations = Vec::new();

    // Initialize a stack of active finalize states.
    let mut states = Vec::new();

    // Initialize a nonce for the finalize registers.
    // Note that this nonce must be unique for each sub-transition being finalized.
    let mut nonce = 0;

    // Initialize the top-level finalize state.
    states.push(
        initialize_finalize_state(state, future, stack, *transition.id(), nonce)
            .into_indexed(transition_locator, None::<(_, String)>)?,
    );

    // While there are active finalize states, finalize them.
    'outer: while let Some(FinalizeState { mut counter, mut registers, stack, mut call_counter, mut awaited }) =
        states.pop()
    {
        // Get the finalize logic.
        let finalize_locator = &format!("{}/{}", stack.program_id(), registers.function_name());
        let Some(finalize) = stack
            .get_function_ref(registers.function_name())
            .into_indexed(finalize_locator, None::<(_, String)>)?
            .finalize_logic()
        else {
            indexed_finalize_bail!(
                finalize_locator,
                "The function '{finalize_locator}' does not have an associated finalize scope",
            )
        };
        // Evaluate the commands.
        while counter < finalize.commands().len() {
            // Retrieve the command.
            let command = &finalize.commands()[counter];
            // Finalize the command.
            match &command {
                Command::Await(await_) => {
                    // Check that the `await` register's is a locator.
                    if let Register::Access(_, _) = await_.register() {
                        indexed_finalize_bail!(finalize_locator, "The 'await' register must be a locator")
                    };
                    // Check that the future has not previously been awaited.
                    if awaited.contains(await_.register()) {
                        indexed_finalize_bail!(
                            finalize_locator,
                            counter,
                            command,
                            "The future register '{}' has already been awaited",
                            await_.register()
                        );
                    }

                    // Get the transition ID used to initialize the finalize registers.
                    // If the block height is greater than or equal to `ConsensusVersion::V3`, then use the top-level transition ID.
                    // Otherwise, query the call graph for the child transition ID corresponding to the future that is being awaited.
                    let consensus_version = N::CONSENSUS_VERSION(state.block_height())
                        .into_indexed(finalize_locator, Some((counter, command.to_string())))?;
                    let transition_id = if (ConsensusVersion::V1..=ConsensusVersion::V2).contains(&consensus_version) {
                        // Get the current transition ID.
                        let transition_id = registers.transition_id();
                        // Get the child transition ID.
                        match call_graph.get(transition_id) {
                            Some(transitions) => match transitions.get(call_counter) {
                                Some(transition_id) => *transition_id,
                                None => indexed_finalize_bail!(
                                    finalize_locator,
                                    counter,
                                    command,
                                    "Child transition ID not found."
                                ),
                            },
                            None => indexed_finalize_bail!(
                                finalize_locator,
                                counter,
                                command,
                                "Transition ID '{transition_id}' not found in call graph"
                            ),
                        }
                    } else {
                        *transition.id()
                    };

                    // Increment the nonce.
                    nonce += 1;

                    // Set up the finalize state for the await.
                    let callee_state = match try_vm_runtime!(|| setup_await(
                        state,
                        await_,
                        &stack,
                        &registers,
                        transition_id,
                        nonce
                    )) {
                        Ok(Ok(callee_state)) => callee_state,
                        // If the evaluation fails, bail and return the error.
                        Ok(Err(error)) => indexed_finalize_bail!(
                            finalize_locator,
                            counter,
                            command,
                            "'finalize' failed to evaluate command ({command}): {error}"
                        ),
                        // If the evaluation fails, bail and return the error.
                        Err(_) => indexed_finalize_bail!(
                            finalize_locator,
                            counter,
                            command,
                            "'finalize' failed to evaluate command ({command})"
                        ),
                    };

                    // Increment the call counter.
                    call_counter += 1;
                    // Increment the counter.
                    counter += 1;
                    // Add the awaited register to the tracked set.
                    awaited.insert(await_.register().clone());

                    // Aggregate the caller state.
                    let caller_state = FinalizeState { counter, registers, stack, call_counter, awaited };

                    // Push the caller state onto the stack.
                    states.push(caller_state);
                    // Push the callee state onto the stack.
                    states.push(callee_state);

                    continue 'outer;
                }
                _ => finalize_command_except_await(
                    &format!("{program_id}/{function_name}"),
                    store,
                    stack.deref(),
                    &mut registers,
                    finalize.positions(),
                    command,
                    &mut counter,
                    &mut finalize_operations,
                )?,
            };
        }
        // Check that all future registers have been awaited.
        let mut unawaited = Vec::new();
        for input in finalize.inputs() {
            if matches!(input.finalize_type(), FinalizeType::Future(_)) && !awaited.contains(input.register()) {
                unawaited.push(input.register().clone());
            }
        }
        if !unawaited.is_empty() {
            indexed_finalize_bail!(
                finalize_locator,
                "The following future registers have not been awaited: {}",
                unawaited.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(", ")
            );
        }
    }

    // Return the finalize operations.
    Ok(finalize_operations)
}

// A helper struct to track the execution of a finalize scope.
struct FinalizeState<N: Network> {
    // A counter for the index of the commands.
    counter: usize,
    // The registers.
    registers: FinalizeRegisters<N>,
    // The stack.
    stack: Arc<Stack<N>>,
    // Call counter.
    call_counter: usize,
    // Awaited futures.
    awaited: HashSet<Register<N>>,
}

// A helper function to initialize the finalize state.
fn initialize_finalize_state<N: Network>(
    state: FinalizeGlobalState,
    future: &Future<N>,
    stack: &Arc<Stack<N>>,
    transition_id: N::TransitionID,
    nonce: u64,
) -> Result<FinalizeState<N>> {
    // Get the stack.
    let stack = match stack.program_id() == future.program_id() {
        true => stack.clone(),
        false => stack.get_external_stack(future.program_id())?,
    };
    // Get the finalize logic and check that it exists.
    let Some(finalize) = stack.get_function_ref(future.function_name())?.finalize_logic() else {
        bail!(
            "The function '{}/{}' does not have an associated finalize scope",
            future.program_id(),
            future.function_name()
        )
    };
    // Initialize the registers.
    let mut registers = FinalizeRegisters::new(
        state,
        transition_id,
        *future.function_name(),
        stack.get_finalize_types(future.function_name())?.clone(),
        nonce,
    );

    // Store the inputs.
    finalize.inputs().iter().map(|i| i.register()).zip_eq(future.arguments().iter()).try_for_each(
        |(register, input)| {
            // Assign the input value to the register.
            registers.store(stack.deref(), register, Value::from(input))
        },
    )?;

    Ok(FinalizeState { counter: 0, registers, stack, call_counter: 0, awaited: Default::default() })
}

// A helper function to finalize all commands except `await`, updating the finalize operations and the counter.
#[inline]
fn finalize_command_except_await<N: Network>(
    locator: &str,
    store: &FinalizeStore<N, impl FinalizeStorage<N>>,
    stack: &impl StackTrait<N>,
    registers: &mut FinalizeRegisters<N>,
    positions: &HashMap<Identifier<N>, usize>,
    command: &Command<N>,
    counter: &mut usize,
    finalize_operations: &mut Vec<FinalizeOperation<N>>,
) -> Result<(), IndexedFinalizeError> {
    // Finalize the command.
    match &command {
        Command::BranchEq(branch_eq) => {
            let result = try_vm_runtime!(|| branch_to(*counter, branch_eq, positions, stack, registers));
            match result {
                Ok(Ok(new_counter)) => {
                    *counter = new_counter;
                }
                // If the evaluation fails, bail and return the error.
                Ok(Err(error)) => indexed_finalize_bail!(
                    locator,
                    *counter,
                    command,
                    "'{locator}' failed to evaluate command ({command}): {error}"
                ),
                // If the evaluation fails, bail and return the error.
                Err(_) => indexed_finalize_bail!(
                    locator,
                    *counter,
                    command,
                    "'{locator}' failed to evaluate command ({command})"
                ),
            }
        }
        Command::BranchNeq(branch_neq) => {
            let result = try_vm_runtime!(|| branch_to(*counter, branch_neq, positions, stack, registers));
            match result {
                Ok(Ok(new_counter)) => {
                    *counter = new_counter;
                }
                // If the evaluation fails, bail and return the error.
                Ok(Err(error)) => indexed_finalize_bail!(
                    locator,
                    *counter,
                    command,
                    "'{locator}' failed to evaluate command ({command}): {error}"
                ),
                // If the evaluation fails, bail and return the error.
                Err(_) => indexed_finalize_bail!(
                    locator,
                    *counter,
                    command,
                    "'{locator}' failed to evaluate command ({command})"
                ),
            }
        }
        Command::Await(_) => {
            indexed_finalize_bail!(
                locator,
                *counter,
                command,
                "Cannot use `finalize_command_except_await` with an 'await' command"
            )
        }
        _ => {
            let result = try_vm_runtime!(|| command.finalize(stack, store, registers));
            match result {
                // If the evaluation succeeds with an operation, add it to the list.
                Ok(Ok(Some(finalize_operation))) => finalize_operations.push(finalize_operation),
                // If the evaluation succeeds with no operation, continue.
                Ok(Ok(None)) => {}
                // If the evaluation fails, bail and return the error.
                Ok(Err(error)) => {
                    return Err(IndexedFinalizeError::new(
                        locator.to_string(),
                        Some((*counter, command.to_string())),
                        error,
                    ));
                }
                // If the evaluation fails, bail and return the error.
                Err(_) => indexed_finalize_bail!(
                    locator,
                    *counter,
                    command,
                    "'{locator}' failed to evaluate command ({command})"
                ),
            }
            *counter += 1;
        }
    };
    Ok(())
}

// A helper function that sets up the await operation.
#[inline]
fn setup_await<N: Network>(
    state: FinalizeGlobalState,
    await_: &Await<N>,
    stack: &Arc<Stack<N>>,
    registers: &FinalizeRegisters<N>,
    transition_id: N::TransitionID,
    nonce: u64,
) -> Result<FinalizeState<N>> {
    // Retrieve the input as a future.
    let future = match registers.load(stack.deref(), &Operand::Register(await_.register().clone()))? {
        Value::Future(future) => future,
        _ => bail!("The input to 'await' is not a future"),
    };
    // Initialize the state.
    initialize_finalize_state(state, &future, stack, transition_id, nonce)
}

// A helper function that returns the index to branch to.
fn branch_to<N: Network, const VARIANT: u8>(
    counter: usize,
    branch: &Branch<N, VARIANT>,
    positions: &HashMap<Identifier<N>, usize>,
    stack: &impl StackTrait<N>,
    registers: &impl RegistersTrait<N>,
) -> Result<usize> {
    // Retrieve the inputs.
    let first = registers.load(stack, branch.first())?;
    let second = registers.load(stack, branch.second())?;

    // A helper to get the index corresponding to a position.
    let get_position_index = |position: &Identifier<N>| match positions.get(position) {
        Some(index) if *index > counter => Ok(*index),
        Some(_) => bail!("Cannot branch to an earlier position '{position}' in the program"),
        None => bail!("The position '{position}' does not exist."),
    };

    // Compare the operands and determine the index to branch to.
    match VARIANT {
        // The `branch.eq` variant.
        0 if first == second => get_position_index(branch.position()),
        0 if first != second => Ok(counter + 1),
        // The `branch.neq` variant.
        1 if first == second => Ok(counter + 1),
        1 if first != second => get_position_index(branch.position()),
        _ => bail!("Invalid 'branch' variant: {VARIANT}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_execute::{sample_fee, sample_finalize_state};
    use console::prelude::TestRng;
    use snarkvm_ledger_store::{
        BlockStore,
        helpers::memory::{BlockMemory, FinalizeMemory},
    };

    use aleo_std::StorageMode;

    type CurrentNetwork = console::network::MainnetV0;
    type CurrentAleo = circuit::network::AleoV0;

    #[test]
    fn test_finalize_deployment() {
        let rng = &mut TestRng::default();

        // Initialize a new program.
        let program = Program::<CurrentNetwork>::from_str(
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
        .unwrap();

        // Initialize a new process.
        let mut process = Process::load().unwrap();
        // Deploy the program.
        let deployment = process.deploy::<CurrentAleo, _>(&program, rng).unwrap();

        // Initialize a new block store.
        let block_store = BlockStore::<CurrentNetwork, BlockMemory<_>>::open(StorageMode::new_test(None)).unwrap();
        // Initialize a new finalize store.
        let finalize_store = FinalizeStore::<_, FinalizeMemory<_>>::open(StorageMode::new_test(None)).unwrap();

        // Ensure the program does not exist.
        assert!(!process.contains_program(program.id()));

        // Compute the fee.
        let fee = sample_fee::<_, CurrentAleo, _, _>(&process, &block_store, &finalize_store, rng);
        // Finalize the deployment.
        let (stack, _) =
            process.finalize_deployment(sample_finalize_state(1), &finalize_store, &deployment, &fee).unwrap();
        // Add the stack *manually* to the process.
        process.add_stack(stack);

        // Ensure the program exists.
        assert!(process.contains_program(program.id()));
    }
}
