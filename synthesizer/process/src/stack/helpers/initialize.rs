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

// TODO: @d0cd. Check the MAX_PROGRAM_DEPTH at runtime?
// TODO: @d0cd. Recursion safety for the number of calls and costs at runtime?

impl<N: Network> Stack<N> {
    /// Initializes a new stack, given the process and program.
    #[inline]
    pub(crate) fn initialize(process: &Process<N>, program: &Program<N>, edition: u16) -> Result<Self> {
        // Construct the stack for the program.
        let mut stack = Self {
            program: program.clone(),
            stacks: process.stacks.clone(),
            register_types: Default::default(),
            finalize_types: Default::default(),
            universal_srs: process.universal_srs().clone(),
            proving_keys: Default::default(),
            verifying_keys: Default::default(),
            program_address: program.id().to_address()?,
            edition,
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

        println!("ONE");

        // Add the program closures to the stack.
        for closure in program.closures().values() {
            // Add the closure to the stack.
            stack.insert_closure(closure)?;
        }

        println!("TWO");

        // Add the program functions to the stack.
        for function in program.functions().values() {
            // Add the function to the stack.
            stack.insert_function(function)?;
            // Determine the number of calls for the function.
            // This includes a safety check for the maximum number of calls.
            stack.get_number_of_calls(function.name())?;
        }

        println!("THREE");

        // Return the stack.
        Ok(stack)
    }
}

impl<N: Network> Stack<N> {
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
