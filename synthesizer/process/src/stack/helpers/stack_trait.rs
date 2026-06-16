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

impl<N: Network> StackTrait<N> for Stack<N> {
    /// Checks that the given value matches the layout of the value type.
    fn matches_value_type(&self, value: &Value<N>, value_type: &ValueType<N>) -> Result<()> {
        // Ensure the value matches the declared value type in the register.
        match (value, value_type) {
            (Value::Plaintext(plaintext), ValueType::Constant(plaintext_type))
            | (Value::Plaintext(plaintext), ValueType::Public(plaintext_type))
            | (Value::Plaintext(plaintext), ValueType::Private(plaintext_type)) => {
                self.matches_plaintext(plaintext, plaintext_type)
            }
            (Value::Record(record), ValueType::Record(record_name)) => self.matches_record(record, record_name),
            (Value::Record(record), ValueType::ExternalRecord(locator)) => {
                self.matches_external_record(record, locator)
            }
            (Value::Future(future), ValueType::Future(locator)) => self.matches_future(future, locator),
            (Value::DynamicRecord(_), ValueType::DynamicRecord) => Ok(()),
            (Value::DynamicFuture(_), ValueType::DynamicFuture) => Ok(()),
            (value, _) => bail!("A value '{value}' does not match its declared value type '{value_type}'"),
        }
    }

    /// Checks that the given stack value matches the layout of the register type.
    fn matches_register_type(&self, stack_value: &Value<N>, register_type: &RegisterType<N>) -> Result<()> {
        match (stack_value, register_type) {
            (Value::Plaintext(plaintext), RegisterType::Plaintext(plaintext_type)) => {
                self.matches_plaintext(plaintext, plaintext_type)
            }
            (Value::Record(record), RegisterType::Record(record_name)) => self.matches_record(record, record_name),
            (Value::Record(record), RegisterType::ExternalRecord(locator)) => {
                self.matches_external_record(record, locator)
            }
            (Value::Future(future), RegisterType::Future(locator)) => self.matches_future(future, locator),
            (Value::DynamicRecord(_), RegisterType::DynamicRecord) => Ok(()),
            (Value::DynamicFuture(_), RegisterType::DynamicFuture) => Ok(()),
            (value, _) => bail!("A value '{value}' does not match its declared register type '{register_type}'"),
        }
    }

    /// Checks that the given record matches the layout of the external record type.
    fn matches_external_record(&self, record: &Record<N, Plaintext<N>>, locator: &Locator<N>) -> Result<()> {
        // Retrieve the record name.
        let record_name = locator.resource();

        // Ensure the record name is valid.
        ensure!(!Program::is_reserved_keyword(record_name), "Record name '{record_name}' is reserved");

        // Retrieve the external stack.
        let external_stack = self.get_external_stack(locator.program_id())?;
        // Retrieve the record type from the program.
        let Ok(record_type) = external_stack.program().get_record(locator.resource()) else {
            bail!("External '{locator}' is not defined in the program")
        };

        // Ensure the record name matches.
        if record_type.name() != record_name {
            bail!("Expected external record '{record_name}', found external record '{}'", record_type.name())
        }

        external_stack.matches_record_internal(record, record_type, 0)
    }

    /// Checks that the given record matches the layout of the record type.
    fn matches_record(&self, record: &Record<N, Plaintext<N>>, record_name: &Identifier<N>) -> Result<()> {
        // Ensure the record name is valid.
        ensure!(!Program::is_reserved_keyword(record_name), "Record name '{record_name}' is reserved");

        // Retrieve the record type from the program.
        let Ok(record_type) = self.program().get_record(record_name) else {
            bail!("Record '{record_name}' is not defined in the program")
        };

        // Ensure the record name matches.
        if record_type.name() != record_name {
            bail!("Expected record '{record_name}', found record '{}'", record_type.name())
        }

        self.matches_record_internal(record, record_type, 0)
    }

    /// Checks that the given plaintext matches the layout of the plaintext type.
    fn matches_plaintext(&self, plaintext: &Plaintext<N>, plaintext_type: &PlaintextType<N>) -> Result<()> {
        self.matches_plaintext_internal(plaintext, plaintext_type, 0)
    }

    /// Checks that the given future matches the layout of the future type.
    fn matches_future(&self, future: &Future<N>, locator: &Locator<N>) -> Result<()> {
        self.matches_future_internal(future, locator, 0)
    }

    /// Returns `true` if the proving key for the given name exists.
    /// The name can be a function name or a record name (for translation keys).
    fn contains_proving_key(&self, function_or_record_name: &Identifier<N>) -> bool {
        self.proving_keys.read().contains_key(function_or_record_name)
    }

    /// Returns the proving key for the given name.
    /// The name can be a function name or a record name (for translation keys).
    fn get_proving_key(&self, function_or_record_name: &Identifier<N>) -> Result<ProvingKey<N>> {
        // If the program is 'credits.aleo', try to load the proving key, if it does not exist.
        self.try_insert_credits_function_proving_key(function_or_record_name)?;
        // Return the proving key, if it exists.
        match self.proving_keys.read().get(function_or_record_name) {
            Some(pk) => Ok(pk.clone()),
            None => bail!("Proving key not found for: {}/{}", self.program.id(), function_or_record_name),
        }
    }

    /// Inserts the given proving key for the given name.
    /// The name can be a function name or a record name (for translation keys).
    fn insert_proving_key(&self, function_or_record_name: &Identifier<N>, proving_key: ProvingKey<N>) -> Result<()> {
        // Ensure the name exists in the program as a function or record.
        ensure!(
            self.program.contains_function(function_or_record_name)
                || self.program.contains_record(function_or_record_name),
            "'{function_or_record_name}' does not exist as a function or record in program '{}'.",
            self.program.id()
        );
        // Insert the proving key.
        self.proving_keys.write().insert(*function_or_record_name, proving_key);
        Ok(())
    }

    /// Removes the proving key for the given name.
    /// The name can be a function name or a record name (for translation keys).
    fn remove_proving_key(&self, function_or_record_name: &Identifier<N>) {
        self.proving_keys.write().shift_remove(function_or_record_name);
    }

    /// Returns `true` if the verifying key for the given name exists.
    /// The name can be a function name or a record name (for translation keys).
    fn contains_verifying_key(&self, function_or_record_name: &Identifier<N>) -> bool {
        self.verifying_keys.read().contains_key(function_or_record_name)
    }

    /// Returns the verifying key for the given name.
    /// The name can be a function name or a record name (for translation keys).
    fn get_verifying_key(&self, function_or_record_name: &Identifier<N>) -> Result<VerifyingKey<N>> {
        // Return the verifying key, if it exists.
        match self.verifying_keys.read().get(function_or_record_name) {
            Some(vk) => Ok(vk.clone()),
            None => bail!("Verifying key not found for: {}/{}", self.program.id(), function_or_record_name),
        }
    }

    /// Inserts the given verifying key for the given name.
    /// The name can be a function name or a record name (for translation keys).
    fn insert_verifying_key(
        &self,
        function_or_record_name: &Identifier<N>,
        verifying_key: VerifyingKey<N>,
    ) -> Result<()> {
        // Ensure the name exists in the program as a function or record.
        ensure!(
            self.program.contains_function(function_or_record_name)
                || self.program.contains_record(function_or_record_name),
            "'{function_or_record_name}' does not exist as a function or record in program '{}'.",
            self.program.id()
        );
        // Insert the verifying key.
        self.verifying_keys.write().insert(*function_or_record_name, verifying_key);
        Ok(())
    }

    /// Removes the verifying key for the given name.
    /// The name can be a function name or a record name (for translation keys).
    fn remove_verifying_key(&self, function_or_record_name: &Identifier<N>) {
        self.verifying_keys.write().shift_remove(function_or_record_name);
    }

    /// Returns the program.
    fn program(&self) -> &Program<N> {
        &self.program
    }

    /// Returns the program ID.
    fn program_id(&self) -> &ProgramID<N> {
        self.program.id()
    }

    /// Returns the program address.
    fn program_address(&self) -> &Address<N> {
        &self.program_address
    }

    /// Returns the program checksum.
    fn program_checksum(&self) -> &[U8<N>; 32] {
        &self.program_checksum
    }

    /// Returns the program checksum as a field element.
    #[inline]
    fn program_checksum_as_field(&self) -> Result<Field<N>> {
        // Get the bits of the program checksum, truncated to the field size.
        let bits = self
            .program_checksum
            .iter()
            .flat_map(|byte| byte.to_bits_le())
            .take(Field::<N>::SIZE_IN_DATA_BITS)
            .collect::<Vec<_>>();
        // Return the field element from the bits.
        Field::from_bits_le(&bits)
    }

    /// Returns the checksum of the program component (function, closure, or view) with the given name.
    fn component_checksum(&self, name: &Identifier<N>) -> Result<&[U8<N>; 32]> {
        self.component_checksums
            .get(name)
            .ok_or_else(|| anyhow!("'{name}' is not a function, closure, or view in '{}'.", self.program_id()))
    }

    /// Returns the program edition.
    #[inline]
    fn program_edition(&self) -> U16<N> {
        self.program_edition
    }

    /// Returns the number of amendments for the current program edition.
    #[inline]
    fn program_amendment_count(&self) -> u64 {
        self.program_amendment_count
    }

    /// Sets the number of amendments for the current program edition.
    fn set_program_amendment_count(&mut self, program_amendment_count: u64) {
        self.program_amendment_count = program_amendment_count;
    }

    /// Returns the program owner.
    #[inline]
    fn program_owner(&self) -> &Option<Address<N>> {
        &self.program_owner
    }

    /// Sets the program owner.
    /// The program owner should only be set for programs that are deployed after `ConsensusVersion::V9` is active.
    fn set_program_owner(&mut self, program_owner: Option<Address<N>>) {
        self.program_owner = program_owner;
    }

    /// Returns the external stack for the given program ID.
    ///
    /// Attention - this function is used to check the existence of the external program.
    /// Developers should explicitly handle the error case so as to not default to the main program.
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

    /// Returns the stack for the given program ID.
    ///
    /// Attention - this function does **NOT** check that the program is imported by the current program.
    /// This function is only to be used for resolution during dynamic dispatch.
    fn get_stack_global(&self, program_id: &ProgramID<N>) -> Result<Arc<Stack<N>>> {
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
    fn get_function(&self, function_name: &Identifier<N>) -> Result<Function<N>> {
        self.program.get_function(function_name)
    }

    /// Returns a reference to the function with the given function name.
    fn get_function_ref(&self, function_name: &Identifier<N>) -> Result<&Function<N>> {
        self.program.get_function_ref(function_name)
    }

    /// Returns the minimum number of calls for the given function name.
    /// In the case where there are no dynamic calls, this is equivalent to the total number of calls.
    fn get_minimum_number_of_calls(&self, function_name: &Identifier<N>) -> Result<usize> {
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
                match instruction {
                    Instruction::Call(call) => {
                        // Determine if this is a function call.
                        if call.is_function_call(&*stack_ref)? {
                            // Increment the number of calls.
                            num_calls += 1;
                            // Add the function to the queue.
                            match call.operator() {
                                CallOperator::Locator(locator) => {
                                    // If the locator matches the program ID of the provided stack, use it directly.
                                    // Otherwise, retrieve the external stack.
                                    let stack = if locator.program_id() == self.program().id() {
                                        StackRef::Internal(self)
                                    } else {
                                        StackRef::External(stack_ref.get_external_stack(locator.program_id())?)
                                    };
                                    queue.push((stack, *locator.resource()));
                                }
                                CallOperator::Resource(resource) => {
                                    queue.push((stack_ref.clone(), *resource));
                                }
                            }
                        }
                    }
                    Instruction::CallDynamic(_) => {
                        // Increment the number of calls.
                        num_calls += 1
                    }
                    _ => (),
                }
            }
        }
        // Return the number of calls.
        Ok(num_calls)
    }

    /// Returns whether or not a function has a dynamic call in its execution.
    fn contains_dynamic_call(&self, function_name: &Identifier<N>) -> Result<bool> {
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
                match instruction {
                    Instruction::Call(call) => {
                        // Determine if this is a function call.
                        if call.is_function_call(&*stack_ref)? {
                            // Increment by the number of calls.
                            num_calls += 1;
                            // Add the function to the queue.
                            match call.operator() {
                                CallOperator::Locator(locator) => {
                                    // If the locator matches the program ID of the provided stack, use it directly.
                                    // Otherwise, retrieve the external stack.
                                    let stack = if locator.program_id() == self.program().id() {
                                        StackRef::Internal(self)
                                    } else {
                                        StackRef::External(stack_ref.get_external_stack(locator.program_id())?)
                                    };
                                    queue.push((stack, *locator.resource()));
                                }
                                CallOperator::Resource(resource) => {
                                    queue.push((stack_ref.clone(), *resource));
                                }
                            }
                        }
                    }
                    Instruction::CallDynamic(_) => return Ok(true),
                    _ => (),
                }
            }
        }
        // No dynamic calls have been found.
        Ok(false)
    }

    /// Returns a value for the given register type.
    fn sample_value<R: Rng + CryptoRng>(
        &self,
        burner_address: &Address<N>,
        register_type: &RegisterType<N>,
        rng: &mut R,
    ) -> Result<Value<N>> {
        match register_type {
            RegisterType::Plaintext(plaintext_type) => {
                Ok(Value::Plaintext(self.sample_plaintext(plaintext_type, rng)?))
            }
            RegisterType::Record(record_name) => {
                Ok(Value::Record(self.sample_record(burner_address, record_name, Group::rand(rng), rng)?))
            }
            RegisterType::ExternalRecord(locator) => {
                // Retrieve the external stack.
                let stack = self.get_external_stack(locator.program_id())?;
                // Sample the output.
                Ok(Value::Record(stack.sample_record(burner_address, locator.resource(), Group::rand(rng), rng)?))
            }
            RegisterType::Future(locator) => Ok(Value::Future(self.sample_future(locator, rng)?)),
            RegisterType::DynamicRecord => Ok(Value::DynamicRecord(self.sample_dynamic_record(rng)?)),
            RegisterType::DynamicFuture => Ok(Value::DynamicFuture(self.sample_dynamic_future(rng)?)),
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

    fn evaluate_view(
        &self,
        state: FinalizeGlobalState,
        store: &dyn FinalizeStoreTrait<N>,
        view_name: &Identifier<N>,
        inputs: Vec<Value<N>>,
    ) -> Result<Vec<Value<N>>> {
        crate::view::evaluate_view_inner(state, store, self, view_name, inputs)
    }
}

impl<N: Network> Stack<N> {
    /// Checks that the given record matches the layout of the record type.
    fn matches_record_internal(
        &self,
        record: &Record<N, Plaintext<N>>,
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
            record.owner().is_public() == record_type.owner().is_public(),
            "Visibility of record entry 'owner' does not match"
        );
        ensure!(
            record.owner().is_private() == record_type.owner().is_private(),
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
            // Ensure the entry name matches.
            if expected_name != entry_name {
                bail!("Entry '{i}' in '{record_name}' is incorrect: expected '{expected_name}', found '{entry_name}'")
            }
            // Ensure the entry name is valid.
            ensure!(!Program::is_reserved_keyword(entry_name), "Entry name '{entry_name}' is reserved");
            // Ensure the entry matches (recursive call).
            self.matches_entry_internal(record_name, entry_name, entry, expected_type, depth + 1)?;
        }

        Ok(())
    }

    /// Checks that the given entry matches the layout of the entry type.
    fn matches_entry_internal(
        &self,
        record_name: &Identifier<N>,
        entry_name: &Identifier<N>,
        entry: &Entry<N, Plaintext<N>>,
        entry_type: &EntryType<N>,
        depth: usize,
    ) -> Result<()> {
        match (entry, entry_type) {
            (Entry::Constant(plaintext), EntryType::Constant(plaintext_type))
            | (Entry::Public(plaintext), EntryType::Public(plaintext_type))
            | (Entry::Private(plaintext), EntryType::Private(plaintext_type)) => {
                match self.matches_plaintext_internal(plaintext, plaintext_type, depth) {
                    Ok(()) => Ok(()),
                    Err(error) => bail!("Invalid record entry '{record_name}.{entry_name}': {error}"),
                }
            }
            _ => bail!(
                "Type mismatch in record entry '{record_name}.{entry_name}':\n'{entry}'\n does not match\n'{entry_type}'"
            ),
        }
    }

    /// Checks that the given plaintext matches the layout of the plaintext type.
    fn matches_plaintext_internal(
        &self,
        plaintext: &Plaintext<N>,
        plaintext_type: &PlaintextType<N>,
        depth: usize,
    ) -> Result<()> {
        // If the depth exceeds the maximum depth, then the plaintext type is invalid.
        ensure!(depth <= N::MAX_DATA_DEPTH, "Plaintext exceeded maximum depth of {}", N::MAX_DATA_DEPTH);

        // Ensure the plaintext matches the plaintext definition in the program.
        match plaintext_type {
            PlaintextType::Literal(literal_type) => match plaintext {
                // If `plaintext` is a literal, it must match the literal type.
                Plaintext::Literal(literal, ..) => {
                    // Ensure the literal type matches.
                    match literal.to_type() == *literal_type {
                        true => Ok(()),
                        false => bail!("'{literal}' is invalid: expected {literal_type}"),
                    }
                }
                // If `plaintext` is a struct, this is a mismatch.
                Plaintext::Struct(..) => bail!("'{plaintext_type}' is invalid: expected literal, found struct"),
                // If `plaintext` is an array, this is a mismatch.
                Plaintext::Array(..) => bail!("'{plaintext_type}' is invalid: expected literal, found array"),
            },
            PlaintextType::ExternalStruct(locator) => {
                let external_stack = self.get_external_stack(locator.program_id())?;
                let new_type = PlaintextType::Struct(*locator.resource());
                external_stack.matches_plaintext_internal(plaintext, &new_type, depth)
            }
            PlaintextType::Struct(struct_name) => {
                // Ensure the struct name is valid.
                ensure!(!Program::is_reserved_keyword(struct_name), "Struct '{struct_name}' is reserved");

                // Retrieve the struct from the program.
                let Ok(struct_) = self.program().get_struct(struct_name) else {
                    bail!("Struct '{struct_name}' is not defined in the program")
                };

                // Ensure the struct name matches.
                if struct_.name() != struct_name {
                    bail!("Expected struct '{struct_name}', found struct '{}'", struct_.name())
                }

                // Retrieve the struct members.
                let members = match plaintext {
                    Plaintext::Literal(..) => bail!("'{struct_name}' is invalid: expected struct, found literal"),
                    Plaintext::Struct(members, ..) => members,
                    Plaintext::Array(..) => bail!("'{struct_name}' is invalid: expected struct, found array"),
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
                    // Ensure the member name matches.
                    if expected_name != member_name {
                        bail!(
                            "Member '{i}' in '{struct_name}' is incorrect: expected '{expected_name}', found '{member_name}'"
                        )
                    }
                    // Ensure the member name is valid.
                    ensure!(!Program::is_reserved_keyword(member_name), "Member name '{member_name}' is reserved");
                    // Ensure the member plaintext matches (recursive call).
                    self.matches_plaintext_internal(member, expected_type, depth + 1)?;
                }

                Ok(())
            }
            PlaintextType::Array(array_type) => match plaintext {
                // If `plaintext` is a literal, this is a mismatch.
                Plaintext::Literal(..) => bail!("'{plaintext_type}' is invalid: expected array, found literal"),
                // If `plaintext` is a struct, this is a mismatch.
                Plaintext::Struct(..) => bail!("'{plaintext_type}' is invalid: expected array, found struct"),
                // If `plaintext` is an array, it must match the array type.
                Plaintext::Array(array, ..) => {
                    // Ensure the array length matches.
                    let (actual_length, expected_length) = (array.len(), array_type.length());
                    if **expected_length as usize != actual_length {
                        bail!(
                            "'{plaintext_type}' is invalid: expected {expected_length} elements, found {actual_length} elements"
                        )
                    }
                    // Ensure the array elements match.
                    for element in array.iter() {
                        self.matches_plaintext_internal(element, array_type.next_element_type(), depth + 1)?;
                    }
                    Ok(())
                }
            },
        }
    }

    /// Checks that the given future matches the layout of the future type.
    fn matches_future_internal(&self, future: &Future<N>, locator: &Locator<N>, depth: usize) -> Result<()> {
        // If the depth exceeds the maximum depth, then the future type is invalid.
        ensure!(depth <= N::MAX_DATA_DEPTH, "Future exceeded maximum depth of {}", N::MAX_DATA_DEPTH);

        // Ensure that the program IDs match.
        ensure!(future.program_id() == locator.program_id(), "Future program ID does not match");

        // Ensure that the function names match.
        ensure!(future.function_name() == locator.resource(), "Future name does not match");

        // Retrieve the external stack, if needed.
        let external_stack = match locator.program_id() == self.program_id() {
            true => None,
            // Attention - This method must fail here and early return if the external program is missing.
            // Otherwise, this method will proceed to look for the requested function in its own program.
            false => Some(self.get_external_stack(locator.program_id())?),
        };
        // Retrieve the associated function.
        let function = match &external_stack {
            Some(external_stack) => external_stack.get_function_ref(locator.resource())?,
            None => self.get_function_ref(locator.resource())?,
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
                (Argument::Plaintext(plaintext), FinalizeType::Plaintext(plaintext_type)) => match &external_stack {
                    Some(external_stack) => {
                        external_stack.matches_plaintext_internal(plaintext, plaintext_type, depth + 1)?
                    }
                    None => self.matches_plaintext_internal(plaintext, plaintext_type, depth + 1)?,
                },
                (Argument::Future(future), FinalizeType::Future(locator)) => match &external_stack {
                    Some(external_stack) => external_stack.matches_future_internal(future, locator, depth + 1)?,
                    None => self.matches_future_internal(future, locator, depth + 1)?,
                },
                (Argument::DynamicFuture(_), FinalizeType::DynamicFuture) => {}
                (_, input_type) => {
                    bail!("Argument type does not match input type: expected '{input_type}'")
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;
    use snarkvm_synthesizer_program::Program;

    type CurrentNetwork = MainnetV0;

    // This test verifies that `contains_dynamic_call` returns `false` for a function
    // that contains only static `call` instructions (no `call.dynamic`).
    #[test]
    fn test_contains_dynamic_call_with_static_calls() {
        // `Process::load` always includes `credits.aleo`, which has no dynamic calls.
        let process = Process::<CurrentNetwork>::load().unwrap();
        let stack = process.get_stack("credits.aleo").unwrap();
        // `transfer_public` uses only static operations — no `call.dynamic`.
        let function_name = Identifier::from_str("transfer_public").unwrap();
        assert!(!stack.contains_dynamic_call(&function_name).unwrap());
    }

    // This test verifies that `contains_dynamic_call` returns `true` for a function
    // that directly contains a `call.dynamic` instruction.
    #[test]
    fn test_contains_dynamic_call_with_dynamic_calls() {
        // Define a program with a function that issues a bare `call.dynamic` (no inputs or outputs).
        let program = Program::<CurrentNetwork>::from_str(
            r"
program dynamic_test.aleo;

function dynamic_func:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as field.public;
    call.dynamic r0 r1 r2;",
        )
        .unwrap();
        // Add the program to a fresh process (no deployment needed for stack inspection).
        let process = Process::<CurrentNetwork>::load().unwrap();
        process.lock().add_program(&program).unwrap();
        let stack = process.get_stack("dynamic_test.aleo").unwrap();
        // `dynamic_func` contains a `call.dynamic` instruction and must be detected.
        let function_name = Identifier::from_str("dynamic_func").unwrap();
        assert!(stack.contains_dynamic_call(&function_name).unwrap());
    }

    // This test verifies that `contains_dynamic_call` returns `true` for a function that
    // transitively reaches a `call.dynamic` via a static external call.
    #[test]
    fn test_contains_dynamic_call_transitive() {
        // Define a helper program whose function issues `call.dynamic`.
        let helper_program = Program::<CurrentNetwork>::from_str(
            r"
program helper.aleo;

function dynamic_helper:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as field.public;
    call.dynamic r0 r1 r2;",
        )
        .unwrap();
        // Define a caller program that statically calls `helper.aleo/dynamic_helper`.
        let caller_program = Program::<CurrentNetwork>::from_str(
            r"
import helper.aleo;

program caller.aleo;

function caller_func:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as field.public;
    call helper.aleo/dynamic_helper r0 r1 r2;",
        )
        .unwrap();
        // Add programs in dependency order: helper first, then caller.
        let process = Process::<CurrentNetwork>::load().unwrap();
        process.lock().add_program(&helper_program).unwrap();
        process.lock().add_program(&caller_program).unwrap();
        let stack = process.get_stack("caller.aleo").unwrap();
        // `caller_func` transitively reaches `call.dynamic` via `helper.aleo/dynamic_helper`.
        let function_name = Identifier::from_str("caller_func").unwrap();
        assert!(stack.contains_dynamic_call(&function_name).unwrap());
    }
}
