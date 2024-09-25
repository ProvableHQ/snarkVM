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

impl<N: Network> RegisterTypes<N> {
    /// Initializes a new instance of `RegisterTypes` for the given closure.
    /// Checks that the given closure is well-formed for the given stack.
    #[inline]
    pub(super) fn initialize_closure_types(
        stack: &(impl StackMatches<N> + StackProgram<N>),
        closure: &Closure<N>,
    ) -> Result<Self> {
        // Initialize a map of registers to their types.
        let mut register_types = Self { inputs: IndexMap::new(), destinations: IndexMap::new() };

        // Step 1. Check the inputs are well-formed.
        for input in closure.inputs() {
            // Check the input register type.
            register_types.check_input(stack, input.register(), input.register_type())?;
        }

        // Step 2. Check the instructions are well-formed.
        for instruction in closure.instructions() {
            // Ensure the closure contains no async instructions.
            ensure!(instruction.opcode() != Opcode::Async, "An 'async' instruction is not allowed in closures");
            // Ensure the closure contains no call instructions.
            ensure!(instruction.opcode() != Opcode::Call, "A 'call' instruction is not allowed in closures");
            // Check the instruction opcode, operands, and destinations.
            register_types.check_instruction(stack, closure.name(), instruction)?;
        }

        // Step 3. Check the outputs are well-formed.
        for output in closure.outputs() {
            // Ensure the closure output register is not a record.
            ensure!(
                !matches!(output.register_type(), RegisterType::Record(..)),
                "Closure outputs do not support records"
            );

            // Check the output operand type.
            register_types.check_output(stack, output.operand(), output.register_type())?;
        }

        Ok(register_types)
    }

    /// Initializes a new instance of `RegisterTypes` for the given function.
    /// Checks that the given function is well-formed for the given stack.
    #[inline]
    pub(super) fn initialize_function_types(
        stack: &(impl StackMatches<N> + StackProgram<N>),
        function: &Function<N>,
    ) -> Result<Self> {
        // Initialize a map of registers to their types.
        let mut register_types = Self { inputs: IndexMap::new(), destinations: IndexMap::new() };

        /* Step 1. Check the inputs are well-formed. */

        for input in function.inputs() {
            // TODO (howardwu): In order to support constant inputs, update `Self::deploy()` to allow
            //  the caller to provide optional constant inputs (instead of sampling random constants).
            //  Then, this check can be removed to enable support for constant inputs in functions.
            ensure!(!matches!(input.value_type(), ValueType::Constant(..)), "Constant inputs are not supported");
            ensure!(!matches!(input.value_type(), ValueType::Future(..)), "Future inputs are not supported");

            // Check the input register type.
            register_types.check_input(stack, input.register(), &RegisterType::from(input.value_type().clone()))?;
        }

        /* Step 2. Check the instructions are well-formed. */
        // - If the function has a finalize block, then it must contain exactly one `async` instruction.
        // - If the function has no finalize block, then it must **not** have `async` instructions.
        // - All `call` instructions must precede any `async` instruction.

        let mut async_ = None;
        for instruction in function.instructions() {
            // Check the instruction opcode, operands, and destinations.
            register_types.check_instruction(stack, function.name(), instruction)?;
            // Additional validation.
            match instruction.opcode() {
                Opcode::Async => {
                    // Ensure the function does not contain more than one `async` instruction.
                    ensure!(
                        async_.is_none(),
                        "Function '{}' can contain at most one 'async' instruction",
                        function.name()
                    );
                    // Save the `async` instruction.
                    async_ = match &instruction {
                        Instruction::Async(async_) => Some(async_),
                        _ => bail!("Expected 'async' instruction"),
                    };
                }
                Opcode::Call => {
                    // Ensure the `call` instruction precedes any `async` instruction.
                    ensure!(async_.is_none(), "The 'call' can only be invoked before an 'async' instruction")
                }
                _ => {}
            }
        }

        // Ensure the number of `async` instructions is valid.
        if function.finalize_logic().is_some() {
            ensure!(async_.is_some(), "Function '{}' must contain exactly one 'async' instruction.", function.name());
        } else {
            ensure!(async_.is_none(), "Function '{}' must not contain any 'async' instructions.", function.name());
        }

        /* Step 3. Check the outputs are well-formed. */
        // - If the function has a finalize block, then its last output must be a future associated with itself.
        // - If the function has no finalize block, then it must **not** have `future` outputs.

        let mut num_futures = 0;
        for output in function.outputs() {
            // Check the output operand type.
            register_types.check_output(stack, output.operand(), &RegisterType::from(output.value_type().clone()))?;
            // Additional validation.
            if matches!(output.value_type(), ValueType::Future(..)) {
                num_futures += 1;
            }
        }

        // Ensure the `future` outputs are valid.
        if function.finalize_logic().is_some() {
            ensure!(
                num_futures == 1,
                "Function '{}' must contain exactly one 'future' output, found {num_futures}",
                function.name()
            );
            ensure!(
                match function.outputs().last().map(|output| output.value_type()) {
                    Some(ValueType::Future(locator)) =>
                        locator.program_id() == stack.program_id() && locator.resource() == function.name(),
                    _ => false,
                },
                "The last output of function '{}' must be a future associated with itself",
                function.name()
            );
        } else {
            ensure!(
                num_futures == 0,
                "Function '{}' must not contain any 'future' outputs, found {num_futures}",
                function.name()
            );
        }

        /* Additional checks. */
        // - All futures produces before the `async` call must be consumed by the `async` call.

        // Get all registers containing futures.
        let mut future_registers: IndexSet<(Register<N>, Locator<N>)> = register_types
            .destinations
            .iter()
            .filter_map(|(index, register_type)| match register_type {
                RegisterType::Future(locator) => Some((Register::<N>::Locator(*index), *locator)),
                _ => None,
            })
            .collect();

        match async_ {
            // If no `async` instruction exists, then there should not be any future registers.
            None => {
                ensure!(
                    future_registers.is_empty(),
                    "Function '{}' contains futures, but does not contain an 'async' instruction",
                    function.name()
                )
            }
            // Otherwise, check that all the registers were consumed by the `async` call.
            Some(async_) => {
                // Remove the last future, since this is the future created by the `async` call.
                future_registers.pop();
                // Check only the register operands that are `future` types.
                for operand in async_.operands() {
                    if let Operand::Register(register) = operand {
                        if let Ok(RegisterType::Future(locator)) = register_types.get_type(stack, register) {
                            assert!(future_registers.swap_remove(&(register.clone(), locator)));
                        }
                    }
                }
                // Ensure that all the futures created are consumed in the async call.
                ensure!(
                    future_registers.is_empty(),
                    "Function '{}' contains futures, but the 'async' instruction does not consume all of the ones produced.",
                    function.name()
                );
            }
        }

        Ok(register_types)
    }
}

impl<N: Network> RegisterTypes<N> {
    /// Inserts the given input register and type into the registers.
    /// Note: The given input register must be a `Register::Locator`.
    fn add_input(&mut self, register: Register<N>, register_type: RegisterType<N>) -> Result<()> {
        // Ensure there are no destination registers set yet.
        ensure!(self.destinations.is_empty(), "Cannot add input registers after destination registers.");

        // Check the input register.
        match register {
            Register::Locator(locator) => {
                // Ensure the registers are monotonically increasing.
                ensure!(self.inputs.len() as u64 == locator, "Register '{register}' is out of order");

                // Insert the input register and type.
                match self.inputs.insert(locator, register_type) {
                    // If the register already exists, throw an error.
                    Some(..) => bail!("Input '{register}' already exists"),
                    // If the register does not exist, return success.
                    None => Ok(()),
                }
            }
            // Ensure the register is a locator, and not an access.
            Register::Access(..) => bail!("Register '{register}' must be a locator."),
        }
    }

    /// Inserts the given destination register and type into the registers.
    /// Note: The given destination register must be a `Register::Locator`.
    fn add_destination(&mut self, register: Register<N>, register_type: RegisterType<N>) -> Result<()> {
        // Check the destination register.
        match register {
            Register::Locator(locator) => {
                // Ensure the registers are monotonically increasing.
                let expected_locator = (self.inputs.len() as u64) + self.destinations.len() as u64;
                ensure!(expected_locator == locator, "Register '{register}' is out of order");

                // Insert the destination register and type.
                match self.destinations.insert(locator, register_type) {
                    // If the register already exists, throw an error.
                    Some(..) => bail!("Destination '{register}' already exists"),
                    // If the register does not exist, return success.
                    None => Ok(()),
                }
            }
            // Ensure the register is a locator, and not an access.
            Register::Access(..) => bail!("Register '{register}' must be a locator."),
        }
    }
}

impl<N: Network> RegisterTypes<N> {
    /// Ensure the given input register is well-formed.
    #[inline]
    fn check_input(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        register: &Register<N>,
        register_type: &RegisterType<N>,
    ) -> Result<()> {
        // Ensure the register type is defined in the program.
        match register_type {
            RegisterType::Plaintext(PlaintextType::Literal(..)) => (),
            RegisterType::Plaintext(PlaintextType::Struct(struct_name)) => Self::check_struct(stack, struct_name)?,
            RegisterType::Plaintext(PlaintextType::Array(array_type)) => Self::check_array(stack, array_type)?,
            RegisterType::Record(identifier) => {
                // Ensure the record type is defined in the program.
                if !stack.program().contains_record(identifier) {
                    bail!("Record '{identifier}' in '{}' is not defined.", stack.program_id())
                }
            }
            RegisterType::ExternalRecord(locator) => {
                // Ensure the external record type is defined in the program.
                if !stack.contains_external_record(locator) {
                    bail!("External record '{locator}' in '{}' is not defined.", stack.program_id())
                }
            }
            RegisterType::Future(..) => bail!("Input '{register}' cannot be a future."),
        };

        // Insert the input register.
        self.add_input(register.clone(), register_type.clone())?;

        // Ensure the register type and the input type match.
        if *register_type != self.get_type(stack, register)? {
            bail!("Input '{register}' does not match the expected input register type.")
        }
        Ok(())
    }

    /// Ensure the given output register is well-formed.
    #[inline]
    fn check_output(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        operand: &Operand<N>,
        register_type: &RegisterType<N>,
    ) -> Result<()> {
        #[cfg(feature = "aleo-cli")]
        match operand {
            // Inform the user the output operand is an input register, to ensure this is intended behavior.
            Operand::Register(register) if self.is_input(register) => {
                eprintln!("Output {operand} in '{}' is an input register, ensure this is intended", stack.program_id())
            }
            // Inform the user the output operand is a literal, to ensure this is intended behavior.
            Operand::Literal(..) => {
                eprintln!("Output {operand} in '{}' is a literal, ensure this is intended", stack.program_id())
            }
            // Otherwise, do nothing.
            _ => (),
        }

        // Ensure the register type is defined in the program.
        match register_type {
            RegisterType::Plaintext(PlaintextType::Literal(..)) => (),
            RegisterType::Plaintext(PlaintextType::Struct(struct_name)) => Self::check_struct(stack, struct_name)?,
            RegisterType::Plaintext(PlaintextType::Array(array_type)) => Self::check_array(stack, array_type)?,
            RegisterType::Record(identifier) => {
                // Ensure the record type is defined in the program.
                if !stack.program().contains_record(identifier) {
                    bail!("Record '{identifier}' in '{}' is not defined.", stack.program_id())
                }
            }
            RegisterType::ExternalRecord(locator) => {
                // Ensure the external record type is defined in the program.
                if !stack.contains_external_record(locator) {
                    bail!("External record '{locator}' in '{}' is not defined.", stack.program_id())
                }
            }
            RegisterType::Future(locator) => {
                // Ensure that the locator is defined.
                match locator.program_id() == stack.program_id() {
                    true => stack.get_function(locator.resource())?,
                    false => stack.get_external_program(locator.program_id())?.get_function(locator.resource())?,
                };
            }
        };

        // Ensure the operand type and the output type match.
        if *register_type != self.get_type_from_operand(stack, operand)? {
            bail!(
                "Output '{operand}' does not match the expected output operand type: expected '{}', found '{}'",
                self.get_type_from_operand(stack, operand)?,
                register_type
            )
        }
        Ok(())
    }

    /// Ensures the given instruction is well-formed.
    #[inline]
    fn check_instruction(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        closure_or_function_name: &Identifier<N>,
        instruction: &Instruction<N>,
    ) -> Result<()> {
        // Ensure the opcode is well-formed.
        self.check_instruction_opcode(stack, closure_or_function_name, instruction)?;

        // Initialize a vector to store the register types of the operands.
        let mut operand_types = Vec::with_capacity(instruction.operands().len());
        // Iterate over the operands, and retrieve the register type of each operand.
        for operand in instruction.operands() {
            // Retrieve and append the register type.
            operand_types.push(self.get_type_from_operand(stack, operand)?);
        }

        // Compute the destination register types.
        let destination_types = instruction.output_types(stack, &operand_types)?;

        // Insert the destination register.
        for (destination, destination_type) in
            instruction.destinations().into_iter().zip_eq(destination_types.into_iter())
        {
            // Ensure the destination register is a locator (and does not reference an access).
            ensure!(matches!(destination, Register::Locator(..)), "Destination '{destination}' must be a locator.");
            // Insert the destination register.
            self.add_destination(destination, destination_type)?;
        }
        Ok(())
    }

    /// Ensures the opcode is a valid opcode and corresponds to the correct instruction.
    /// This method is called when adding a new closure or function to the program.
    #[inline]
    fn check_instruction_opcode(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        closure_or_function_name: &Identifier<N>,
        instruction: &Instruction<N>,
    ) -> Result<()> {
        match instruction.opcode() {
            Opcode::Literal(opcode) => {
                // Ensure the opcode **is** a reserved opcode.
                ensure!(Program::<N>::is_reserved_opcode(opcode), "'{opcode}' is not an opcode.");
                // Ensure the instruction is not the cast operation.
                ensure!(!matches!(instruction, Instruction::Cast(..)), "Instruction '{instruction}' is a 'cast'.");
                // Ensure the instruction has one destination register.
                ensure!(
                    instruction.destinations().len() == 1,
                    "Instruction '{instruction}' has multiple destinations."
                );
            }
            Opcode::Assert(opcode) => match opcode {
                "assert.eq" => ensure!(
                    matches!(instruction, Instruction::AssertEq(..)),
                    "Instruction '{instruction}' is not for opcode '{opcode}'."
                ),
                "assert.neq" => ensure!(
                    matches!(instruction, Instruction::AssertNeq(..)),
                    "Instruction '{instruction}' is not for opcode '{opcode}'."
                ),
                _ => bail!("Instruction '{instruction}' is not for opcode '{opcode}'."),
            },
            Opcode::Async => {
                // Retrieve the async operation.
                let async_ = match instruction {
                    Instruction::Async(async_) => async_,
                    _ => bail!("Instruction '{instruction}' is not an async operation."),
                };

                // Ensure the function name matches the one in the operation.
                ensure!(
                    async_.function_name() == closure_or_function_name,
                    "Instruction '{instruction}' does not match the function name '{closure_or_function_name}'."
                );
            }
            Opcode::Call => {
                // Retrieve the call operation.
                let call = match instruction {
                    Instruction::Call(call) => call,
                    _ => bail!("Instruction '{instruction}' is not a call operation."),
                };

                // Retrieve the operator.
                match call.operator() {
                    CallOperator::Locator(locator) => {
                        // Retrieve the program ID.
                        let program_id = locator.program_id();
                        // Retrieve the resource from the locator.
                        let resource = locator.resource();

                        // Ensure the locator does not reference the current program.
                        if stack.program_id() == program_id {
                            bail!("Locator '{locator}' does not reference an external program.");
                        }
                        // Ensure the current program contains an import for this external program.
                        if !stack.program().imports().keys().contains(program_id) {
                            bail!("External program '{}' is not imported by '{program_id}'.", locator.program_id());
                        }

                        // Retrieve the program.
                        let external = stack.get_external_program(program_id)?;
                        // Check that function exists in the program.
                        if let Ok(child_function) = external.get_function_ref(resource) {
                            // If the child function contains a finalize block, then the parent function must also contain a finalize block.
                            let child_contains_finalize = child_function.finalize_logic().is_some();
                            let parent_contains_finalize =
                                stack.get_function_ref(closure_or_function_name)?.finalize_logic().is_some();
                            if child_contains_finalize && !parent_contains_finalize {
                                bail!(
                                    "Function '{}/{closure_or_function_name}' must contain a finalize block, since it calls '{}/{resource}'.",
                                    stack.program_id(),
                                    program_id
                                )
                            }
                        }
                        // Otherwise, ensure the closure exists in the program.
                        else if !external.contains_closure(resource) {
                            bail!("'{resource}' is not defined in '{}'.", external.id())
                        }
                    }
                    CallOperator::Resource(resource) => {
                        // Ensure the resource does not reference this closure or function.
                        if resource == closure_or_function_name {
                            bail!("Cannot invoke 'call' to self (in '{resource}'): self-recursive call.")
                        }

                        // TODO (howardwu): Revisit this decision to forbid calling internal functions. A record cannot be spent again.
                        //  But there are legitimate uses for passing a record through to an internal function.
                        //  We could invoke the internal function without a state transition, but need to match visibility.
                        if stack.program().contains_function(resource) {
                            bail!(
                                "Cannot call '{resource}' from '{closure_or_function_name}'. Use a closure ('closure {resource}:') instead."
                            )
                        }
                        // Ensure the function or closure exists in the program.
                        // if !self.program.contains_function(resource) && !self.program.contains_closure(resource) {
                        if !stack.program().contains_closure(resource) {
                            bail!("'{resource}' is not defined in '{}'.", stack.program_id())
                        }
                    }
                }
            }
            Opcode::Cast(opcode) => match opcode {
                "cast" => {
                    // Retrieve the cast operation.
                    let operation = match instruction {
                        Instruction::Cast(operation) => operation,
                        _ => bail!("Instruction '{instruction}' is not a cast operation."),
                    };

                    // Ensure the instruction has one destination register.
                    ensure!(
                        instruction.destinations().len() == 1,
                        "Instruction '{instruction}' has multiple destinations."
                    );

                    // Ensure the casted register type is defined.
                    match operation.cast_type() {
                        CastType::GroupXCoordinate
                        | CastType::GroupYCoordinate
                        | CastType::Plaintext(PlaintextType::Literal(..)) => {
                            ensure!(instruction.operands().len() == 1, "Expected 1 operand.");
                        }
                        CastType::Plaintext(PlaintextType::Struct(struct_name)) => {
                            // Ensure the struct name exists in the program.
                            if !stack.program().contains_struct(struct_name) {
                                bail!("Struct '{struct_name}' is not defined.")
                            }
                            // Retrieve the struct.
                            let struct_ = stack.program().get_struct(struct_name)?;
                            // Ensure the operand types match the struct.
                            self.matches_struct(stack, instruction.operands(), struct_)?;
                        }
                        CastType::Plaintext(PlaintextType::Array(array_type)) => {
                            // Ensure that the array type is valid.
                            RegisterTypes::check_array(stack, array_type)?;
                            // Ensure the operand types match the element type.
                            self.matches_array(stack, instruction.operands(), array_type)?;
                        }
                        CastType::Record(record_name) => {
                            // Ensure the record type is defined in the program.
                            if !stack.program().contains_record(record_name) {
                                bail!("Record '{record_name}' is not defined.")
                            }
                            // Retrieve the record type.
                            let record_type = stack.program().get_record(record_name)?;
                            // Ensure the operand types match the record type.
                            self.matches_record(stack, instruction.operands(), record_type)?;
                        }
                        CastType::ExternalRecord(_locator) => {
                            bail!("Illegal operation: Cannot cast to an external record.")
                        }
                    }
                }
                "cast.lossy" => {
                    // Retrieve the cast operation.
                    let operation = match instruction {
                        Instruction::CastLossy(operation) => operation,
                        _ => bail!("Instruction '{instruction}' is not a cast.lossy operation."),
                    };

                    // Ensure the instruction has one destination register.
                    ensure!(
                        instruction.destinations().len() == 1,
                        "Instruction '{instruction}' has multiple destinations."
                    );

                    // Ensure the casted register type is valid and defined.
                    match operation.cast_type() {
                        CastType::Plaintext(PlaintextType::Literal(_)) => {
                            ensure!(instruction.operands().len() == 1, "Expected 1 operand.");
                        }
                        _ => bail!("`cast.lossy` is only supported for casting to a literal type."),
                    }
                }
                _ => bail!("Instruction '{instruction}' is not for opcode '{opcode}'."),
            },
            Opcode::Command(opcode) => {
                bail!("Forbidden operation: Instruction '{instruction}' cannot invoke command '{opcode}'.");
            }
            Opcode::Commit(opcode) => Self::check_commit_opcode(opcode, instruction)?,
            Opcode::Hash(opcode) => Self::check_hash_opcode(opcode, instruction)?,
            Opcode::Is(opcode) => match opcode {
                "is.eq" => ensure!(
                    matches!(instruction, Instruction::IsEq(..)),
                    "Instruction '{instruction}' is not for opcode '{opcode}'."
                ),
                "is.neq" => ensure!(
                    matches!(instruction, Instruction::IsNeq(..)),
                    "Instruction '{instruction}' is not for opcode '{opcode}'."
                ),
                _ => bail!("Instruction '{instruction}' is not for opcode '{opcode}'."),
            },
            Opcode::Sign => {
                // Ensure the instruction has one destination register.
                ensure!(
                    instruction.destinations().len() == 1,
                    "Instruction '{instruction}' has multiple destinations."
                );
            }
        }
        Ok(())
    }
}

impl<N: Network> RegisterTypes<N> {
    // TODO (howardwu & d0cd): Reimplement this for cast and cast.lossy.
    // /// Checks the cast operation is well-formed.
    // fn check_cast_operation<const VARIANT: u8>(
    //     &self,
    //     stack: &(impl StackMatches<N> + StackProgram<N>),
    //     operation: &CastOperation<N, VARIANT>,
    // ) -> Result<()> {
    //     // Ensure the operation has one destination register.
    //     ensure!(operation.destinations().len() == 1, "Instruction '{operation}' has multiple destinations.");
    //     // Ensure the casted register type is defined.
    //     match operation.register_type() {
    //         RegisterType::Plaintext(PlaintextType::Literal(..)) => {
    //             ensure!(operation.operands().len() == 1, "Expected 1 operand.");
    //         }
    //         RegisterType::Plaintext(PlaintextType::Struct(struct_name)) => {
    //             // Ensure the struct name exists in the program.
    //             if !stack.program().contains_struct(struct_name) {
    //                 bail!("Struct '{struct_name}' is not defined.")
    //             }
    //             // Retrieve the struct.
    //             let struct_ = stack.program().get_struct(struct_name)?;
    //             // Ensure the operand types match the struct.
    //             self.matches_struct(stack, operation.operands(), struct_)?;
    //         }
    //         RegisterType::Plaintext(PlaintextType::Array(array_type)) => {
    //             // Ensure that the array type is valid.
    //             RegisterTypes::check_array(stack, array_type)?;
    //             // Ensure the operand types match the element type.
    //             self.matches_array(stack, operation.operands(), array_type)?;
    //         }
    //         RegisterType::Record(record_name) => {
    //             // Ensure the record type is defined in the program.
    //             if !stack.program().contains_record(record_name) {
    //                 bail!("Record '{record_name}' is not defined.")
    //             }
    //             // Retrieve the record type.
    //             let record_type = stack.program().get_record(record_name)?;
    //             // Ensure the operand types match the record type.
    //             self.matches_record(stack, operation.operands(), record_type)?;
    //         }
    //         RegisterType::ExternalRecord(_locator) => {
    //             bail!("Illegal operation: Cannot cast to an external record.")
    //         }
    //     }
    //     Ok(())
    // }

    /// Ensures the struct exists in the program, and recursively-checks for array members.
    pub(crate) fn check_struct(
        stack: &(impl StackMatches<N> + StackProgram<N>),
        struct_name: &Identifier<N>,
    ) -> Result<()> {
        // Retrieve the struct from the program.
        let Ok(struct_) = stack.program().get_struct(struct_name) else {
            bail!("Struct '{struct_name}' in '{}' is not defined.", stack.program_id())
        };

        // If the struct contains arrays, ensure their base element types are defined in the program.
        for member in struct_.members().values() {
            match member {
                PlaintextType::Literal(..) => (),
                PlaintextType::Struct(struct_name) => Self::check_struct(stack, struct_name)?,
                PlaintextType::Array(array_type) => Self::check_array(stack, array_type)?,
            }
        }
        Ok(())
    }

    /// Ensure the base element type of the array is defined in the program.
    pub(crate) fn check_array(
        stack: &(impl StackMatches<N> + StackProgram<N>),
        array_type: &ArrayType<N>,
    ) -> Result<()> {
        // If the base element type is a struct, check that it is defined in the program.
        if let PlaintextType::Struct(struct_name) = array_type.base_element_type() {
            // Ensure the struct is defined in the program.
            if !stack.program().contains_struct(struct_name) {
                bail!("Struct '{struct_name}' in '{}' is not defined.", stack.program_id())
            }
        }
        Ok(())
    }

    /// Ensures the opcode is a valid opcode and corresponds to the `commit` instruction.
    #[inline]
    pub(crate) fn check_commit_opcode(opcode: &str, instruction: &Instruction<N>) -> Result<()> {
        // Ensure the instruction is the correct one.
        match opcode {
            "commit.bhp256" => ensure!(
                matches!(instruction, Instruction::CommitBHP256(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "commit.bhp512" => ensure!(
                matches!(instruction, Instruction::CommitBHP512(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "commit.bhp768" => ensure!(
                matches!(instruction, Instruction::CommitBHP768(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "commit.bhp1024" => ensure!(
                matches!(instruction, Instruction::CommitBHP1024(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "commit.ped64" => ensure!(
                matches!(instruction, Instruction::CommitPED64(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "commit.ped128" => ensure!(
                matches!(instruction, Instruction::CommitPED128(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            _ => bail!("Instruction '{instruction}' is not for opcode '{opcode}'."),
        }
        Ok(())
    }

    /// Ensures the opcode is a valid opcode and corresponds to the `hash` instruction.
    #[inline]
    pub(crate) fn check_hash_opcode(opcode: &str, instruction: &Instruction<N>) -> Result<()> {
        // Ensure the instruction is the correct one.
        match opcode {
            "hash.bhp256" => ensure!(
                matches!(instruction, Instruction::HashBHP256(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.bhp512" => ensure!(
                matches!(instruction, Instruction::HashBHP512(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.bhp768" => ensure!(
                matches!(instruction, Instruction::HashBHP768(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.bhp1024" => ensure!(
                matches!(instruction, Instruction::HashBHP1024(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.keccak256" => ensure!(
                matches!(instruction, Instruction::HashKeccak256(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.keccak384" => ensure!(
                matches!(instruction, Instruction::HashKeccak384(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.keccak512" => ensure!(
                matches!(instruction, Instruction::HashKeccak512(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.ped64" => ensure!(
                matches!(instruction, Instruction::HashPED64(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.ped128" => ensure!(
                matches!(instruction, Instruction::HashPED128(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.psd2" => ensure!(
                matches!(instruction, Instruction::HashPSD2(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.psd4" => ensure!(
                matches!(instruction, Instruction::HashPSD4(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.psd8" => ensure!(
                matches!(instruction, Instruction::HashPSD8(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.sha3_256" => ensure!(
                matches!(instruction, Instruction::HashSha3_256(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.sha3_384" => ensure!(
                matches!(instruction, Instruction::HashSha3_384(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash.sha3_512" => ensure!(
                matches!(instruction, Instruction::HashSha3_512(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash_many.psd2" => ensure!(
                matches!(instruction, Instruction::HashManyPSD2(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash_many.psd4" => ensure!(
                matches!(instruction, Instruction::HashManyPSD4(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            "hash_many.psd8" => ensure!(
                matches!(instruction, Instruction::HashManyPSD8(..)),
                "Instruction '{instruction}' is not for opcode '{opcode}'."
            ),
            _ => bail!("Instruction '{instruction}' is not for opcode '{opcode}'."),
        }
        Ok(())
    }
}
