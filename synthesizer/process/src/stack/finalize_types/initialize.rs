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

use snarkvm_synthesizer_error::{CommandError, FinalizeTypesInitError, InstructionCheckError, RegisterTypesInitError};
use snarkvm_synthesizer_program::types_equivalent;

use super::*;

impl<N: Network> FinalizeTypes<N> {
    /// Initializes a new instance of `FinalizeTypes` for the given constructor.
    /// Checks that the given constructor is well-formed for the given stack.
    #[inline]
    pub(super) fn initialize_finalize_types_from_constructor(
        stack: &Stack<N>,
        constructor: &Constructor<N>,
    ) -> Result<Self, FinalizeTypesInitError> {
        // Initialize a map of registers to their types.
        let mut finalize_types = Self { inputs: IndexMap::new(), destinations: IndexMap::new() };

        // Check the commands are well-formed.
        for command in constructor.commands() {
            // Ensure the command is not a call instruction.
            if command.is_call() {
                return Err(FinalizeTypesInitError::ConstructorWithCall);
            }
            // Ensure the command is not a cast to record instruction.
            if command.is_cast_to_record() {
                return Err(FinalizeTypesInitError::ConstructorWithCast);
            }
            // Ensure the command is not an await command.
            if command.is_await() {
                return Err(FinalizeTypesInitError::ConstructorWithAwait);
            }
            // Check the command opcode, operands, and destinations.
            finalize_types.check_command(stack, constructor.positions(), command)?;
        }

        Ok(finalize_types)
    }

    /// Initializes a new instance of `FinalizeTypes` for the given finalize.
    /// Checks that the given finalize is well-formed for the given stack.
    ///
    /// Attention: To support user-defined ordering for awaiting on futures, this method does **not** check
    /// that all input futures are awaited **exactly** once. It does however check that all input
    /// futures are awaited at least once. This means that it is possible to deploy a program
    /// whose finalize is not well-formed, but it is not possible to execute a program whose finalize
    /// is not well-formed.
    #[inline]
    pub(super) fn initialize_finalize_types_from_finalize(
        stack: &Stack<N>,
        finalize: &Finalize<N>,
    ) -> Result<Self, FinalizeTypesInitError> {
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
            finalize_types.check_command(stack, finalize.positions(), command)?;

            // If the command is an `await`, add the future to the set of consumed futures.
            if let Command::Await(await_) = command {
                // Note: `check_command` ensures that the register is a future. This is an additional check.
                let locator = match finalize_types.get_type(stack, await_.register())? {
                    FinalizeType::Future(locator) => locator,
                    FinalizeType::Plaintext(..) => {
                        return Err(FinalizeTypesInitError::AwaitRegisterTypeInvalid(await_.register().to_string()));
                    }
                };
                consumed_futures.insert((await_.register(), locator));
            }
        }

        // Check that all input futures are consumed.
        for input_future in &input_futures {
            if !consumed_futures.contains(input_future) {
                return Err(FinalizeTypesInitError::MissingAwait(finalize.name().to_string()));
            }
        }

        Ok(finalize_types)
    }
}

impl<N: Network> FinalizeTypes<N> {
    /// Inserts the given input register and type into the registers.
    /// Note: The given input register must be a `Register::Locator`.
    fn add_input(
        &mut self,
        register: Register<N>,
        finalize_type: FinalizeType<N>,
    ) -> Result<(), RegisterTypesInitError> {
        // Ensure there are no destination registers set yet.
        if !self.destinations.is_empty() {
            return Err(RegisterTypesInitError::InvalidAddOrder);
        }

        // Check the input register.
        match register {
            Register::Locator(locator) => {
                // Ensure the registers are monotonically increasing.
                if self.inputs.len() as u64 != locator {
                    return Err(RegisterTypesInitError::OutOfOrder(register.to_string()));
                }

                // Insert the input register and type.
                match self.inputs.insert(locator, finalize_type) {
                    // If the register already exists, throw an error.
                    Some(..) => Err(RegisterTypesInitError::AlreadyExists(register.to_string())),
                    // If the register does not exist, return success.
                    None => Ok(()),
                }
            }
            // Ensure the register is a locator, and not an access.
            Register::Access(..) => Err(RegisterTypesInitError::NotALocator(register.to_string())),
        }
    }

    /// Inserts the given destination register and type into the registers.
    /// Note: The given destination register must be a `Register::Locator`.
    fn add_destination(
        &mut self,
        register: Register<N>,
        finalize_type: FinalizeType<N>,
    ) -> Result<(), RegisterTypesInitError> {
        // Check the destination register.
        match register {
            Register::Locator(locator) => {
                // Ensure the registers are monotonically increasing.
                let expected_locator = (self.inputs.len() as u64) + self.destinations.len() as u64;
                if expected_locator != locator {
                    return Err(RegisterTypesInitError::OutOfOrder(register.to_string()));
                }

                // Insert the destination register and type.
                match self.destinations.insert(locator, finalize_type) {
                    // If the register already exists, throw an error.
                    Some(..) => Err(RegisterTypesInitError::AlreadyExists(register.to_string())),
                    // If the register does not exist, return success.
                    None => Ok(()),
                }
            }
            // Ensure the register is a locator, and not an access.
            Register::Access(..) => Err(RegisterTypesInitError::NotALocator(register.to_string())),
        }
    }
}

impl<N: Network> FinalizeTypes<N> {
    /// Ensure the given input register is well-formed.
    fn check_input(
        &mut self,
        stack: &Stack<N>,
        register: &Register<N>,
        finalize_type: &FinalizeType<N>,
    ) -> Result<(), FinalizeTypesInitError> {
        // Ensure the register type is defined in the program.
        match finalize_type {
            FinalizeType::Plaintext(plaintext_type) => RegisterTypes::check_plaintext_type(stack, plaintext_type)?,
            FinalizeType::Future(locator) => {
                if !stack.program().contains_import(locator.program_id()) {
                    return Err(FinalizeTypesInitError::MissingImport(
                        locator.to_string(),
                        stack.program().id().to_string(),
                    ));
                }
            }
        };

        // Insert the input register.
        self.add_input(register.clone(), finalize_type.clone())?;

        // Ensure the register type and the input type are equivalent.
        if !finalize_types_equivalent(stack, finalize_type, stack, &self.get_type(stack, register)?)? {
            return Err(RegisterTypesInitError::IncompatibleInputType(register.to_string()).into());
        }

        Ok(())
    }

    /// Ensures the given command is well-formed.
    #[inline]
    fn check_command(
        &mut self,
        stack: &Stack<N>,
        positions: &HashMap<Identifier<N>, usize>,
        command: &Command<N>,
    ) -> Result<(), FinalizeTypesInitError> {
        // Check the operands.
        for operand in command.operands() {
            // If the operand is `Operand::Checksum`, `Operand::Edition`, or `Operand::ProgramOwner` and it contains a program ID,
            // ensure that the program ID is imported by the current program.
            match operand {
                Operand::Checksum(program_id) | Operand::Edition(program_id) | Operand::ProgramOwner(program_id) => {
                    if let Some(program_id) = program_id {
                        if stack.get_external_stack(program_id).is_err() {
                            return Err(FinalizeTypesInitError::MissingExternalImport(
                                program_id.to_string(),
                                stack.program_id().to_string(),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        match command {
            Command::Instruction(instruction) => self.check_instruction(stack, instruction)?,
            Command::Await(await_) => self.check_await(stack, await_)?,
            Command::Contains(contains) => self.check_contains(stack, contains)?,
            Command::Get(get) => self.check_get(stack, get)?,
            Command::GetOrUse(get_or_use) => self.check_get_or_use(stack, get_or_use)?,
            Command::RandChaCha(rand_chacha) => self.check_rand_chacha(stack, rand_chacha)?,
            Command::Remove(remove) => self.check_remove(stack, remove)?,
            Command::Set(set) => self.check_set(stack, set)?,
            Command::BranchEq(branch_eq) => self.check_branch(stack, positions, branch_eq)?,
            Command::BranchNeq(branch_neq) => self.check_branch(stack, positions, branch_neq)?,
            // Note that the `Position`s are checked for uniqueness when constructing `Finalize` or `Constructor`.
            Command::Position(_) => (),
        }
        Ok(())
    }

    /// Checks that the given `await` command is well-formed.
    #[inline]
    fn check_await(&mut self, stack: &Stack<N>, await_: &Await<N>) -> Result<(), FinalizeTypesInitError> {
        // Ensure that the register is a locator.
        if !matches!(await_.register(), Register::Locator(..)) {
            return Err(FinalizeTypesInitError::AwaitRegisterInvalid(await_.register().to_string()));
        }
        // Ensure that the register is a future.
        match self.get_type(stack, await_.register())? {
            // If the register is a plaintext type, throw an error.
            FinalizeType::Plaintext(..) => {
                Err(FinalizeTypesInitError::AwaitRegisterTypeInvalid(await_.register().to_string()))
            }
            // If the register is a future, return success.
            // Note that there are not restrictions on the exact type of future.
            FinalizeType::Future(..) => Ok(()),
        }
    }

    /// Checks that the given variant of the `branch` command is well-formed.
    #[inline]
    fn check_branch<const VARIANT: u8>(
        &mut self,
        stack: &Stack<N>,
        positions: &HashMap<Identifier<N>, usize>,
        branch: &Branch<N, VARIANT>,
    ) -> Result<(), FinalizeTypesInitError> {
        // Get the type of the first operand.
        let first_type = match self.get_type_from_operand(stack, branch.first())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => {
                return Err(CommandError::IncompatibleWithFuture("branch".into()).into());
            }
        };
        // Get the type of the second operand.
        let second_type = match self.get_type_from_operand(stack, branch.second())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => {
                return Err(CommandError::IncompatibleWithFuture("branch".into()).into());
            }
        };
        // Check that the operands have equivalent types.
        if !types_equivalent(stack, &first_type, stack, &second_type)? {
            return Err(CommandError::IncompatibleTypes(
                Branch::<N, VARIANT>::opcode().to_string(),
                first_type.to_string(),
                second_type.to_string(),
            )
            .into());
        }
        // Check that the `Position` has been defined.
        if positions.get(branch.position()).is_none() {
            return Err(CommandError::BranchUndefinedPosition(
                Branch::<N, VARIANT>::opcode().to_string(),
                branch.position().to_string(),
            )
            .into());
        }
        Ok(())
    }

    /// Ensures the given `contains` command is well-formed.
    #[inline]
    fn check_contains(&mut self, stack: &Stack<N>, contains: &Contains<N>) -> Result<(), FinalizeTypesInitError> {
        // Retrieve the mapping.
        let mapping = match contains.mapping() {
            CallOperator::Locator(locator) => {
                // Retrieve the program ID.
                let program_id = locator.program_id();
                // Retrieve the mapping_name.
                let mapping_name = locator.resource();

                // Ensure the locator does not reference the current program.
                if stack.program_id() == program_id {
                    return Err(FinalizeTypesInitError::LocatorInternal(locator.to_string()));
                }
                // Ensure the current program contains an import for this external program.
                if !stack.program().imports().keys().contains(program_id) {
                    return Err(FinalizeTypesInitError::MissingExternalImport(
                        program_id.to_string(),
                        stack.program_id().to_string(),
                    ));
                }
                // Retrieve the program.
                let external_stack = stack.get_external_stack(program_id)?;
                let external = external_stack.program();
                // Ensure the mapping exists in the program.
                if !external.contains_mapping(mapping_name) {
                    return Err(FinalizeTypesInitError::MappingUndefined(
                        mapping_name.to_string(),
                        program_id.to_string(),
                    ));
                }
                // Retrieve the mapping from the program.
                external.get_mapping(mapping_name)?
            }
            CallOperator::Resource(mapping_name) => {
                // Ensure the declared mapping in `contains` is defined in the current program.
                if !stack.program().contains_mapping(mapping_name) {
                    return Err(FinalizeTypesInitError::MappingUndefined(
                        mapping_name.to_string(),
                        stack.program_id().to_string(),
                    ));
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
            FinalizeType::Future(..) => {
                return Err(CommandError::IncompatibleWithFuture("contains".into()).into());
            }
        };
        // Check that the key type in the mapping is equivalent to the key type in the instruction.
        if !types_equivalent(stack, mapping_key_type, stack, &key_type)? {
            return Err(CommandError::MappingKeyTypeMismatch(
                "`contains`".into(),
                key_type.to_string(),
                mapping_key_type.to_string(),
            )
            .into());
        }
        // Get the destination register.
        let destination = contains.destination().clone();
        // Ensure the destination register is a locator (and does not reference an access).
        if !matches!(destination, Register::Locator(..)) {
            return Err(CommandError::DestinationNotALocator(destination.to_string()).into());
        }
        // Insert the destination register.
        self.add_destination(destination, FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::Boolean)))?;
        Ok(())
    }

    /// Ensures the given `get` command is well-formed.
    #[inline]
    fn check_get(&mut self, stack: &Stack<N>, get: &Get<N>) -> Result<(), FinalizeTypesInitError> {
        // Retrieve the mapping.
        let mapping = match get.mapping() {
            CallOperator::Locator(locator) => {
                // Retrieve the program ID.
                let program_id = locator.program_id();
                // Retrieve the mapping_name.
                let mapping_name = locator.resource();

                // Ensure the locator does not reference the current program.
                if stack.program_id() == program_id {
                    return Err(FinalizeTypesInitError::LocatorInternal(locator.to_string()));
                }
                // Ensure the current program contains an import for this external program.
                if !stack.program().imports().keys().contains(program_id) {
                    return Err(FinalizeTypesInitError::MissingExternalImport(
                        program_id.to_string(),
                        stack.program_id().to_string(),
                    ));
                }
                // Retrieve the program.
                let external_stack = stack.get_external_stack(program_id)?;
                let external = external_stack.program();
                // Ensure the mapping exists in the program.
                if !external.contains_mapping(mapping_name) {
                    return Err(FinalizeTypesInitError::MappingUndefined(
                        mapping_name.to_string(),
                        program_id.to_string(),
                    ));
                }
                // Retrieve the mapping from the program.
                external.get_mapping(mapping_name)?
            }
            CallOperator::Resource(mapping_name) => {
                // Ensure the declared mapping in `get` is defined in the current program.
                if !stack.program().contains_mapping(mapping_name) {
                    return Err(FinalizeTypesInitError::MappingUndefined(
                        mapping_name.to_string(),
                        stack.program_id().to_string(),
                    ));
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
            FinalizeType::Future(..) => {
                return Err(CommandError::IncompatibleWithFuture("get".into()).into());
            }
        };
        // Check that the key type in the mapping is equivalent to the key type in the instruction.
        if !types_equivalent(stack, mapping_key_type, stack, &key_type)? {
            return Err(CommandError::MappingKeyTypeMismatch(
                "get".into(),
                key_type.to_string(),
                mapping_key_type.to_string(),
            )
            .into());
        }
        // Get the destination register.
        let destination = get.destination().clone();
        // Ensure the destination register is a locator (and does not reference an access).
        if !matches!(destination, Register::Locator(..)) {
            return Err(CommandError::DestinationNotALocator(destination.to_string()).into());
        }
        // Insert the destination register.
        self.add_destination(destination, FinalizeType::Plaintext(mapping_value_type.clone()))?;
        Ok(())
    }

    /// Ensures the given `get.or_use` command is well-formed.
    #[inline]
    fn check_get_or_use(&mut self, stack: &Stack<N>, get_or_use: &GetOrUse<N>) -> Result<(), FinalizeTypesInitError> {
        // Retrieve the mapping.
        let mapping = match get_or_use.mapping() {
            CallOperator::Locator(locator) => {
                // Retrieve the program ID.
                let program_id = locator.program_id();
                // Retrieve the mapping_name.
                let mapping_name = locator.resource();

                // Ensure the locator does not reference the current program.
                if stack.program_id() == program_id {
                    return Err(FinalizeTypesInitError::LocatorInternal(locator.to_string()));
                }
                // Ensure the current program contains an import for this external program.
                if !stack.program().imports().keys().contains(program_id) {
                    return Err(FinalizeTypesInitError::MissingExternalImport(
                        program_id.to_string(),
                        stack.program_id().to_string(),
                    ));
                }
                // Retrieve the program.
                let external_stack = stack.get_external_stack(program_id)?;
                let external = external_stack.program();
                // Ensure the mapping exists in the program.
                if !external.contains_mapping(mapping_name) {
                    return Err(FinalizeTypesInitError::MappingUndefined(
                        mapping_name.to_string(),
                        program_id.to_string(),
                    ));
                }
                // Retrieve the mapping from the program.
                external.get_mapping(mapping_name)?
            }
            CallOperator::Resource(mapping_name) => {
                // Ensure the declared mapping in `get.or_use` is defined in the current program.
                if !stack.program().contains_mapping(mapping_name) {
                    return Err(FinalizeTypesInitError::MappingUndefined(
                        mapping_name.to_string(),
                        stack.program_id().to_string(),
                    ));
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
            FinalizeType::Future(..) => {
                return Err(CommandError::IncompatibleWithFuture("get.or_use".into()).into());
            }
        };
        // Check that the key type in the mapping is equivalent to the key type.
        if !types_equivalent(stack, mapping_key_type, stack, &key_type)? {
            return Err(CommandError::MappingKeyTypeMismatch(
                "get.or_use".into(),
                key_type.to_string(),
                mapping_key_type.to_string(),
            )
            .into());
        }
        // Retrieve the register type of the default value.
        let default_value_type = match self.get_type_from_operand(stack, get_or_use.default())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => {
                return Err(CommandError::DefaultValueFuture.into());
            }
        };
        // Check that the value type in the mapping is equivalent to the default value type.
        if !types_equivalent(stack, mapping_value_type, stack, &default_value_type)? {
            return Err(CommandError::MappingValueTypeMismatch(
                "get.or_use".into(),
                default_value_type.to_string(),
                mapping_value_type.to_string(),
            )
            .into());
        }
        // Get the destination register.
        let destination = get_or_use.destination().clone();
        // Ensure the destination register is a locator (and does not reference an access).
        if !matches!(destination, Register::Locator(..)) {
            return Err(CommandError::DestinationNotALocator(destination.to_string()).into());
        }
        // Insert the destination register.
        self.add_destination(destination, FinalizeType::Plaintext(default_value_type))?;
        Ok(())
    }

    /// Ensure the given `rand.chacha` command is well-formed.
    #[inline]
    fn check_rand_chacha(
        &mut self,
        _stack: &Stack<N>,
        rand_chacha: &RandChaCha<N>,
    ) -> Result<(), FinalizeTypesInitError> {
        // Ensure the number of operands is within bounds.
        if rand_chacha.operands().len() > MAX_ADDITIONAL_SEEDS {
            return Err(CommandError::TooManyOperands(MAX_ADDITIONAL_SEEDS).into());
        }

        // Get the destination register.
        let destination = rand_chacha.destination().clone();
        // Ensure the destination register is a locator (and does not reference an access).
        if !matches!(destination, Register::Locator(..)) {
            return Err(CommandError::DestinationNotALocator(destination.to_string()).into());
        }

        // Get the destination type.
        let destination_type = rand_chacha.destination_type();
        // Ensure the destination type is allowed.
        if matches!(destination_type, LiteralType::String) {
            return Err(CommandError::InvalidDestinationType(destination_type.to_string()).into());
        }

        // Insert the destination register.
        self.add_destination(destination, FinalizeType::Plaintext(PlaintextType::from(destination_type)))?;
        Ok(())
    }

    /// Ensures the given `set` command is well-formed.
    #[inline]
    fn check_set(&self, stack: &Stack<N>, set: &Set<N>) -> Result<(), FinalizeTypesInitError> {
        // Ensure the declared mapping in `set` is defined in the program.
        if !stack.program().contains_mapping(set.mapping_name()) {
            return Err(FinalizeTypesInitError::MappingUndefined(
                set.mapping_name().to_string(),
                stack.program_id().to_string(),
            ));
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
            FinalizeType::Future(..) => {
                return Err(CommandError::IncompatibleWithFuture("set".into()).into());
            }
        };
        // Check that the key type in the mapping is equivalent the key type.
        if !types_equivalent(stack, mapping_key_type, stack, &key_type)? {
            return Err(CommandError::MappingKeyTypeMismatch(
                "set".into(),
                key_type.to_string(),
                mapping_key_type.to_string(),
            )
            .into());
        }
        // Retrieve the type of the value.
        let value_type = match self.get_type_from_operand(stack, set.value())? {
            // If the register is a plaintext type, return it.
            FinalizeType::Plaintext(plaintext_type) => plaintext_type,
            // If the register is a future, throw an error.
            FinalizeType::Future(..) => {
                return Err(CommandError::IncompatibleWithFuture("set".into()).into());
            }
        };
        // Check that the value type in the mapping is equivalent the type of the value.
        if !types_equivalent(stack, mapping_value_type, stack, &value_type)? {
            return Err(CommandError::MappingValueTypeMismatch(
                "set".into(),
                value_type.to_string(),
                mapping_value_type.to_string(),
            )
            .into());
        }
        Ok(())
    }

    /// Ensures the given `remove` command is well-formed.
    #[inline]
    fn check_remove(&self, stack: &Stack<N>, remove: &Remove<N>) -> Result<(), FinalizeTypesInitError> {
        // Ensure the declared mapping in `remove` is defined in the program.
        if !stack.program().contains_mapping(remove.mapping_name()) {
            return Err(FinalizeTypesInitError::MappingUndefined(
                remove.mapping_name().to_string(),
                stack.program_id().to_string(),
            ));
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
            FinalizeType::Future(..) => {
                return Err(CommandError::IncompatibleWithFuture("remove".into()).into());
            }
        };
        // Check that the key type in the mapping is equivalent the key type.
        if !types_equivalent(stack, mapping_key_type, stack, &key_type)? {
            return Err(CommandError::MappingKeyTypeMismatch(
                "remove".into(),
                key_type.to_string(),
                mapping_key_type.to_string(),
            )
            .into());
        }
        Ok(())
    }

    /// Ensures the given instruction is well-formed.
    #[inline]
    fn check_instruction(
        &mut self,
        stack: &Stack<N>,
        instruction: &Instruction<N>,
    ) -> Result<(), FinalizeTypesInitError> {
        // Ensure the opcode is well-formed.
        self.check_instruction_opcode(stack, instruction)?;

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
            if !matches!(destination, Register::Locator(..)) {
                return Err(RegisterTypesInitError::NotALocator(destination.to_string()).into());
            }
            // Ensure that the destination type is a plaintext type.
            let destination_type = match destination_type {
                RegisterType::Plaintext(destination_type) => FinalizeType::Plaintext(destination_type),
                RegisterType::Future(locator) => FinalizeType::Future(locator),
                _ => {
                    return Err(CommandError::DestinationNotPlaintext(destination.to_string()).into());
                }
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
        stack: &Stack<N>,
        instruction: &Instruction<N>,
    ) -> Result<(), InstructionCheckError> {
        match instruction.opcode() {
            Opcode::Literal(opcode) => {
                // Ensure the opcode **is** a reserved opcode.
                if !Program::<N>::is_reserved_opcode(opcode) {
                    return Err(InstructionCheckError::OpcodeInvalid(opcode.to_string()));
                }
                // Ensure the instruction is not the cast operation.
                if matches!(instruction, Instruction::Cast(..)) {
                    return Err(InstructionCheckError::ContextDisallowed("cast".into()));
                }
                // Ensure the instruction has one destination register.
                if instruction.destinations().len() != 1 {
                    return Err(InstructionCheckError::MultipleDestinations(instruction.to_string()));
                }
            }
            Opcode::Assert(opcode) => match opcode {
                "assert.eq" => {
                    if !matches!(instruction, Instruction::AssertEq(..)) {
                        return Err(InstructionCheckError::OpcodeMismatch(instruction.to_string(), opcode.to_string()));
                    }
                }
                "assert.neq" => {
                    if !matches!(instruction, Instruction::AssertNeq(..)) {
                        return Err(InstructionCheckError::OpcodeMismatch(instruction.to_string(), opcode.to_string()));
                    }
                }
                _ => {
                    return Err(InstructionCheckError::OpcodeMismatch(instruction.to_string(), opcode.to_string()));
                }
            },
            Opcode::Async => {
                return Err(InstructionCheckError::ContextDisallowed("async".into()));
            }
            Opcode::Call => {
                return Err(InstructionCheckError::ContextDisallowed("call".into()));
            }
            Opcode::Cast(opcode) => match opcode {
                "cast" => {
                    // Retrieve the cast operation.
                    let operation = match instruction {
                        Instruction::Cast(operation) => operation,
                        _ => bail!("Instruction '{instruction}' is not a cast operation."),
                    };

                    // Ensure the instruction has one destination register.
                    if instruction.destinations().len() != 1 {
                        return Err(InstructionCheckError::MultipleDestinations(instruction.to_string()));
                    }

                    // Ensure the casted register type is defined.
                    match operation.cast_type() {
                        CastType::GroupXCoordinate
                        | CastType::GroupYCoordinate
                        | CastType::Plaintext(PlaintextType::Literal(..)) => {
                            return Err(CommandError::TooManyOperands(1).into());
                        }
                        CastType::Plaintext(plaintext @ PlaintextType::Struct(struct_name)) => {
                            // Ensure that the type is valid.
                            RegisterTypes::check_plaintext_type(stack, plaintext)?;
                            // Retrieve the struct.
                            let struct_ = stack.program().get_struct(struct_name)?;
                            // Ensure the operand types match the struct.
                            self.matches_struct(stack, instruction.operands(), struct_)?;
                        }
                        CastType::Plaintext(plaintext @ PlaintextType::ExternalStruct(locator)) => {
                            // Ensure that the type is valid.
                            RegisterTypes::check_plaintext_type(stack, plaintext)?;
                            let external_stack = stack.get_external_stack(locator.program_id())?;
                            let struct_name = locator.resource();
                            // Retrieve the struct.
                            let struct_ = external_stack.program().get_struct(struct_name)?;
                            // Ensure the operand types match the struct.
                            self.matches_struct(&*external_stack, instruction.operands(), struct_)?;
                        }
                        CastType::Plaintext(plaintext @ PlaintextType::Array(array_type)) => {
                            // Ensure that the type is valid.
                            RegisterTypes::check_plaintext_type(stack, plaintext)?;
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
                    if instruction.destinations().len() != 1 {
                        return Err(InstructionCheckError::MultipleDestinations(instruction.to_string()));
                    }

                    // Ensure the casted register type is valid and defined.
                    match operation.cast_type() {
                        CastType::Plaintext(PlaintextType::Literal(_)) => {
                            return Err(CommandError::TooManyOperands(1).into());
                        }
                        _ => bail!("`cast.lossy` is only supported for casting to a literal type."),
                    }
                }
                _ => {
                    return Err(InstructionCheckError::OpcodeMismatch(instruction.to_string(), opcode.to_string()));
                }
            },
            Opcode::Command(opcode) => {
                bail!("Fatal error: Cannot check command '{opcode}' as an instruction.")
            }
            Opcode::Commit(opcode) => RegisterTypes::check_commit_opcode(opcode, instruction)?,
            Opcode::Hash(opcode) => RegisterTypes::check_hash_opcode(opcode, instruction)?,
            Opcode::Is(opcode) => match opcode {
                "is.eq" => {
                    if !matches!(instruction, Instruction::IsEq(..)) {
                        return Err(InstructionCheckError::OpcodeMismatch(instruction.to_string(), opcode.to_string()));
                    }
                }
                "is.neq" => {
                    if !matches!(instruction, Instruction::IsNeq(..)) {
                        return Err(InstructionCheckError::OpcodeMismatch(instruction.to_string(), opcode.to_string()));
                    }
                }
                _ => {
                    return Err(InstructionCheckError::OpcodeMismatch(instruction.to_string(), opcode.to_string()));
                }
            },
            Opcode::Sign(_) => {
                // Ensure the instruction has one destination register.
                if instruction.destinations().len() != 1 {
                    return Err(InstructionCheckError::MultipleDestinations(instruction.to_string()));
                }
            }
            Opcode::ECDSA(opcode) => RegisterTypes::check_ecdsa_opcode(opcode, instruction)?,
            Opcode::Serialize(opcode) => RegisterTypes::check_serialize_opcode(opcode, instruction)?,
            Opcode::Deserialize(opcode) => RegisterTypes::check_deserialize_opcode(opcode, instruction)?,
        }
        Ok(())
    }

    // TODO (howardwu & d0cd): Reimplement this for cast and cast.lossy.
    // /// Checks the cast operation is well-formed.
    // fn check_cast_operation<const VARIANT: u8>(
    //     &self,
    //     stack: &impl StackTrait<N>,
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
