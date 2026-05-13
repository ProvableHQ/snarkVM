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

use circuit::Eject;
use console::program::{EntryType, FinalizeType, Identifier, Locator, RecordType};
use snarkvm_synthesizer_program::Program;

use super::*;

impl<N: Network, A: circuit::Aleo<Network = N>> RegistersCircuit<N, A> for Registers<N, A> {
    /// Returns the transition signer, as a circuit.
    #[inline]
    fn signer_circuit(&self) -> Result<circuit::Address<A>> {
        self.signer_circuit.clone().ok_or_else(|| anyhow!("Signer address (circuit) is not set in the registers."))
    }

    /// Sets the transition signer, as a circuit.
    #[inline]
    fn set_signer_circuit(&mut self, signer_circuit: circuit::Address<A>) {
        self.signer_circuit = Some(signer_circuit);
    }

    /// Returns the root transition view key, as a circuit.
    #[inline]
    fn root_tvk_circuit(&self) -> Result<circuit::Field<A>> {
        self.root_tvk_circuit.clone().ok_or_else(|| anyhow!("Root tvk (circuit) is not set in the registers."))
    }

    /// Sets the root transition view key, as a circuit.
    #[inline]
    fn set_root_tvk_circuit(&mut self, root_tvk_circuit: circuit::Field<A>) {
        self.root_tvk_circuit = Some(root_tvk_circuit);
    }

    /// Returns the transition caller, as a circuit.
    #[inline]
    fn caller_circuit(&self) -> Result<circuit::Address<A>> {
        self.caller_circuit.clone().ok_or_else(|| anyhow!("Caller address (circuit) is not set in the registers."))
    }

    /// Sets the transition caller, as a circuit.
    #[inline]
    fn set_caller_circuit(&mut self, caller_circuit: circuit::Address<A>) {
        self.caller_circuit = Some(caller_circuit);
    }

    /// Returns the transition view key, as a circuit.
    #[inline]
    fn tvk_circuit(&self) -> Result<circuit::Field<A>> {
        self.tvk_circuit.clone().ok_or_else(|| anyhow!("Transition view key (circuit) is not set in the registers."))
    }

    /// Sets the transition view key, as a circuit.
    #[inline]
    fn set_tvk_circuit(&mut self, tvk_circuit: circuit::Field<A>) {
        self.tvk_circuit = Some(tvk_circuit);
    }

    /// Loads the value of a given operand from the registers.
    ///
    /// # Errors
    /// This method will halt if the register locator is not found.
    /// In the case of register accesses, this method will halt if the access is not found.
    fn load_circuit(&self, stack: &impl StackTrait<N>, operand: &Operand<N>) -> Result<circuit::Value<A>> {
        use circuit::Inject;

        // Retrieve the register.
        let register = match operand {
            // If the operand is a literal, return the literal.
            Operand::Literal(literal) => {
                return Ok(circuit::Value::Plaintext(circuit::Plaintext::from(circuit::Literal::constant(
                    literal.clone(),
                ))));
            }
            // If the operand is a register, load the value from the register.
            Operand::Register(register) => register,
            // If the operand is the program ID, load the program address.
            Operand::ProgramID(program_id) => {
                return Ok(circuit::Value::Plaintext(circuit::Plaintext::from(circuit::Literal::constant(
                    Literal::Address(program_id.to_address()?),
                ))));
            }
            // If the operand is the signer, load the value of the signer.
            Operand::Signer => {
                return Ok(circuit::Value::Plaintext(circuit::Plaintext::from(circuit::Literal::Address(
                    self.signer_circuit()?,
                ))));
            }
            // If the operand is the caller, load the value of the caller.
            Operand::Caller => {
                return Ok(circuit::Value::Plaintext(circuit::Plaintext::from(circuit::Literal::Address(
                    self.caller_circuit()?,
                ))));
            }
            // If the operand is the generator, retrieve the Aleo generator.
            Operand::AleoGenerator => {
                return A::g_powers()
                    .first()
                    .map(|element| {
                        circuit::Value::Plaintext(circuit::Plaintext::from(circuit::Literal::Group(element.clone())))
                    })
                    .ok_or_else(|| anyhow!("Failed to retrieve the Aleo generator"));
            }
            // If the operand is the generator powers, retrieve the generator powers or the indexed group.
            Operand::AleoGeneratorPowers(index) => match index {
                None => {
                    return Ok(circuit::Value::Plaintext(circuit::Plaintext::Array(
                        A::g_powers()
                            .into_iter()
                            .map(|element| circuit::Plaintext::from(circuit::Literal::Group(element)))
                            .collect(),
                        OnceCell::new(),
                    )));
                }
                Some(index) => {
                    return A::g_powers()
                        .get(**index as usize)
                        .map(|element| {
                            circuit::Value::Plaintext(circuit::Plaintext::from(circuit::Literal::Group(
                                element.clone(),
                            )))
                        })
                        .ok_or_else(|| anyhow!("Index {index} out of bounds for Aleo generator"));
                }
            },
            // If the operand is the block height, throw an error.
            Operand::BlockHeight => bail!("Cannot load the block height in a non-finalize context"),
            // If the operand is the block timestamp, throw an error.
            Operand::BlockTimestamp => bail!("Cannot load the block timestamp in a non-finalize context"),
            // If the operand is the network ID, throw an error.
            Operand::NetworkID => bail!("Cannot load the network ID in a non-finalize context"),
            // If the operand is the checksum, throw an error.
            Operand::Checksum(_) => bail!("Cannot load the checksum in a non-finalize context."),
            // If the operand is the edition, throw an error.
            Operand::Edition(_) => bail!("Cannot load the edition in a non-finalize context"),
            // If the operand is the program owner, throw an error.
            Operand::ProgramOwner(_) => bail!("Cannot load the program owner in a non-finalize context"),
        };

        // Retrieve the circuit value.
        let circuit_value =
            self.circuit_registers.get(&register.locator()).ok_or_else(|| anyhow!("'{register}' does not exist"))?;

        // Return the value for the given register or register access.
        let circuit_value = match register {
            // If the register is a locator, then return the stack value.
            Register::Locator(..) => circuit_value.clone(),
            // If the register is a register access, then load the specific stack value.
            Register::Access(_, path) => {
                // Inject the path.
                let path = path.iter().map(|access| circuit::Access::constant(*access)).collect::<Vec<_>>();

                match circuit_value {
                    // Retrieve the plaintext member from the path.
                    circuit::Value::Plaintext(plaintext) => circuit::Value::Plaintext(plaintext.find(&path)?),
                    // Retrieve the record entry from the path.
                    circuit::Value::Record(record) => match record.find(&path)? {
                        circuit::Entry::Constant(plaintext)
                        | circuit::Entry::Public(plaintext)
                        | circuit::Entry::Private(plaintext) => circuit::Value::Plaintext(plaintext),
                    },
                    // Retrieve the argument from the future.
                    circuit::Value::Future(future) => future.find(&path)?,
                    // A dynamic record cannot be accessed directly.
                    circuit::Value::DynamicRecord(dynamic_record) => dynamic_record.find(&path)?,
                    // A dynamic future cannot be accessed directly.
                    circuit::Value::DynamicFuture(_) => {
                        bail!("Cannot invoke `find` on a dynamic future value")
                    }
                }
            }
        };

        // Retrieve the register type.
        match self.register_types.get_type(stack, register) {
            // Ensure the stack value matches the register type.
            Ok(register_type) => Self::circuit_matches_register_type(stack, &circuit_value, &register_type)?,
            // Ensure the register is defined.
            Err(error) => bail!("Register '{register}' is not a member of the function: {error}"),
        };

        Ok(circuit_value)
    }

    /// Assigns the given value to the given register, assuming the register is not already assigned.
    ///
    /// # Errors
    /// This method will halt if the given register is a register access.
    /// This method will halt if the given register is an input register.
    /// This method will halt if the register is already used.
    fn store_circuit(
        &mut self,
        stack: &impl StackTrait<N>,
        register: &Register<N>,
        circuit_value: circuit::Value<A>,
    ) -> Result<()> {
        match register {
            Register::Locator(locator) => {
                // Ensure the register assignments are monotonically increasing.
                let expected_locator = self.circuit_registers.len() as u64;
                ensure!(expected_locator == *locator, "Out-of-order write operation at '{register}'");
                // Ensure the register does not already exist.
                ensure!(
                    !self.circuit_registers.contains_key(locator),
                    "Cannot write to occupied register '{register}'"
                );

                // Ensure the register type is valid.
                match self.register_types.get_type(stack, register) {
                    // Ensure the stack value matches the register type.
                    Ok(register_type) => Self::circuit_matches_register_type(stack, &circuit_value, &register_type)?,
                    // Ensure the register is defined.
                    Err(error) => bail!("Register '{register}' is missing a type definition: {error}"),
                };

                // Store the stack value.
                match self.circuit_registers.insert(*locator, circuit_value) {
                    // Ensure the register has not been previously stored.
                    Some(..) => bail!("Attempted to write to register '{register}' again"),
                    // Return on success.
                    None => Ok(()),
                }
            }
            // Ensure the register is not a register access.
            Register::Access(..) => bail!("Cannot store to a register access: '{register}'"),
        }
    }

    /// Checks that the given circuit value matches the layout of the register type. This is a circuit analogue of [`StackTrait::matches_register_type`].
    fn circuit_matches_register_type(
        stack: &impl StackTrait<N>,
        circuit_value: &circuit::Value<A>,
        register_type: &RegisterType<N>,
    ) -> Result<()> {
        match (circuit_value, register_type) {
            (circuit::Value::Plaintext(plaintext), RegisterType::Plaintext(plaintext_type)) => {
                Self::circuit_matches_plaintext_internal(stack, plaintext, plaintext_type, 0)
            }
            (circuit::Value::Record(record), RegisterType::Record(record_name)) => {
                Self::circuit_matches_record(stack, record, record_name)
            }
            (circuit::Value::Record(record), RegisterType::ExternalRecord(locator)) => {
                Self::circuit_matches_external_record(stack, record, locator)
            }
            (circuit::Value::Future(future), RegisterType::Future(locator)) => {
                Self::circuit_matches_future(stack, future, locator)
            }
            (circuit::Value::DynamicRecord(_), RegisterType::DynamicRecord) => Ok(()),
            (circuit::Value::DynamicFuture(_), RegisterType::DynamicFuture) => Ok(()),
            (value, _) => {
                // Circuit values do not implement `Display`, so we report the variant kind instead.
                // For plaintext literals we additionally surface the literal type via `Literal::to_type()`.
                let value_kind = match value {
                    circuit::Value::Plaintext(circuit::Plaintext::Literal(literal, _)) => {
                        format!("plaintext literal of type '{}'", literal.to_type())
                    }
                    circuit::Value::Plaintext(circuit::Plaintext::Struct(..)) => "plaintext struct".to_string(),
                    circuit::Value::Plaintext(circuit::Plaintext::Array(..)) => "plaintext array".to_string(),
                    circuit::Value::Record(..) => "record".to_string(),
                    circuit::Value::Future(..) => "future".to_string(),
                    circuit::Value::DynamicRecord(..) => "dynamic record".to_string(),
                    circuit::Value::DynamicFuture(..) => "dynamic future".to_string(),
                };
                bail!("A circuit value ({value_kind}) does not match its declared register type '{register_type}'")
            }
        }
    }
}

impl<N: Network, A: circuit::Aleo<Network = N>> Registers<N, A> {
    // Checks that the given circuit plaintext matches the layout of the plaintext type. This is an analogue of
    // StackTrait's private method matches_plaintext_internal.
    fn circuit_matches_plaintext_internal(
        stack: &impl StackTrait<N>,
        circuit_plaintext: &circuit::Plaintext<A>,
        plaintext_type: &PlaintextType<N>,
        depth: usize,
    ) -> Result<()> {
        // If the depth exceeds the maximum depth, then the plaintext type is invalid.
        ensure!(depth <= N::MAX_DATA_DEPTH, "Plaintext exceeded maximum depth of {}", N::MAX_DATA_DEPTH);

        // Ensure the plaintext matches the plaintext definition in the program.
        match plaintext_type {
            PlaintextType::Literal(literal_type) => match circuit_plaintext {
                // If `plaintext` is a literal, it must match the literal type.
                circuit::Plaintext::Literal(literal, ..) => {
                    // Ensure the literal type matches.
                    match literal.to_type() == *literal_type {
                        true => Ok(()),
                        false => bail!("'{literal}' is invalid: expected {literal_type}"),
                    }
                }
                // If `plaintext` is a struct, this is a mismatch.
                circuit::Plaintext::Struct(..) => {
                    bail!("'{plaintext_type}' is invalid: expected literal, found struct")
                }
                // If `plaintext` is an array, this is a mismatch.
                circuit::Plaintext::Array(..) => bail!("'{plaintext_type}' is invalid: expected literal, found array"),
            },
            PlaintextType::ExternalStruct(locator) => {
                let external_stack = stack.get_external_stack(locator.program_id())?;
                let new_type = PlaintextType::Struct(*locator.resource());
                Self::circuit_matches_plaintext_internal(&*external_stack, circuit_plaintext, &new_type, depth)
            }
            PlaintextType::Struct(struct_name) => {
                // Ensure the struct name is valid.
                ensure!(!Program::is_reserved_keyword(struct_name), "Struct '{struct_name}' is reserved");

                // Retrieve the struct from the program.
                let Ok(struct_) = stack.program().get_struct(struct_name) else {
                    bail!("Struct '{struct_name}' is not defined in the program")
                };

                // Ensure the struct name matches.
                if struct_.name() != struct_name {
                    bail!("Expected struct '{struct_name}', found struct '{}'", struct_.name())
                }

                // Retrieve the struct members.
                let members = match circuit_plaintext {
                    circuit::Plaintext::Literal(..) => {
                        bail!("'{struct_name}' is invalid: expected struct, found literal")
                    }
                    circuit::Plaintext::Struct(members, ..) => members,
                    circuit::Plaintext::Array(..) => bail!("'{struct_name}' is invalid: expected struct, found array"),
                };

                let num_members = members.len();
                // Ensure the number of struct members does not go below the minimum.
                ensure!(
                    num_members >= N::MIN_STRUCT_ENTRIES,
                    "'{struct_name}' cannot be less than {} entries",
                    N::MIN_STRUCT_ENTRIES
                );
                // Ensure the number of struct members does not exceed the maximum.
                ensure!(
                    num_members <= N::MAX_STRUCT_ENTRIES,
                    "'{struct_name}' cannot exceed {} entries",
                    N::MAX_STRUCT_ENTRIES
                );

                // Ensure the number of struct members match.
                let expected_num_members = struct_.members().len();
                if expected_num_members != num_members {
                    bail!("'{struct_name}' expected {expected_num_members} members, found {num_members} members")
                }

                // Ensure the struct members match, in the same order.
                for (i, ((expected_name, expected_type), (member_name, member))) in
                    struct_.members().iter().zip_eq(members.iter()).enumerate()
                {
                    let ejected_member_name = member_name.eject_value();

                    // Ensure the member name matches.
                    if expected_name != &ejected_member_name {
                        bail!(
                            "Member '{i}' in '{struct_name}' is incorrect: expected '{expected_name}', found '{member_name}'"
                        )
                    }
                    // Ensure the member name is valid.
                    ensure!(
                        !Program::is_reserved_keyword(&ejected_member_name),
                        "Member name '{member_name}' is reserved"
                    );
                    // Ensure the member plaintext matches (recursive call).
                    Self::circuit_matches_plaintext_internal(stack, member, expected_type, depth + 1)?;
                }

                Ok(())
            }
            PlaintextType::Array(array_type) => match circuit_plaintext {
                // If `plaintext` is a literal, this is a mismatch.
                circuit::Plaintext::Literal(..) => {
                    bail!("'{plaintext_type}' is invalid: expected array, found literal")
                }
                // If `plaintext` is a struct, this is a mismatch.
                circuit::Plaintext::Struct(..) => bail!("'{plaintext_type}' is invalid: expected array, found struct"),
                // If `plaintext` is an array, it must match the array type.
                circuit::Plaintext::Array(array, ..) => {
                    // Ensure the array length matches.
                    let (actual_length, expected_length) = (array.len(), array_type.length());
                    if **expected_length as usize != actual_length {
                        bail!(
                            "'{plaintext_type}' is invalid: expected {expected_length} elements, found {actual_length} elements"
                        )
                    }
                    // Ensure the array elements match.
                    for element in array.iter() {
                        Self::circuit_matches_plaintext_internal(
                            stack,
                            element,
                            array_type.next_element_type(),
                            depth + 1,
                        )?;
                    }
                    Ok(())
                }
            },
        }
    }

    // Checks that the given circuit future matches the layout of the future type. This is an analogue of
    // StackTrait's private method matches_future.
    fn circuit_matches_future(
        stack: &impl StackTrait<N>,
        future: &circuit::Future<A>,
        locator: &Locator<N>,
    ) -> Result<()> {
        Self::circuit_matches_future_internal(stack, future, locator, 0)
    }

    // Checks that the given circuit future matches the layout of the future type. This is an analogue of
    // StackTrait's private method matches_future_internal.
    fn circuit_matches_future_internal(
        stack: &impl StackTrait<N>,
        future: &circuit::Future<A>,
        locator: &Locator<N>,
        depth: usize,
    ) -> Result<()> {
        // If the depth exceeds the maximum depth, then the future type is invalid.
        ensure!(depth <= N::MAX_DATA_DEPTH, "Future exceeded maximum depth of {}", N::MAX_DATA_DEPTH);

        // Ensure that the program IDs match.
        ensure!(&Eject::eject_value(future.program_id()) == locator.program_id(), "Future program ID does not match");

        // Ensure that the function names match.
        ensure!(&Eject::eject_value(future.function_name()) == locator.resource(), "Future name does not match");

        // Retrieve the external stack, if needed.
        let external_stack = match locator.program_id() == stack.program_id() {
            true => None,
            // Attention - This method must fail here and early return if the external program is missing.
            // Otherwise, this method will proceed to look for the requested function in its own program.
            false => Some(stack.get_external_stack(locator.program_id())?),
        };
        // Retrieve the associated function.
        let function = match &external_stack {
            Some(external_stack) => external_stack.get_function_ref(locator.resource())?,
            None => stack.get_function_ref(locator.resource())?,
        };
        // Retrieve the finalize inputs.
        let inputs = match function.finalize_logic() {
            Some(finalize_logic) => finalize_logic.inputs(),
            None => bail!("Function '{locator}' does not have a finalize block"),
        };

        // Ensure the number of arguments matches the number of inputs.
        ensure!(future.arguments().len() == inputs.len(), "Future arguments do not match");

        // Check that the arguments match the inputs.
        // Use the external stack if the future is from an external program.
        for (argument, input) in future.arguments().iter().zip_eq(inputs.iter()) {
            match (argument, input.finalize_type()) {
                (circuit::Argument::Plaintext(plaintext), FinalizeType::Plaintext(plaintext_type)) => {
                    match &external_stack {
                        Some(external_stack) => Self::circuit_matches_plaintext_internal(
                            &**external_stack,
                            plaintext,
                            plaintext_type,
                            depth + 1,
                        )?,
                        None => Self::circuit_matches_plaintext_internal(stack, plaintext, plaintext_type, depth + 1)?,
                    }
                }
                (circuit::Argument::Future(future), FinalizeType::Future(locator)) => match &external_stack {
                    Some(external_stack) => {
                        Self::circuit_matches_future_internal(&**external_stack, future, locator, depth + 1)?
                    }
                    None => Self::circuit_matches_future_internal(stack, future, locator, depth + 1)?,
                },
                (circuit::Argument::DynamicFuture(_), FinalizeType::DynamicFuture) => {}
                (_, input_type) => {
                    bail!("Argument type does not match input type: expected '{input_type}'")
                }
            }
        }

        Ok(())
    }

    // Checks that the given circuit record matches the layout of the record type. This is an analogue of
    // StackTrait's private method matches_record.
    fn circuit_matches_record(
        stack: &impl StackTrait<N>,
        record: &circuit::Record<A, circuit::Plaintext<A>>,
        record_name: &Identifier<N>,
    ) -> Result<()> {
        // Ensure the record name is valid.
        ensure!(!Program::is_reserved_keyword(record_name), "Record name '{record_name}' is reserved");

        // Retrieve the record type from the program.
        let Ok(record_type) = stack.program().get_record(record_name) else {
            bail!("Record '{record_name}' is not defined in the program")
        };

        // Ensure the record name matches.
        if record_type.name() != record_name {
            bail!("Expected record '{record_name}', found record '{}'", record_type.name())
        }
        Self::circuit_matches_record_internal(stack, record, record_type, 0)
    }

    // Checks that the given circuit record matches the layout of the external record type. This is an analogue of
    // StackTrait's private method matches_external_record.
    fn circuit_matches_external_record(
        stack: &impl StackTrait<N>,
        record: &circuit::Record<A, circuit::Plaintext<A>>,
        locator: &Locator<N>,
    ) -> Result<()> {
        // Retrieve the record name.
        let record_name = locator.resource();

        // Ensure the record name is valid.
        ensure!(!Program::is_reserved_keyword(record_name), "Record name '{record_name}' is reserved");

        // Retrieve the external stack.
        let external_stack = stack.get_external_stack(locator.program_id())?;
        // Retrieve the record type from the program.
        let Ok(record_type) = external_stack.program().get_record(locator.resource()) else {
            bail!("External '{locator}' is not defined in the program")
        };

        // Ensure the record name matches.
        if record_type.name() != record_name {
            bail!("Expected external record '{record_name}', found external record '{}'", record_type.name())
        }
        Self::circuit_matches_record_internal(&*external_stack, record, record_type, 0)
    }

    // Checks that the given circuit record matches the layout of the record type. This is an analogue of
    // StackTrait's private method matches_record_internal.
    fn circuit_matches_record_internal(
        stack: &impl StackTrait<N>,
        record: &circuit::Record<A, circuit::Plaintext<A>>,
        record_type: &RecordType<N>,
        depth: usize,
    ) -> Result<()> {
        // If the depth exceeds the maximum depth, then the plaintext type is invalid.
        ensure!(depth <= N::MAX_DATA_DEPTH, "Plaintext exceeded maximum depth of {}", N::MAX_DATA_DEPTH);

        // Retrieve the record name.
        let record_name = record_type.name();
        // Ensure the record name is valid.
        ensure!(!Program::is_reserved_keyword(record_name), "Record name '{record_name}' is reserved");

        // Ensure the visibility of the record owner matches the visibility in the record type.
        ensure!(
            record.owner().is_public().eject_value() == record_type.owner().is_public(),
            "Visibility of record entry 'owner' does not match"
        );
        ensure!(
            record.owner().is_private().eject_value() == record_type.owner().is_private(),
            "Visibility of record entry 'owner' does not match"
        );

        // Ensure the number of record entries does not exceed the maximum.
        let num_entries = record.data().len();
        ensure!(num_entries <= N::MAX_DATA_ENTRIES, "'{record_name}' cannot exceed {} entries", N::MAX_DATA_ENTRIES);

        // Ensure the number of record entries match.
        let expected_num_entries = record_type.entries().len();
        if expected_num_entries != num_entries {
            bail!("'{record_name}' expected {expected_num_entries} entries, found {num_entries} entries")
        }

        // Ensure the record data match, in the same order.
        for (i, ((expected_name, expected_type), (entry_name, entry))) in
            record_type.entries().iter().zip_eq(record.data().iter()).enumerate()
        {
            let ejected_entry_name = entry_name.eject_value();

            // Ensure the entry name matches.
            if expected_name != &ejected_entry_name {
                bail!("Entry '{i}' in '{record_name}' is incorrect: expected '{expected_name}', found '{entry_name}'")
            }
            // Ensure the entry name is valid.
            ensure!(!Program::is_reserved_keyword(&ejected_entry_name), "Entry name '{entry_name}' is reserved");
            // Ensure the entry matches (recursive call).
            Self::circuit_matches_entry_internal(stack, record_name, entry_name, entry, expected_type, depth + 1)?;
        }

        Ok(())
    }

    // Checks that the given circuit entry matches the layout of the entry type.
    fn circuit_matches_entry_internal(
        stack: &impl StackTrait<N>,
        record_name: &Identifier<N>,
        entry_name: &circuit::Identifier<A>,
        entry: &circuit::Entry<A, circuit::Plaintext<A>>,
        entry_type: &EntryType<N>,
        depth: usize,
    ) -> Result<()> {
        match (entry, entry_type) {
            (circuit::Entry::Constant(plaintext), EntryType::Constant(plaintext_type))
            | (circuit::Entry::Public(plaintext), EntryType::Public(plaintext_type))
            | (circuit::Entry::Private(plaintext), EntryType::Private(plaintext_type)) => {
                match Self::circuit_matches_plaintext_internal(stack, plaintext, plaintext_type, depth) {
                    Ok(()) => Ok(()),
                    Err(error) => bail!("Invalid record entry '{record_name}.{entry_name}': {error}"),
                }
            }
            _ => bail!(
                "Type mismatch in record entry '{record_name}.{entry_name}': value does not match\n'{entry_type}'"
            ),
        }
    }
}
