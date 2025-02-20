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

#![forbid(unsafe_code)]
#![allow(clippy::too_many_arguments)]
#![warn(clippy::cast_possible_truncation)]

mod bytes;
mod parse;
mod serialize;

use super::*;

#[derive(Clone, PartialEq, Eq)]
pub struct ProgramCoreV2<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> {
    /// The ID of the program.
    id: ProgramID<N>,
    /// A map of the declared imports for the program.
    imports: IndexMap<ProgramID<N>, Import<N>>,
    /// A map of identifiers to their program declaration.
    identifiers: IndexMap<Identifier<N>, ProgramDefinition>,
    /// A map of the declared mappings for the program.
    mappings: IndexMap<Identifier<N>, Mapping<N>>,
    /// A map of the declared structs for the program.
    structs: IndexMap<Identifier<N>, StructType<N>>,
    /// A map of the declared record types for the program.
    records: IndexMap<Identifier<N>, RecordType<N>>,
    /// A map of the declared closures for the program.
    closures: IndexMap<Identifier<N>, ClosureCore<N, Instruction>>,
    /// A map of the declared functions for the program.
    functions: IndexMap<Identifier<N>, FunctionCore<N, Instruction, Command>>,
    /// The program constructor.
    constructor: Option<ConstructorCore<N, Command>>,
    /// Additional metadata for the program.
    metadata: IndexMap<Identifier<N>, Metadata<N>>,
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ProgramCoreV2<N, Instruction, Command> {
    /// Initializes an empty program.
    #[inline]
    pub fn new(id: ProgramID<N>) -> Result<Self> {
        // Ensure the program name is valid.
        ensure!(!Self::is_reserved_keyword(id.name()), "Program name is invalid: {}", id.name());

        Ok(Self {
            id,
            imports: IndexMap::new(),
            identifiers: IndexMap::new(),
            mappings: IndexMap::new(),
            structs: IndexMap::new(),
            records: IndexMap::new(),
            closures: IndexMap::new(),
            functions: IndexMap::new(),
            constructor: None,
            metadata: IndexMap::new(),
        })
    }

    /// Returns the ID of the program.
    pub const fn id(&self) -> &ProgramID<N> {
        &self.id
    }

    /// Returns the imports in the program.
    pub const fn imports(&self) -> &IndexMap<ProgramID<N>, Import<N>> {
        &self.imports
    }

    /// Returns the mappings in the program.
    pub const fn mappings(&self) -> &IndexMap<Identifier<N>, Mapping<N>> {
        &self.mappings
    }

    /// Returns the structs in the program.
    pub const fn structs(&self) -> &IndexMap<Identifier<N>, StructType<N>> {
        &self.structs
    }

    /// Returns the records in the program.
    pub const fn records(&self) -> &IndexMap<Identifier<N>, RecordType<N>> {
        &self.records
    }

    /// Returns the closures in the program.
    pub const fn closures(&self) -> &IndexMap<Identifier<N>, ClosureCore<N, Instruction>> {
        &self.closures
    }

    /// Returns the functions in the program.
    pub const fn functions(&self) -> &IndexMap<Identifier<N>, FunctionCore<N, Instruction, Command>> {
        &self.functions
    }

    /// Returns the constructor for the program.
    pub const fn constructor(&self) -> &Option<ConstructorCore<N, Command>> {
        &self.constructor
    }

    /// Returns the metadata for the program.
    pub const fn metadata(&self) -> &IndexMap<Identifier<N>, Metadata<N>> {
        &self.metadata
    }

    /// Returns `true` if the program contains an import with the given program ID.
    pub fn contains_import(&self, id: &ProgramID<N>) -> bool {
        self.imports.contains_key(id)
    }

    /// Returns `true` if the program contains a mapping with the given name.
    pub fn contains_mapping(&self, name: &Identifier<N>) -> bool {
        self.mappings.contains_key(name)
    }

    /// Returns `true` if the program contains a struct with the given name.
    pub fn contains_struct(&self, name: &Identifier<N>) -> bool {
        self.structs.contains_key(name)
    }

    /// Returns `true` if the program contains a record with the given name.
    pub fn contains_record(&self, name: &Identifier<N>) -> bool {
        self.records.contains_key(name)
    }

    /// Returns `true` if the program contains a closure with the given name.
    pub fn contains_closure(&self, name: &Identifier<N>) -> bool {
        self.closures.contains_key(name)
    }

    /// Returns `true` if the program contains a function with the given name.
    pub fn contains_function(&self, name: &Identifier<N>) -> bool {
        self.functions.contains_key(name)
    }

    /// Returns the mapping with the given name.
    pub fn get_mapping(&self, name: &Identifier<N>) -> Result<Mapping<N>> {
        // Attempt to retrieve the mapping.
        let mapping = self.mappings.get(name).cloned().ok_or_else(|| anyhow!("Mapping '{name}' is not defined."))?;
        // Ensure the mapping name matches.
        ensure!(mapping.name() == name, "Expected mapping '{name}', but found mapping '{}'", mapping.name());
        // Return the mapping.
        Ok(mapping)
    }

    /// Returns the struct with the given name.
    pub fn get_struct(&self, name: &Identifier<N>) -> Result<&StructType<N>> {
        // Attempt to retrieve the struct.
        let struct_ = self.structs.get(name).ok_or_else(|| anyhow!("Struct '{name}' is not defined."))?;
        // Ensure the struct name matches.
        ensure!(struct_.name() == name, "Expected struct '{name}', but found struct '{}'", struct_.name());
        // Ensure the struct contains members.
        ensure!(!struct_.members().is_empty(), "Struct '{name}' is missing members.");
        // Return the struct.
        Ok(struct_)
    }

    /// Returns the record with the given name.
    pub fn get_record(&self, name: &Identifier<N>) -> Result<&RecordType<N>> {
        // Attempt to retrieve the record.
        let record = self.records.get(name).ok_or_else(|| anyhow!("Record '{name}' is not defined."))?;
        // Ensure the record name matches.
        ensure!(record.name() == name, "Expected record '{name}', but found record '{}'", record.name());
        // Return the record.
        Ok(record)
    }

    /// Returns the closure with the given name.
    pub fn get_closure(&self, name: &Identifier<N>) -> Result<ClosureCore<N, Instruction>> {
        // Attempt to retrieve the closure.
        let closure = self.closures.get(name).cloned().ok_or_else(|| anyhow!("Closure '{name}' is not defined."))?;
        // Ensure the closure name matches.
        ensure!(closure.name() == name, "Expected closure '{name}', but found closure '{}'", closure.name());
        // Ensure there are input statements in the closure.
        ensure!(!closure.inputs().is_empty(), "Cannot evaluate a closure without input statements");
        // Ensure the number of inputs is within the allowed range.
        ensure!(closure.inputs().len() <= N::MAX_INPUTS, "Closure exceeds maximum number of inputs");
        // Ensure there are instructions in the closure.
        ensure!(!closure.instructions().is_empty(), "Cannot evaluate a closure without instructions");
        // Ensure the number of outputs is within the allowed range.
        ensure!(closure.outputs().len() <= N::MAX_OUTPUTS, "Closure exceeds maximum number of outputs");
        // Return the closure.
        Ok(closure)
    }

    /// Returns the function with the given name.
    pub fn get_function(&self, name: &Identifier<N>) -> Result<FunctionCore<N, Instruction, Command>> {
        self.get_function_ref(name).cloned()
    }

    /// Returns a reference to the function with the given name.
    pub fn get_function_ref(&self, name: &Identifier<N>) -> Result<&FunctionCore<N, Instruction, Command>> {
        // Attempt to retrieve the function.
        let function = self.functions.get(name).ok_or(anyhow!("Function '{}/{name}' is not defined.", self.id))?;
        // Ensure the function name matches.
        ensure!(function.name() == name, "Expected function '{name}', but found function '{}'", function.name());
        // Ensure the number of inputs is within the allowed range.
        ensure!(function.inputs().len() <= N::MAX_INPUTS, "Function exceeds maximum number of inputs");
        // Ensure the number of instructions is within the allowed range.
        ensure!(function.instructions().len() <= N::MAX_INSTRUCTIONS, "Function exceeds maximum instructions");
        // Ensure the number of outputs is within the allowed range.
        ensure!(function.outputs().len() <= N::MAX_OUTPUTS, "Function exceeds maximum number of outputs");
        // Return the function.
        Ok(function)
    }

    /// Returns the metadata value with the given name.
    pub fn get_metadata(&self, name: &Identifier<N>) -> Result<&Metadata<N>> {
        // Attempt to retrieve the metadata value.
        let metadata = self.metadata.get(name).ok_or_else(|| anyhow!("Metadata '{name}' is not defined."))?;
        // Ensure the metadata name matches.
        ensure!(metadata.name() == name, "Expected metadata '{name}', but found metadata '{}'", metadata.name());
        // Return the metadata value.
        Ok(metadata)
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ProgramCoreV2<N, Instruction, Command> {
    /// Adds a new import statement to the program.
    ///
    /// # Errors
    /// This method will halt if the imported program was previously added.
    #[inline]
    pub fn add_import(&mut self, import: Import<N>) -> Result<()> {
        // Retrieve the imported program name.
        let import_name = *import.name();

        // Ensure that the number of imports is within the allowed range.
        ensure!(self.imports.len() < N::MAX_IMPORTS, "Program exceeds the maximum number of imports");

        // Ensure the import name is new.
        ensure!(self.is_unique_name(&import_name), "'{import_name}' is already in use.");
        // Ensure the import name is not a reserved opcode.
        ensure!(!Self::is_reserved_opcode(&import_name.to_string()), "'{import_name}' is a reserved opcode.");
        // Ensure the import name is not a reserved keyword.
        ensure!(!Self::is_reserved_keyword(&import_name), "'{import_name}' is a reserved keyword.");

        // Ensure the import is new.
        ensure!(
            !self.imports.contains_key(import.program_id()),
            "Import '{}' is already defined.",
            import.program_id()
        );

        // Add the import statement to the program.
        if self.imports.insert(*import.program_id(), import.clone()).is_some() {
            bail!("'{}' already exists in the program.", import.program_id())
        }
        Ok(())
    }

    /// Adds a new mapping to the program.
    ///
    /// # Errors
    /// This method will halt if the mapping name is already in use.
    /// This method will halt if the mapping name is a reserved opcode or keyword.
    #[inline]
    pub fn add_mapping(&mut self, mapping: Mapping<N>) -> Result<()> {
        // Retrieve the mapping name.
        let mapping_name = *mapping.name();

        // Ensure the program has not exceeded the maximum number of mappings.
        ensure!(self.mappings.len() < N::MAX_MAPPINGS, "Program exceeds the maximum number of mappings");

        // Ensure the mapping name is new.
        ensure!(self.is_unique_name(&mapping_name), "'{mapping_name}' is already in use.");
        // Ensure the mapping name is not a reserved keyword.
        ensure!(!Self::is_reserved_keyword(&mapping_name), "'{mapping_name}' is a reserved keyword.");
        // Ensure the mapping name is not a reserved opcode.
        ensure!(!Self::is_reserved_opcode(&mapping_name.to_string()), "'{mapping_name}' is a reserved opcode.");

        // Add the mapping name to the identifiers.
        if self.identifiers.insert(mapping_name, ProgramDefinition::Mapping).is_some() {
            bail!("'{mapping_name}' already exists in the program.")
        }
        // Add the mapping to the program.
        if self.mappings.insert(mapping_name, mapping).is_some() {
            bail!("'{mapping_name}' already exists in the program.")
        }
        Ok(())
    }

    /// Adds a new struct to the program.
    ///
    /// # Errors
    /// This method will halt if the struct was previously added.
    /// This method will halt if the struct name is already in use in the program.
    /// This method will halt if the struct name is a reserved opcode or keyword.
    /// This method will halt if any structs in the struct's members are not already defined.
    #[inline]
    pub fn add_struct(&mut self, struct_: StructType<N>) -> Result<()> {
        // Retrieve the struct name.
        let struct_name = *struct_.name();

        // Ensure the program has not exceeded the maximum number of structs.
        ensure!(self.structs.len() < N::MAX_STRUCTS, "Program exceeds the maximum number of structs.");

        // Ensure the struct name is new.
        ensure!(self.is_unique_name(&struct_name), "'{struct_name}' is already in use.");
        // Ensure the struct name is not a reserved opcode.
        ensure!(!Self::is_reserved_opcode(&struct_name.to_string()), "'{struct_name}' is a reserved opcode.");
        // Ensure the struct name is not a reserved keyword.
        ensure!(!Self::is_reserved_keyword(&struct_name), "'{struct_name}' is a reserved keyword.");

        // Ensure the struct contains members.
        ensure!(!struct_.members().is_empty(), "Struct '{struct_name}' is missing members.");

        // Ensure all struct members are well-formed.
        // Note: This design ensures cyclic references are not possible.
        for (identifier, plaintext_type) in struct_.members() {
            // Ensure the member name is not a reserved keyword.
            ensure!(!Self::is_reserved_keyword(identifier), "'{identifier}' is a reserved keyword.");
            // Ensure the member type is already defined in the program.
            match plaintext_type {
                PlaintextType::Literal(_) => continue,
                PlaintextType::Struct(member_identifier) => {
                    // Ensure the member struct name exists in the program.
                    if !self.structs.contains_key(member_identifier) {
                        bail!("'{member_identifier}' in struct '{}' is not defined.", struct_name)
                    }
                }
                PlaintextType::Array(array_type) => {
                    if let PlaintextType::Struct(struct_name) = array_type.base_element_type() {
                        // Ensure the member struct name exists in the program.
                        if !self.structs.contains_key(struct_name) {
                            bail!("'{struct_name}' in array '{array_type}' is not defined.")
                        }
                    }
                }
            }
        }

        // Add the struct name to the identifiers.
        if self.identifiers.insert(struct_name, ProgramDefinition::Struct).is_some() {
            bail!("'{}' already exists in the program.", struct_name)
        }
        // Add the struct to the program.
        if self.structs.insert(struct_name, struct_).is_some() {
            bail!("'{}' already exists in the program.", struct_name)
        }
        Ok(())
    }

    /// Adds a new record to the program.
    ///
    /// # Errors
    /// This method will halt if the record was previously added.
    /// This method will halt if the record name is already in use in the program.
    /// This method will halt if the record name is a reserved opcode or keyword.
    /// This method will halt if any records in the record's members are not already defined.
    #[inline]
    pub fn add_record(&mut self, record: RecordType<N>) -> Result<()> {
        // Retrieve the record name.
        let record_name = *record.name();

        // Ensure the program has not exceeded the maximum number of records.
        ensure!(self.records.len() < N::MAX_RECORDS, "Program exceeds the maximum number of records.");

        // Ensure the record name is new.
        ensure!(self.is_unique_name(&record_name), "'{record_name}' is already in use.");
        // Ensure the record name is not a reserved opcode.
        ensure!(!Self::is_reserved_opcode(&record_name.to_string()), "'{record_name}' is a reserved opcode.");
        // Ensure the record name is not a reserved keyword.
        ensure!(!Self::is_reserved_keyword(&record_name), "'{record_name}' is a reserved keyword.");

        // Ensure all record entries are well-formed.
        // Note: This design ensures cyclic references are not possible.
        for (identifier, entry_type) in record.entries() {
            // Ensure the member name is not a reserved keyword.
            ensure!(!Self::is_reserved_keyword(identifier), "'{identifier}' is a reserved keyword.");
            // Ensure the member type is already defined in the program.
            match entry_type.plaintext_type() {
                PlaintextType::Literal(_) => continue,
                PlaintextType::Struct(identifier) => {
                    if !self.structs.contains_key(identifier) {
                        bail!("Struct '{identifier}' in record '{record_name}' is not defined.")
                    }
                }
                PlaintextType::Array(array_type) => {
                    if let PlaintextType::Struct(struct_name) = array_type.base_element_type() {
                        // Ensure the member struct name exists in the program.
                        if !self.structs.contains_key(struct_name) {
                            bail!("'{struct_name}' in array '{array_type}' is not defined.")
                        }
                    }
                }
            }
        }

        // Add the record name to the identifiers.
        if self.identifiers.insert(record_name, ProgramDefinition::Record).is_some() {
            bail!("'{record_name}' already exists in the program.")
        }
        // Add the record to the program.
        if self.records.insert(record_name, record).is_some() {
            bail!("'{record_name}' already exists in the program.")
        }
        Ok(())
    }

    /// Adds a new closure to the program.
    ///
    /// # Errors
    /// This method will halt if the closure was previously added.
    /// This method will halt if the closure name is already in use in the program.
    /// This method will halt if the closure name is a reserved opcode or keyword.
    /// This method will halt if any registers are assigned more than once.
    /// This method will halt if the registers are not incrementing monotonically.
    /// This method will halt if an input type references a non-existent definition.
    /// This method will halt if an operand register does not already exist in memory.
    /// This method will halt if a destination register already exists in memory.
    /// This method will halt if an output register does not already exist.
    /// This method will halt if an output type references a non-existent definition.
    #[inline]
    pub fn add_closure(&mut self, closure: ClosureCore<N, Instruction>) -> Result<()> {
        // Retrieve the closure name.
        let closure_name = *closure.name();

        // Ensure the program has not exceeded the maximum number of closures.
        ensure!(self.closures.len() < N::MAX_CLOSURES, "Program exceeds the maximum number of closures.");

        // Ensure the closure name is new.
        ensure!(self.is_unique_name(&closure_name), "'{closure_name}' is already in use.");
        // Ensure the closure name is not a reserved opcode.
        ensure!(!Self::is_reserved_opcode(&closure_name.to_string()), "'{closure_name}' is a reserved opcode.");
        // Ensure the closure name is not a reserved keyword.
        ensure!(!Self::is_reserved_keyword(&closure_name), "'{closure_name}' is a reserved keyword.");

        // Ensure there are input statements in the closure.
        ensure!(!closure.inputs().is_empty(), "Cannot evaluate a closure without input statements");
        // Ensure the number of inputs is within the allowed range.
        ensure!(closure.inputs().len() <= N::MAX_INPUTS, "Closure exceeds maximum number of inputs");
        // Ensure there are instructions in the closure.
        ensure!(!closure.instructions().is_empty(), "Cannot evaluate a closure without instructions");
        // Ensure the number of outputs is within the allowed range.
        ensure!(closure.outputs().len() <= N::MAX_OUTPUTS, "Closure exceeds maximum number of outputs");

        // Add the function name to the identifiers.
        if self.identifiers.insert(closure_name, ProgramDefinition::Closure).is_some() {
            bail!("'{closure_name}' already exists in the program.")
        }
        // Add the closure to the program.
        if self.closures.insert(closure_name, closure).is_some() {
            bail!("'{closure_name}' already exists in the program.")
        }
        Ok(())
    }

    /// Adds a new function to the program.
    ///
    /// # Errors
    /// This method will halt if the function was previously added.
    /// This method will halt if the function name is already in use in the program.
    /// This method will halt if the function name is a reserved opcode or keyword.
    /// This method will halt if any registers are assigned more than once.
    /// This method will halt if the registers are not incrementing monotonically.
    /// This method will halt if an input type references a non-existent definition.
    /// This method will halt if an operand register does not already exist in memory.
    /// This method will halt if a destination register already exists in memory.
    /// This method will halt if an output register does not already exist.
    /// This method will halt if an output type references a non-existent definition.
    #[inline]
    pub fn add_function(&mut self, function: FunctionCore<N, Instruction, Command>) -> Result<()> {
        // Retrieve the function name.
        let function_name = *function.name();

        // Ensure the program has not exceeded the maximum number of functions.
        ensure!(self.functions.len() < N::MAX_FUNCTIONS, "Program exceeds the maximum number of functions");

        // Ensure the function name is new.
        ensure!(self.is_unique_name(&function_name), "'{function_name}' is already in use.");
        // Ensure the function name is not a reserved opcode.
        ensure!(!Self::is_reserved_opcode(&function_name.to_string()), "'{function_name}' is a reserved opcode.");
        // Ensure the function name is not a reserved keyword.
        ensure!(!Self::is_reserved_keyword(&function_name), "'{function_name}' is a reserved keyword.");

        // Ensure the number of inputs is within the allowed range.
        ensure!(function.inputs().len() <= N::MAX_INPUTS, "Function exceeds maximum number of inputs");
        // Ensure the number of instructions is within the allowed range.
        ensure!(function.instructions().len() <= N::MAX_INSTRUCTIONS, "Function exceeds maximum instructions");
        // Ensure the number of outputs is within the allowed range.
        ensure!(function.outputs().len() <= N::MAX_OUTPUTS, "Function exceeds maximum number of outputs");

        // Add the function name to the identifiers.
        if self.identifiers.insert(function_name, ProgramDefinition::Function).is_some() {
            bail!("'{function_name}' already exists in the program.")
        }
        // Add the function to the program.
        if self.functions.insert(function_name, function).is_some() {
            bail!("'{function_name}' already exists in the program.")
        }
        Ok(())
    }

    /// Add a constructor for the program.
    ///
    /// # Errors
    /// This method will halt if the constructor was previously set.
    #[inline]
    pub fn add_constructor(&mut self, constructor: ConstructorCore<N, Command>) -> Result<()> {
        // Ensure that the constructor has not been set.
        ensure!(self.constructor.is_none(), "Constructor is already set.");
        // Set the constructor.
        self.constructor = Some(constructor);
        Ok(())
    }

    /// Adds a new metadata value to the program.
    ///
    /// # Errors
    /// This method will halt if the metadata name is already in use.
    /// This method will halt if the metadata name is a reserved opcode or keyword.
    #[inline]
    pub fn add_metadata(&mut self, metadata: Metadata<N>) -> Result<()> {
        // Retrieve the metadata name.
        let name = *metadata.name();

        // Ensure the metadata name is new.
        ensure!(self.is_unique_name(&name), "'{name}' is already in use.");
        // Ensure the metadata name is not a reserved opcode.
        ensure!(!Self::is_reserved_opcode(&name.to_string()), "'{name}' is a reserved opcode.");
        // Ensure the metadata name is not a reserved keyword.
        ensure!(!Self::is_reserved_keyword(&name), "'{name}' is a reserved keyword.");

        // Add the metadata value to the program.
        if self.metadata.insert(name, metadata).is_some() {
            bail!("'{name}' already exists in the metadata.")
        }
        Ok(())
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ProgramCoreV2<N, Instruction, Command> {
    /// Returns `true` if the given name does not already exist in the program.
    fn is_unique_name(&self, name: &Identifier<N>) -> bool {
        !self.identifiers.contains_key(name)
    }

    /// Returns `true` if the given name is a reserved opcode.
    fn is_reserved_opcode(name: &str) -> bool {
        ProgramCore::<N, Instruction, Command>::is_reserved_opcode(name)
    }

    /// Returns `true` if the given name uses a reserved keyword.
    fn is_reserved_keyword(name: &Identifier<N>) -> bool {
        ProgramCore::<N, Instruction, Command>::is_reserved_keyword(name)
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> TypeName
    for ProgramCoreV2<N, Instruction, Command>
{
    /// Returns the type name as a string.
    #[inline]
    fn type_name() -> &'static str {
        "program$2"
    }
}
