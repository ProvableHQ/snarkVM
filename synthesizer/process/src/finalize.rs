// Copyright (C) 2019-2023 Aleo Systems Inc.
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
use synthesizer_program::{Await, FinalizeRegistersState, Operand};
use utilities::try_vm_runtime;

use std::collections::HashSet;

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
    ) -> Result<(Stack<N>, Vec<FinalizeOperation<N>>)> {
        let timer = timer!("Process::finalize_deployment");

        // Compute the program stack.
        let stack = Stack::new(self, deployment.program())?;
        lap!(timer, "Compute the stack");

        // Insert the verifying keys.
        for (function_name, (verifying_key, _)) in deployment.verifying_keys() {
            stack.insert_verifying_key(function_name, verifying_key.clone())?;
        }
        lap!(timer, "Insert the verifying keys");

        // Initialize the mappings, and store their finalize operations.
        atomic_batch_scope!(store, {
            // Initialize a list for the finalize operations.
            let mut finalize_operations = Vec::with_capacity(deployment.program().mappings().len());

            /* Finalize the fee. */

            // Retrieve the fee stack.
            let fee_stack = self.get_stack(fee.program_id())?;
            // Finalize the fee transition.
            finalize_operations.extend(finalize_fee_transition(state, store, fee_stack, fee)?);
            lap!(timer, "Finalize transition for '{}/{}'", fee.program_id(), fee.function_name());

            /* Finalize the deployment. */

            // Retrieve the program ID.
            let program_id = deployment.program_id();
            // Iterate over the mappings.
            for mapping in deployment.program().mappings().values() {
                // Initialize the mapping.
                finalize_operations.push(store.initialize_mapping(*program_id, *mapping.name())?);
            }
            finish!(timer, "Initialize the program mappings");

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
    ) -> Result<Vec<FinalizeOperation<N>>> {
        let timer = timer!("Program::finalize_execution");

        // Ensure the execution contains transitions.
        ensure!(!execution.is_empty(), "There are no transitions in the execution");

        // Ensure the number of transitions matches the program function.
        // Retrieve the root transition (without popping it).
        let transition = execution.peek()?;
        // Retrieve the stack.
        let stack = self.get_stack(transition.program_id())?;
        // Ensure the number of calls matches the number of transitions.
        let number_of_calls = stack.get_number_of_calls(transition.function_name())?;
        ensure!(
            number_of_calls == execution.len(),
            "The number of transitions in the execution is incorrect. Expected {number_of_calls}, but found {}",
            execution.len()
        );
        lap!(timer, "Verify the number of transitions");

        // Construct the call graph.
        let call_graph = self.construct_call_graph(execution)?;

        atomic_batch_scope!(store, {
            // Finalize the root transition.
            // Note that this will result in all the remaining transitions being finalized, since the number
            // of calls matches the number of transitions.
            let mut finalize_operations = finalize_transition(state, store, stack, transition, call_graph)?;

            /* Finalize the fee. */

            if let Some(fee) = fee {
                // Retrieve the fee stack.
                let fee_stack = self.get_stack(fee.program_id())?;
                // Finalize the fee transition.
                finalize_operations.extend(finalize_fee_transition(state, store, fee_stack, fee)?);
                lap!(timer, "Finalize transition for '{}/{}'", fee.program_id(), fee.function_name());
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
    ) -> Result<Vec<FinalizeOperation<N>>> {
        let timer = timer!("Program::finalize_fee");

        atomic_batch_scope!(store, {
            // Retrieve the stack.
            let stack = self.get_stack(fee.program_id())?;
            // Finalize the fee transition.
            let result = finalize_fee_transition(state, store, stack, fee);
            finish!(timer, "Finalize transition for '{}/{}'", fee.program_id(), fee.function_name());
            // Return the result.
            result
        })
    }
}

/// Finalizes the given fee transition.
fn finalize_fee_transition<N: Network, P: FinalizeStorage<N>>(
    state: FinalizeGlobalState,
    store: &FinalizeStore<N, P>,
    stack: &Stack<N>,
    fee: &Fee<N>,
) -> Result<Vec<FinalizeOperation<N>>> {
    // Construct the call graph.
    let mut call_graph = HashMap::new();
    // Insert the fee transition.
    call_graph.insert(*fee.transition_id(), Vec::new());

    // Finalize the transition.
    match finalize_transition(state, store, stack, fee, call_graph) {
        // If the evaluation succeeds, return the finalize operations.
        Ok(finalize_operations) => Ok(finalize_operations),
        // If the evaluation fails, bail and return the error.
        Err(error) => bail!("'finalize' failed on '{}/{}' - {error}", fee.program_id(), fee.function_name()),
    }
}

/// Finalizes the given transition.
fn finalize_transition<N: Network, P: FinalizeStorage<N>>(
    state: FinalizeGlobalState,
    store: &FinalizeStore<N, P>,
    stack: &Stack<N>,
    transition: &Transition<N>,
    call_graph: HashMap<N::TransitionID, Vec<N::TransitionID>>,
) -> Result<Vec<FinalizeOperation<N>>> {
    // Retrieve the program ID.
    let program_id = transition.program_id();
    // Retrieve the function name.
    let function_name = transition.function_name();

    #[cfg(debug_assertions)]
    println!("Finalizing transition for {}/{function_name}...", transition.program_id());
    debug_assert_eq!(stack.program_id(), transition.program_id());

    // If the last output of the transition is a future, retrieve and finalize it. Otherwise, there are no operations to finalize.
    let future = match transition.outputs().last().and_then(|output| output.future()) {
        Some(future) => future,
        _ => return Ok(Vec::new()),
    };

    // Check that the program ID and function name of the transition match those in the future.
    ensure!(
        future.program_id() == program_id && future.function_name() == function_name,
        "The program ID and function name of the future do not match the transition"
    );

    // Initialize a list for finalize operations.
    let mut finalize_operations = Vec::new();

    // Initialize a stack of active finalize states.
    let mut states = Vec::new();

    // Initialize the top-level finalize state.
    states.push(initialize_finalize_state(state, future, stack, *transition.id())?);

    // While there are active finalize states, finalize them.
    'outer: while let Some(FinalizeState {
        mut counter,
        finalize,
        mut registers,
        stack,
        mut call_counter,
        mut awaited,
    }) = states.pop()
    {
        // Evaluate the commands.
        while counter < finalize.commands().len() {
            // Retrieve the command.
            let command = &finalize.commands()[counter];
            // Finalize the command.
            match &command {
                Command::BranchEq(branch_eq) => {
                    let result = try_vm_runtime!(|| branch_to(counter, branch_eq, finalize, stack, &registers));
                    match result {
                        Ok(Ok(new_counter)) => {
                            counter = new_counter;
                        }
                        // If the evaluation fails, bail and return the error.
                        Ok(Err(error)) => bail!("'finalize' failed to evaluate command ({command}): {error}"),
                        // If the evaluation fails, bail and return the error.
                        Err(_) => bail!("'finalize' failed to evaluate command ({command})"),
                    }
                }
                Command::BranchNeq(branch_neq) => {
                    let result = try_vm_runtime!(|| branch_to(counter, branch_neq, finalize, stack, &registers));
                    match result {
                        Ok(Ok(new_counter)) => {
                            counter = new_counter;
                        }
                        // If the evaluation fails, bail and return the error.
                        Ok(Err(error)) => bail!("'finalize' failed to evaluate command ({command}): {error}"),
                        // If the evaluation fails, bail and return the error.
                        Err(_) => bail!("'finalize' failed to evaluate command ({command})"),
                    }
                }
                Command::Await(await_) => {
                    // Check that the `await` register's is a locator.
                    if let Register::Access(_, _) = await_.register() {
                        bail!("The 'await' register must be a locator")
                    };
                    // Check that the future has not previously been awaited.
                    ensure!(
                        !awaited.contains(await_.register()),
                        "The future register '{}' has already been awaited",
                        await_.register()
                    );

                    // Get the current transition ID.
                    let transition_id = registers.transition_id();
                    // Get the child transition ID.
                    let child_transition_id = match call_graph.get(transition_id) {
                        Some(transitions) => match transitions.get(call_counter) {
                            Some(transition_id) => *transition_id,
                            None => bail!("Child transition ID not found."),
                        },
                        None => bail!("Transition ID '{transition_id}' not found in call graph"),
                    };

                    // Set up the finalize state for the await.
                    let callee_state =
                        match try_vm_runtime!(|| setup_await(state, await_, stack, &registers, child_transition_id)) {
                            Ok(Ok(callee_state)) => callee_state,
                            // If the evaluation fails, bail and return the error.
                            Ok(Err(error)) => bail!("'finalize' failed to evaluate command ({command}): {error}"),
                            // If the evaluation fails, bail and return the error.
                            Err(_) => bail!("'finalize' failed to evaluate command ({command})"),
                        };

                    // Increment the call counter.
                    call_counter += 1;
                    // Increment the counter.
                    counter += 1;
                    // Add the awaited register to the tracked set.
                    awaited.insert(await_.register().clone());

                    // Aggregate the caller state.
                    let caller_state = FinalizeState { counter, finalize, registers, stack, call_counter, awaited };

                    // Push the caller state onto the stack.
                    states.push(caller_state);
                    // Push the callee state onto the stack.
                    states.push(callee_state);

                    continue 'outer;
                }
                _ => {
                    let result = try_vm_runtime!(|| command.finalize(stack, store, &mut registers));
                    match result {
                        // If the evaluation succeeds with an operation, add it to the list.
                        Ok(Ok(Some(finalize_operation))) => finalize_operations.push(finalize_operation),
                        // If the evaluation succeeds with no operation, continue.
                        Ok(Ok(None)) => {}
                        // If the evaluation fails, bail and return the error.
                        Ok(Err(error)) => bail!("'finalize' failed to evaluate command ({command}): {error}"),
                        // If the evaluation fails, bail and return the error.
                        Err(_) => bail!("'finalize' failed to evaluate command ({command})"),
                    }
                    counter += 1;
                }
            };
        }
        // Check that all future registers have been awaited.
        let mut unawaited = Vec::new();
        for input in finalize.inputs() {
            if matches!(input.finalize_type(), FinalizeType::Future(_)) && !awaited.contains(input.register()) {
                unawaited.push(input.register().clone());
            }
        }
        ensure!(
            unawaited.is_empty(),
            "The following future registers have not been awaited: {}",
            unawaited.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(", ")
        );
    }

    // Return the finalize operations.
    Ok(finalize_operations)
}

// A helper struct to track the execution of a finalize block.
struct FinalizeState<'a, N: Network> {
    // A counter for the index of the commands.
    counter: usize,
    // The finalize logic.
    finalize: &'a Finalize<N>,
    // The registers.
    registers: FinalizeRegisters<N>,
    // The stack.
    stack: &'a Stack<N>,
    // Call counter.
    call_counter: usize,
    // Awaited futures.
    awaited: HashSet<Register<N>>,
}

// A helper function to initialize the finalize state.
fn initialize_finalize_state<'a, N: Network>(
    state: FinalizeGlobalState,
    future: &Future<N>,
    stack: &'a Stack<N>,
    transition_id: N::TransitionID,
) -> Result<FinalizeState<'a, N>> {
    // Get the finalize logic and the stack.
    let (finalize, stack) = match stack.program_id() == future.program_id() {
        true => (stack.get_function_ref(future.function_name())?.finalize_logic(), stack),
        false => {
            let stack = stack.get_external_stack(future.program_id())?.as_ref();
            (stack.get_function_ref(future.function_name())?.finalize_logic(), stack)
        }
    };
    // Check that the finalize logic exists.
    let finalize = match finalize {
        Some(finalize) => finalize,
        None => bail!(
            "The function '{}/{}' does not have an associated finalize block",
            future.program_id(),
            future.function_name()
        ),
    };
    // Initialize the registers.
    let mut registers = FinalizeRegisters::new(
        state,
        transition_id,
        *future.function_name(),
        stack.get_finalize_types(future.function_name())?.clone(),
    );

    // Store the inputs.
    finalize.inputs().iter().map(|i| i.register()).zip_eq(future.arguments().iter()).try_for_each(
        |(register, input)| {
            // Assign the input value to the register.
            registers.store(stack, register, Value::from(input))
        },
    )?;

    Ok(FinalizeState { counter: 0, finalize, registers, stack, call_counter: 0, awaited: Default::default() })
}

// A helper function that sets up the await operation.
#[inline]
fn setup_await<'a, N: Network>(
    state: FinalizeGlobalState,
    await_: &Await<N>,
    stack: &'a Stack<N>,
    registers: &FinalizeRegisters<N>,
    transition_id: N::TransitionID,
) -> Result<FinalizeState<'a, N>> {
    // Retrieve the input as a future.
    let future = match registers.load(stack, &Operand::Register(await_.register().clone()))? {
        Value::Future(future) => future,
        _ => bail!("The input to 'await' is not a future"),
    };
    // Initialize the state.
    initialize_finalize_state(state, &future, stack, transition_id)
}

// A helper function that returns the index to branch to.
#[inline]
fn branch_to<N: Network, const VARIANT: u8>(
    counter: usize,
    branch: &Branch<N, VARIANT>,
    finalize: &Finalize<N>,
    stack: &Stack<N>,
    registers: &FinalizeRegisters<N>,
) -> Result<usize> {
    // Retrieve the inputs.
    let first = registers.load(stack, branch.first())?;
    let second = registers.load(stack, branch.second())?;

    // A helper to get the index corresponding to a position.
    let get_position_index = |position: &Identifier<N>| match finalize.positions().get(position) {
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
    use ledger_store::{
        helpers::memory::{BlockMemory, FinalizeMemory},
        BlockStore,
    };

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
        let block_store = BlockStore::<CurrentNetwork, BlockMemory<_>>::open(None).unwrap();
        // Initialize a new finalize store.
        let finalize_store = FinalizeStore::<_, FinalizeMemory<_>>::open(None).unwrap();

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
