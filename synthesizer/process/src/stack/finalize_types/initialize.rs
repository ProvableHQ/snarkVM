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

impl<N: Network> FinalizeTypes<N> {
    /// Initializes a new instance of `FinalizeTypes` for the given finalize.
    /// Checks that the given finalize is well-formed for the given stack.
    ///
    /// Attention: To support user-defined ordering for awaiting on futures, this method does **not** check
    /// that all input futures are awaited **exactly** once. It does however check that all input
    /// futures are awaited at least once. This means that it is possible to deploy a program
    /// whose finalize is not well-formed, but it is not possible to execute a program whose finalize
    /// is not well-formed.
    #[inline]
    pub(super) fn initialize_finalize_types(
        stack: &(impl StackMatches<N> + StackProgram<N>),
        finalize: &Finalize<N>,
    ) -> Result<Self> {
        // Initialize a map of registers to their types.
        let mut finalize_types = Self { inputs: IndexMap::new(), destinations: IndexMap::new() };

        // Initialize a list of input futures.
        let mut input_futures = Vec::new();

        // Step 1. Check the inputs are well-formed. Store the input futures.
        for input in finalize.inputs() {
            // Check the input register type.
            finalize_types.check_input(stack, input.register(), input.finalize_type())?;

            // If the input is a future, add it to the list of input futures.
            if let FinalizeType::Future(locator) = input.finalize_type() {
                input_futures.push((input.register(), *locator));
            }
        }

        // Initialize the set of consumed futures.
        let mut consumed_futures = HashSet::new();

        // Step 2. Check the commands are well-formed. Make sure all the input futures are awaited.
        for command in finalize.commands() {
            // Check the command opcode, operands, and destinations.
            finalize_types.check_command(stack, finalize, command)?;

            // If the command is an `await`, add the future to the set of consumed futures.
            if let Command::Await(await_) = command {
                // Note: `check_command` ensures that the register is a future. This is an additional check.
                let locator = match finalize_types.get_type(stack, await_.register())? {
                    FinalizeType::Future(locator) => locator,
                    FinalizeType::Plaintext(..) => bail!("Expected a future in '{await_}'"),
                };
                consumed_futures.insert((await_.register(), locator));
            }
        }

        // Check that all input futures are consumed.
        for input_future in &input_futures {
            ensure!(
                consumed_futures.contains(input_future),
                "Futures in finalize '{}' are not all awaited.",
                finalize.name()
            )
        }

        Ok(finalize_types)
    }
}

impl<N: Network> FinalizeTypes<N> {
    /// Inserts the given input register and type into the registers.
    /// Note: The given input register must be a `Register::Locator`.
    fn add_input(&mut self, register: Register<N>, finalize_type: FinalizeType<N>) -> Result<()> {
        // Ensure there are no destination registers set yet.
        ensure!(self.destinations.is_empty(), "Cannot add input registers after destination registers.");

        // Check the input register.
        match register {
            Register::Locator(locator) => {
                // Ensure the registers are monotonically increasing.
                ensure!(self.inputs.len() as u64 == locator, "Register '{register}' is out of order");

                // Insert the input register and type.
                match self.inputs.insert(locator, finalize_type) {
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
    fn add_destination(&mut self, register: Register<N>, finalize_type: FinalizeType<N>) -> Result<()> {
        // Check the destination register.
        match register {
            Register::Locator(locator) => {
                // Ensure the registers are monotonically increasing.
                let expected_locator = (self.inputs.len() as u64) + self.destinations.len() as u64;
                ensure!(expected_locator == locator, "Register '{register}' is out of order");

                // Insert the destination register and type.
                match self.destinations.insert(locator, finalize_type) {
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

impl<N: Network> FinalizeTypes<N> {
    /// Ensure the given input register is well-formed.
    #[inline]
    fn check_input(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        register: &Register<N>,
        finalize_type: &FinalizeType<N>,
    ) -> Result<()> {
        // Ensure the register type is defined in the program.
        match finalize_type {
            FinalizeType::Plaintext(PlaintextType::Literal(..)) => (),
            FinalizeType::Plaintext(PlaintextType::Struct(struct_name)) => {
                RegisterTypes::check_struct(stack, struct_name)?
            }
            FinalizeType::Plaintext(PlaintextType::Array(array_type)) => RegisterTypes::check_array(stack, array_type)?,
            FinalizeType::Future(..) => (),
        };

        // Insert the input register.
        self.add_input(register.clone(), finalize_type.clone())?;

        // Ensure the register type and the input type match.
        if finalize_type != &self.get_type(stack, register)? {
            bail!("Input '{register}' does not match the expected input register type.")
        }

        Ok(())
    }

    /// Ensures the given command is well-formed.
    #[inline]
    fn check_command(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        finalize: &Finalize<N>,
        command: &Command<N>,
    ) -> Result<()> {
        match command {
            Command::Instruction(instruction) => self.check_instruction(stack, finalize.name(), instruction)?,
            Command::Await(await_) => self.check_await(stack, await_)?,
            Command::Contains(contains) => self.check_contains(stack, contains)?,
            Command::Get(get) => self.check_get(stack, get)?,
            Command::GetOrUse(get_or_use) => self.check_get_or_use(stack, get_or_use)?,
            Command::RandChaCha(rand_chacha) => self.check_rand_chacha(stack, finalize.name(), rand_chacha)?,
            Command::Remove(remove) => self.check_remove(stack, finalize.name(), remove)?,
            Command::Set(set) => self.check_set(stack, finalize.name(), set)?,
            Command::BranchEq(branch_eq) => self.check_branch(stack, finalize, branch_eq)?,
            Command::BranchNeq(branch_neq) => self.check_branch(stack, finalize, branch_neq)?,
            // Note that the `Position`s are checked for uniqueness when constructing `Finalize`.
            Command::Position(_) => (),
        }
        Ok(())
    }

    /// Checks that the given `await` command is well-formed.
    #[inline]
    fn check_await(&mut self, stack: &(impl StackMatches<N> + StackProgram<N>), await_: &Await<N>) -> Result<()> {
        // Ensure that the register is a locator.
        ensure!(
            matches!(await_.register(), Register::Locator(..)),
            "The await register '{}' must be a locator.",
            await_.register()
        );
        // Ensure that the register is a future.
        match self.get_type(stack, await_.register())? {
            // If the register is a plaintext type, throw an error.
            FinalizeType::Plaintext(..) => bail!("Expected a future"),
            // If the register is a future, return success.
            // Note that there are not restrictions on the exact type of future.
            FinalizeType::Future(..) => Ok(()),
        }
    }

    /// Checks that the given variant of the `branch` command is well-formed.
    #[inline]
    fn check_branch<const VARIANT: u8>(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        finalize: &Finalize<N>,
        branch: &Branch<N, VARIANT>,
    ) -> Result<()> {
        // Get the type of the first operand.
        let first_type = match self.get_type_from_operand(stack, branch.first())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => bail!("A future cannot be used in a `branch` command"),
        };
        // Get the type of the second operand.
        let second_type = match self.get_type_from_operand(stack, branch.second())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => bail!("A future cannot be used in a `branch` command"),
        };
        // Check that the operands have the same type.
        ensure!(
            first_type == second_type,
            "Command '{}' expects operands of the same type. Found operands of type '{}' and '{}'",
            Branch::<N, VARIANT>::opcode(),
            first_type,
            second_type
        );
        // Check that the `Position` has been defined.
        ensure!(
            finalize.positions().get(branch.position()).is_some(),
            "Command '{}' expects a defined position to jump to. Found undefined position '{}'",
            Branch::<N, VARIANT>::opcode(),
            branch.position()
        );
        Ok(())
    }

    /// Ensures the given `contains` command is well-formed.
    #[inline]
    fn check_contains(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        contains: &Contains<N>,
    ) -> Result<()> {
        // Retrieve the mapping.
        let mapping = match contains.mapping() {
            CallOperator::Locator(locator) => {
                // Retrieve the program ID.
                let program_id = locator.program_id();
                // Retrieve the mapping_name.
                let mapping_name = locator.resource();

                // Ensure the locator does not reference the current program.
                if stack.program_id() == program_id {
                    bail!("Locator '{locator}' does not reference an external mapping.");
                }
                // Ensure the current program contains an import for this external program.
                if !stack.program().imports().keys().contains(program_id) {
                    bail!("External program '{program_id}' is not imported by '{}'.", stack.program_id());
                }
                // Retrieve the program.
                let external = stack.get_external_program(program_id)?;
                // Ensure the mapping exists in the program.
                if !external.contains_mapping(mapping_name) {
                    bail!("Mapping '{mapping_name}' in '{program_id}' is not defined.")
                }
                // Retrieve the mapping from the program.
                external.get_mapping(mapping_name)?
            }
            CallOperator::Resource(mapping_name) => {
                // Ensure the declared mapping in `contains` is defined in the current program.
                if !stack.program().contains_mapping(mapping_name) {
                    bail!("Mapping '{mapping_name}' in '{}' is not defined.", stack.program_id())
                }
                // Retrieve the mapping from the program.
                stack.program().get_mapping(mapping_name)?
            }
        };

        // Get the mapping key type.
        let mapping_key_type = mapping.key().plaintext_type();
        // Retrieve the register type of the key.
        let key_type = match self.get_type_from_operand(stack, contains.key())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => bail!("A future cannot be used as a key in a `contains` command"),
        };
        // Check that the key type in the mapping matches the key type in the instruction.
        if *mapping_key_type != key_type {
            bail!(
                "Key type in `contains` '{key_type}' does not match the key type in the mapping '{mapping_key_type}'."
            )
        }
        // Get the destination register.
        let destination = contains.destination().clone();
        // Ensure the destination register is a locator (and does not reference an access).
        ensure!(matches!(destination, Register::Locator(..)), "Destination '{destination}' must be a locator.");
        // Insert the destination register.
        self.add_destination(destination, FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::Boolean)))?;
        Ok(())
    }

    /// Ensures the given `get` command is well-formed.
    #[inline]
    fn check_get(&mut self, stack: &(impl StackMatches<N> + StackProgram<N>), get: &Get<N>) -> Result<()> {
        // Retrieve the mapping.
        let mapping = match get.mapping() {
            CallOperator::Locator(locator) => {
                // Retrieve the program ID.
                let program_id = locator.program_id();
                // Retrieve the mapping_name.
                let mapping_name = locator.resource();

                // Ensure the locator does not reference the current program.
                if stack.program_id() == program_id {
                    bail!("Locator '{locator}' does not reference an external mapping.");
                }
                // Ensure the current program contains an import for this external program.
                if !stack.program().imports().keys().contains(program_id) {
                    bail!("External program '{program_id}' is not imported by '{}'.", stack.program_id());
                }
                // Retrieve the program.
                let external = stack.get_external_program(program_id)?;
                // Ensure the mapping exists in the program.
                if !external.contains_mapping(mapping_name) {
                    bail!("Mapping '{mapping_name}' in '{program_id}' is not defined.")
                }
                // Retrieve the mapping from the program.
                external.get_mapping(mapping_name)?
            }
            CallOperator::Resource(mapping_name) => {
                // Ensure the declared mapping in `get` is defined in the current program.
                if !stack.program().contains_mapping(mapping_name) {
                    bail!("Mapping '{mapping_name}' in '{}' is not defined.", stack.program_id())
                }
                // Retrieve the mapping from the program.
                stack.program().get_mapping(mapping_name)?
            }
        };

        // Get the mapping key type.
        let mapping_key_type = mapping.key().plaintext_type();
        // Get the mapping value type.
        let mapping_value_type = mapping.value().plaintext_type();
        // Retrieve the register type of the key.
        let key_type = match self.get_type_from_operand(stack, get.key())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => bail!("A future cannot be used as a key in a `get` command"),
        };
        // Check that the key type in the mapping matches the key type in the instruction.
        if *mapping_key_type != key_type {
            bail!("Key type in `get` '{key_type}' does not match the key type in the mapping '{mapping_key_type}'.")
        }
        // Get the destination register.
        let destination = get.destination().clone();
        // Ensure the destination register is a locator (and does not reference an access).
        ensure!(matches!(destination, Register::Locator(..)), "Destination '{destination}' must be a locator.");
        // Insert the destination register.
        self.add_destination(destination, FinalizeType::Plaintext(mapping_value_type.clone()))?;
        Ok(())
    }

    /// Ensures the given `get.or_use` command is well-formed.
    #[inline]
    fn check_get_or_use(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        get_or_use: &GetOrUse<N>,
    ) -> Result<()> {
        // Retrieve the mapping.
        let mapping = match get_or_use.mapping() {
            CallOperator::Locator(locator) => {
                // Retrieve the program ID.
                let program_id = locator.program_id();
                // Retrieve the mapping_name.
                let mapping_name = locator.resource();

                // Ensure the locator does not reference the current program.
                if stack.program_id() == program_id {
                    bail!("Locator '{locator}' does not reference an external mapping.");
                }
                // Ensure the current program contains an import for this external program.
                if !stack.program().imports().keys().contains(program_id) {
                    bail!("External program '{locator}' is not imported by '{program_id}'.");
                }
                // Retrieve the program.
                let external = stack.get_external_program(program_id)?;
                // Ensure the mapping exists in the program.
                if !external.contains_mapping(mapping_name) {
                    bail!("Mapping '{mapping_name}' in '{program_id}' is not defined.")
                }
                // Retrieve the mapping from the program.
                external.get_mapping(mapping_name)?
            }
            CallOperator::Resource(mapping_name) => {
                // Ensure the declared mapping in `get.or_use` is defined in the current program.
                if !stack.program().contains_mapping(mapping_name) {
                    bail!("Mapping '{mapping_name}' in '{}' is not defined.", stack.program_id())
                }
                // Retrieve the mapping from the program.
                stack.program().get_mapping(mapping_name)?
            }
        };

        // Get the mapping key type.
        let mapping_key_type = mapping.key().plaintext_type();
        // Get the mapping value type.
        let mapping_value_type = mapping.value().plaintext_type();
        // Retrieve the register type of the key.
        let key_type = match self.get_type_from_operand(stack, get_or_use.key())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => bail!("A future cannot be used as a key in a `get.or_use` command"),
        };
        // Check that the key type in the mapping matches the key type.
        if *mapping_key_type != key_type {
            bail!(
                "Key type in `get.or_use` '{key_type}' does not match the key type in the mapping '{mapping_key_type}'."
            )
        }
        // Retrieve the register type of the default value.
        let default_value_type = match self.get_type_from_operand(stack, get_or_use.default())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => bail!("A default value cannot be a future"),
        };
        // Check that the value type in the mapping matches the default value type.
        if mapping_value_type != &default_value_type {
            bail!(
                "Default value type in `get.or_use` '{default_value_type}' does not match the value type in the mapping '{mapping_value_type}'."
            )
        }
        // Get the destination register.
        let destination = get_or_use.destination().clone();
        // Ensure the destination register is a locator (and does not reference an access).
        ensure!(matches!(destination, Register::Locator(..)), "Destination '{destination}' must be a locator.");
        // Insert the destination register.
        self.add_destination(destination, FinalizeType::Plaintext(default_value_type))?;
        Ok(())
    }

    /// Ensure the given `rand.chacha` command is well-formed.
    #[inline]
    fn check_rand_chacha(
        &mut self,
        _stack: &(impl StackMatches<N> + StackProgram<N>),
        _finalize_name: &Identifier<N>,
        rand_chacha: &RandChaCha<N>,
    ) -> Result<()> {
        // Ensure the number of operands is within bounds.
        if rand_chacha.operands().len() > MAX_ADDITIONAL_SEEDS {
            bail!("The number of operands must be <= {MAX_ADDITIONAL_SEEDS}")
        }

        // Get the destination register.
        let destination = rand_chacha.destination().clone();
        // Ensure the destination register is a locator (and does not reference an access).
        ensure!(matches!(destination, Register::Locator(..)), "Destination '{destination}' must be a locator.");

        // Get the destination type.
        let destination_type = rand_chacha.destination_type();
        // Ensure the destination type is allowed.
        ensure!(
            !matches!(destination_type, LiteralType::String),
            "Destination type '{destination_type}' is not allowed."
        );

        // Insert the destination register.
        self.add_destination(destination, FinalizeType::Plaintext(PlaintextType::from(destination_type)))?;
        Ok(())
    }

    /// Ensures the given `set` command is well-formed.
    #[inline]
    fn check_set(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        finalize_name: &Identifier<N>,
        set: &Set<N>,
    ) -> Result<()> {
        // Ensure the declared mapping in `set` is defined in the program.
        if !stack.program().contains_mapping(set.mapping_name()) {
            bail!("Mapping '{}' in '{}/{finalize_name}' is not defined.", set.mapping_name(), stack.program_id())
        }
        // Retrieve the mapping from the program.
        // Note that the unwrap is safe, as we have already checked the mapping exists.
        let mapping = stack.program().get_mapping(set.mapping_name()).unwrap();
        // Get the mapping key type.
        let mapping_key_type = mapping.key().plaintext_type();
        // Get the mapping value type.
        let mapping_value_type = mapping.value().plaintext_type();
        // Retrieve the register type of the key.
        let key_type = match self.get_type_from_operand(stack, set.key())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => bail!("A future cannot be used as a key in a `set` command"),
        };
        // Check that the key type in the mapping matches the key type.
        if *mapping_key_type != key_type {
            bail!("Key type in `set` '{key_type}' does not match the key type in the mapping '{mapping_key_type}'.")
        }
        // Retrieve the type of the value.
        let value_type = match self.get_type_from_operand(stack, set.value())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => bail!("A future cannot be used as a value in a `set` command"),
        };
        // Check that the value type in the mapping matches the type of the value.
        if mapping_value_type != &value_type {
            bail!(
                "Value type in `set` '{value_type}' does not match the value type in the mapping '{mapping_value_type}'."
            )
        }
        Ok(())
    }

    /// Ensures the given `remove` command is well-formed.
    #[inline]
    fn check_remove(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        finalize_name: &Identifier<N>,
        remove: &Remove<N>,
    ) -> Result<()> {
        // Ensure the declared mapping in `remove` is defined in the program.
        if !stack.program().contains_mapping(remove.mapping_name()) {
            bail!("Mapping '{}' in '{}/{finalize_name}' is not defined.", remove.mapping_name(), stack.program_id())
        }
        // Retrieve the mapping from the program.
        // Note that the unwrap is safe, as we have already checked the mapping exists.
        let mapping = stack.program().get_mapping(remove.mapping_name()).unwrap();
        // Get the mapping key type.
        let mapping_key_type = mapping.key().plaintext_type();
        // Retrieve the register type of the key.
        let key_type = match self.get_type_from_operand(stack, remove.key())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => bail!("A future cannot be used as a key in a `remove` command"),
        };
        // Check that the key type in the mapping matches the key type.
        if *mapping_key_type != key_type {
            bail!("Key type in `remove` '{key_type}' does not match the key type in the mapping '{mapping_key_type}'.")
        }
        Ok(())
    }

    /// Ensures the given instruction is well-formed.
    #[inline]
    fn check_instruction(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        finalize_name: &Identifier<N>,
        instruction: &Instruction<N>,
    ) -> Result<()> {
        // Ensure the opcode is well-formed.
        self.check_instruction_opcode(stack, finalize_name, instruction)?;

        // Initialize a vector to store the register types of the operands.
        let mut operand_types = Vec::with_capacity(instruction.operands().len());
        // Iterate over the operands, and retrieve the register type of each operand.
        for operand in instruction.operands() {
            // Retrieve and append the register type.
            operand_types.push(RegisterType::from(self.get_type_from_operand(stack, operand)?));
        }

        // Compute the destination register types.
        let destination_types = instruction.output_types(stack, &operand_types)?;

        // Insert the destination register.
        for (destination, destination_type) in
            instruction.destinations().into_iter().zip_eq(destination_types.into_iter())
        {
            // Ensure the destination register is a locator (and does not reference an access).
            ensure!(matches!(destination, Register::Locator(..)), "Destination '{destination}' must be a locator.");
            // Ensure that the destination type is a plaintext type.
            let destination_type = match destination_type {
                RegisterType::Plaintext(destination_type) => FinalizeType::Plaintext(destination_type),
                RegisterType::Future(locator) => FinalizeType::Future(locator),
                _ => bail!("Destination type '{destination_type}' must be a plaintext type."),
            };
            // Insert the destination register.
            self.add_destination(destination, destination_type)?;
        }
        Ok(())
    }

    /// Ensures the opcode is a valid opcode and corresponds to the correct instruction.
    /// This method is called when adding a new closure or function to the program.
    #[inline]
    fn check_instruction_opcode(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        finalize_name: &Identifier<N>,
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
                bail!("Instruction 'async' is not allowed in 'finalize'");
            }
            Opcode::Call => {
                bail!("Instruction 'call' is not allowed in 'finalize'");
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
                        CastType::Record(..) => {
                            bail!("Illegal operation: Cannot cast to a record.")
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
                bail!("Fatal error: Cannot check command '{opcode}' as an instruction in 'finalize {finalize_name}'.")
            }
            Opcode::Commit(opcode) => RegisterTypes::check_commit_opcode(opcode, instruction)?,
            Opcode::Hash(opcode) => RegisterTypes::check_hash_opcode(opcode, instruction)?,
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
    //         RegisterType::Record(..) => {
    //             bail!("Illegal operation: Cannot cast to a record.")
    //         }
    //         RegisterType::ExternalRecord(_locator) => {
    //             bail!("Illegal operation: Cannot cast to an external record.")
    //         }
    //     }
    //     Ok(())
    // }
}
