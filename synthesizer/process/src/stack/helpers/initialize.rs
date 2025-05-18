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

use super::*;

impl<N: Network> Stack<N> {
    /// Initializes a new stack, given the process and program.
    #[inline]
    pub(crate) fn initialize(process: &Process<N>, program: &Program<N>) -> Result<Self> {
        // Compute the appropriate edition for the stack.
        let edition = match process.contains_program(program.id()) {
            // If the program does not exist in the process, use edition zero.
            false => 0u16,
            // If the new program matches the existing program, use the existing edition.
            // Otherwise, increment the edition.
            true => {
                // Retrieve the stack for the program.
                let stack = process.get_stack(program.id())?;
                // Retrieve the program edition.
                let mut edition = **stack.program_edition();
                // If the program does not match the existing program, increment the edition.
                if stack.program() != program {
                    edition = edition
                        .checked_add(1)
                        .ok_or_else(|| anyhow!("Overflow while incrementing the program edition"))?;
                }
                // Output the edition
                edition
            }
        };

        // Construct the stack for the program.
        let mut stack = Self {
            program: program.clone(),
            stacks: Arc::downgrade(&process.stacks),
            constructor_types: Default::default(),
            register_types: Default::default(),
            finalize_types: Default::default(),
            universal_srs: process.universal_srs().clone(),
            proving_keys: Default::default(),
            verifying_keys: Default::default(),
            program_address: program.id().to_address()?,
            program_checksum: program.to_checksum(),
            program_edition: U16::new(edition),
            program_owner: None,
        };

        // Add all the imports into the stack.
        for import in program.imports().keys() {
            // Ensure that the program does not import itself.
            ensure!(import != program.id(), "Program cannot import itself");
            // Ensure the program imports all exist in the process already.
            if !process.contains_program(import) {
                bail!("Cannot add program '{}' because its import '{import}' must be added first", program.id())
            }
        }

        // Add the constructor to the stack if it exists.
        if let Some(constructor) = program.constructor() {
            // Add the constructor to the stack.
            stack.insert_constructor(constructor)?;
            // Get the constructor cost.
            let constructor_cost = constructor_cost_in_microcredits(&stack)?;
            // Check that the constructor cost does not exceed the maximum.
            ensure!(
                constructor_cost <= N::TRANSACTION_SPEND_LIMIT,
                "Constructor has a cost '{constructor_cost}' which exceeds the transaction spend limit '{}'",
                N::TRANSACTION_SPEND_LIMIT
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
            // This includes a safety check for the maximum number of calls.
            stack.get_number_of_calls(function.name())?;

            // Get the finalize cost.
            let finalize_cost = cost_in_microcredits_v2(&stack, function.name())?;
            // Check that the finalize cost does not exceed the maximum.
            ensure!(
                finalize_cost <= N::TRANSACTION_SPEND_LIMIT,
                "Finalize block '{}' has a cost '{finalize_cost}' which exceeds the transaction spend limit '{}'",
                function.name(),
                N::TRANSACTION_SPEND_LIMIT
            );
        }

        // Return the stack.
        Ok(stack)
    }
}

impl<N: Network> Stack<N> {
    /// Adds the constructor to the stack.
    #[inline]
    fn insert_constructor(&mut self, constructor: &Constructor<N>) -> Result<()> {
        // Ensure that the constsuctor is not already added.
        ensure!(self.constructor_types.is_none(), "Constructor already exists");

        // Compute the constructor types.
        let constructor_types = FinalizeTypes::from_constructor(self, constructor)?;
        // Add the constructor types to the stack.
        self.constructor_types = Some(constructor_types);
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
