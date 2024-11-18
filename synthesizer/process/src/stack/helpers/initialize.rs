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

use super::*;

impl<N: Network> Stack<N> {
    /// Initializes a new stack, given the process and program.
    #[inline]
    pub(crate) fn initialize(process: &Process<N>, program: &Program<N>) -> Result<Self> {
        // Construct the stack for the program.
        let mut stack = Self {
            program: program.clone(),
            external_stacks: Default::default(),
            register_types: Default::default(),
            finalize_types: Default::default(),
            universal_srs: process.universal_srs().clone(),
            proving_keys: Default::default(),
            verifying_keys: Default::default(),
            number_of_calls: Default::default(),
            finalize_costs: Default::default(),
            program_depth: 0,
        };

        // Add all the imports into the stack.
        for import in program.imports().keys() {
            // Ensure the program imports all exist in the process already.
            if !process.contains_program(import) {
                bail!("Cannot add program '{}' because its import '{import}' must be added first", program.id())
            }
            // Retrieve the external stack for the import program ID.
            let external_stack = process.get_stack(import)?;
            // Add the external stack to the stack.
            stack.insert_external_stack(external_stack.clone())?;
            // Update the program depth, checking that it does not exceed the maximum call depth.
            stack.program_depth = std::cmp::max(stack.program_depth, external_stack.program_depth() + 1);
            ensure!(
                stack.program_depth <= N::MAX_PROGRAM_DEPTH,
                "Program depth exceeds the maximum allowed call depth"
            );
        }
        // Add the program closures to the stack.
        for closure in program.closures().values() {
            // Add the closure to the stack.
            stack.insert_closure(closure)?;
        }

        // Add the program functions to the stack.
        for function in program.functions().values() {
            // Add the function to the stack.
            stack.insert_function(function)?;
            // Determine the number of calls for the function.
            let mut num_calls = 1;
            for instruction in function.instructions() {
                if let Instruction::Call(call) = instruction {
                    // Determine if this is a function call.
                    if call.is_function_call(&stack)? {
                        // Increment by the number of calls.
                        num_calls += match call.operator() {
                            CallOperator::Locator(locator) => stack
                                .get_external_stack(locator.program_id())?
                                .get_number_of_calls(locator.resource())?,
                            CallOperator::Resource(resource) => stack.get_number_of_calls(resource)?,
                        };
                    }
                }
            }
            // Check that the number of calls does not exceed the maximum.
            // Note that one transition is reserved for the fee.
            ensure!(
                num_calls < ledger_block::Transaction::<N>::MAX_TRANSITIONS,
                "Number of calls exceeds the maximum allowed number of transitions"
            );
            // Add the number of calls to the stack.
            stack.number_of_calls.insert(*function.name(), num_calls);

            // Get the finalize cost.
            let finalize_cost = cost_in_microcredits_v2(&stack, function.name())?;
            // Check that the finalize cost does not exceed the maximum.
            ensure!(
                finalize_cost <= N::TRANSACTION_SPEND_LIMIT,
                "Finalize block '{}' has a cost '{finalize_cost}' which exceeds the transaction spend limit '{}'",
                function.name(),
                N::TRANSACTION_SPEND_LIMIT
            );
            stack.finalize_costs.insert(*function.name(), finalize_cost);
        }

        // Return the stack.
        Ok(stack)
    }
}

impl<N: Network> Stack<N> {
    /// Inserts the given external stack to the stack.
    #[inline]
    fn insert_external_stack(&mut self, external_stack: Arc<Stack<N>>) -> Result<()> {
        // Retrieve the program ID.
        let program_id = *external_stack.program_id();
        // Ensure the external stack is not already added.
        ensure!(!self.external_stacks.contains_key(&program_id), "Program '{program_id}' already exists");
        // Ensure the program exists in the main program imports.
        ensure!(self.program.contains_import(&program_id), "'{program_id}' does not exist in the main program imports");
        // Ensure the external stack is not for the main program.
        ensure!(self.program.id() != external_stack.program_id(), "External stack program cannot be the main program");
        // Add the external stack to the stack.
        self.external_stacks.insert(program_id, external_stack);
        // Return success.
        Ok(())
    }

    /// Inserts the given closure to the stack.
    #[inline]
    fn insert_closure(&mut self, closure: &Closure<N>) -> Result<()> {
        // Retrieve the closure name.
        let name = closure.name();
        // Ensure the closure name is not already added.
        ensure!(!self.register_types.contains_key(name), "Closure '{name}' already exists");

        // Compute the register types.
        let register_types = RegisterTypes::from_closure(self, closure)?;
        // Add the closure name and register types to the stack.
        self.register_types.insert(*name, register_types);
        // Return success.
        Ok(())
    }

    /// Adds the given function name and register types to the stack.
    #[inline]
    fn insert_function(&mut self, function: &Function<N>) -> Result<()> {
        // Retrieve the function name.
        let name = function.name();
        // Ensure the function name is not already added.
        ensure!(!self.register_types.contains_key(name), "Function '{name}' already exists");

        // Compute the register types.
        let register_types = RegisterTypes::from_function(self, function)?;
        // Add the function name and register types to the stack.
        self.register_types.insert(*name, register_types);

        // If the function contains a finalize, insert it.
        if let Some(finalize) = function.finalize_logic() {
            // Compute the finalize types.
            let finalize_types = FinalizeTypes::from_finalize(self, finalize)?;
            // Add the finalize name and finalize types to the stack.
            self.finalize_types.insert(*name, finalize_types);
        }
        // Return success.
        Ok(())
    }
}
