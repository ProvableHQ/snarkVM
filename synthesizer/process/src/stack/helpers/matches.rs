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

impl<N: Network> StackMatches<N> for Stack<N> {
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
            _ => bail!("A value does not match its declared value type '{value_type}'"),
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
            _ => bail!("A value does not match its declared register type '{register_type}'"),
        }
    }

    /// Checks that the given record matches the layout of the external record type.
    fn matches_external_record(&self, record: &Record<N, Plaintext<N>>, locator: &Locator<N>) -> Result<()> {
        // Retrieve the record name.
        let record_name = locator.resource();

        // Ensure the record name is valid.
        ensure!(!Program::is_reserved_keyword(record_name), "Record name '{record_name}' is reserved");

        // Retrieve the record type from the program.
        let Ok(record_type) = self.get_external_record(locator) else {
            bail!("External '{locator}' is not defined in the program")
        };

        // Ensure the record name matches.
        if record_type.name() != record_name {
            bail!("Expected external record '{record_name}', found external record '{}'", record_type.name())
        }

        self.matches_record_internal(record, record_type, 0)
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
                        false => bail!("'{plaintext_type}' is invalid: expected {literal_type}, found {literal}"),
                    }
                }
                // If `plaintext` is a struct, this is a mismatch.
                Plaintext::Struct(..) => bail!("'{plaintext_type}' is invalid: expected literal, found struct"),
                // If `plaintext` is an array, this is a mismatch.
                Plaintext::Array(..) => bail!("'{plaintext_type}' is invalid: expected literal, found array"),
            },
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

        // Retrieve the associated function.
        let function = match locator.program_id() == self.program_id() {
            true => self.get_function_ref(locator.resource())?,
            false => self.get_external_program(locator.program_id())?.get_function_ref(locator.resource())?,
        };
        // Retrieve the finalize inputs.
        let inputs = match function.finalize_logic() {
            Some(finalize_logic) => finalize_logic.inputs(),
            None => bail!("Function '{locator}' does not have a finalize block"),
        };

        // Ensure the number of arguments matches the number of inputs.
        ensure!(future.arguments().len() == inputs.len(), "Future arguments do not match");

        // Check that the arguments match the inputs.
        for (argument, input) in future.arguments().iter().zip_eq(inputs.iter()) {
            match (argument, input.finalize_type()) {
                (Argument::Plaintext(plaintext), FinalizeType::Plaintext(plaintext_type)) => {
                    self.matches_plaintext_internal(plaintext, plaintext_type, depth + 1)?
                }
                (Argument::Future(future), FinalizeType::Future(locator)) => {
                    self.matches_future_internal(future, locator, depth + 1)?
                }
                (_, input_type) => {
                    bail!("Argument type does not match input type: expected '{input_type}'")
                }
            }
        }

        Ok(())
    }
}
