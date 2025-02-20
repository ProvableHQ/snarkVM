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

pub type Program<N> = crate::ProgramCore<N, Instruction<N>, Command<N>>;
pub type Constructor<N> = crate::ConstructorCore<N, Command<N>>;
pub type Function<N> = crate::FunctionCore<N, Instruction<N>, Command<N>>;
pub type Finalize<N> = crate::FinalizeCore<N, Command<N>>;
pub type Closure<N> = crate::ClosureCore<N, Instruction<N>>;

mod closure;
pub use closure::*;

mod constructor;
pub use constructor::*;

pub mod finalize;
pub use finalize::*;

mod function;
pub use function::*;

mod import;
pub use import::*;

pub mod logic;
pub use logic::*;

mod mapping;
pub use mapping::*;

pub mod traits;
pub use traits::*;

mod bytes;
mod parse;
mod serialize;

use console::{
    network::prelude::{
        Debug,
        Deserialize,
        Deserializer,
        Display,
        Err,
        Error,
        ErrorKind,
        Formatter,
        FromBytes,
        FromBytesDeserializer,
        FromStr,
        IoResult,
        Network,
        Parser,
        ParserResult,
        Read,
        Result,
        Sanitizer,
        Serialize,
        Serializer,
        ToBytes,
        ToBytesSerializer,
        TypeName,
        Write,
        alt,
        anyhow,
        bail,
        de,
        ensure,
        error,
        fmt,
        make_error,
        many0,
        many1,
        map,
        map_res,
        tag,
        take,
    },
    program::{Identifier, PlaintextType, ProgramID, RecordType, StructType, Value},
};
use indexmap::IndexMap;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum ProgramDefinition {
    /// A program mapping.
    Mapping,
    /// A program struct.
    Struct,
    /// A program record.
    Record,
    /// A program closure.
    Closure,
    /// A program function.
    Function,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProgramCoreV1<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> {
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
}

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
    /// A constructor for the program.
    constructor: Option<ConstructorCore<N, Command>>,
    // TODO (@d0cd) Consider versioning the metadata.
    /// Additional metadata for the program.
    metadata: IndexMap<Identifier<N>, Value<N>>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum ProgramCore<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> {
    /// The V1 program.
    ProgramV1(ProgramCoreV1<N, Instruction, Command>),
    /// The V2 program.
    ProgramV2(ProgramCoreV2<N, Instruction, Command>),
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ProgramCore<N, Instruction, Command> {
    /// Initializes an empty V1 program.
    #[inline]
    pub fn new_v1(id: ProgramID<N>) -> Result<Self> {
        // Ensure the program name is valid.
        ensure!(!Self::is_reserved_keyword(id.name()), "Program name is invalid: {}", id.name());

        Ok(Self::ProgramV1(ProgramCoreV1 {
            id,
            imports: IndexMap::new(),
            identifiers: IndexMap::new(),
            mappings: IndexMap::new(),
            structs: IndexMap::new(),
            records: IndexMap::new(),
            closures: IndexMap::new(),
            functions: IndexMap::new(),
        }))
    }

    /// Initializes an empty V2 program.
    #[inline]
    pub fn new_v2(id: ProgramID<N>) -> Result<Self> {
        // Ensure the program name is valid.
        ensure!(!Self::is_reserved_keyword(id.name()), "Program name is invalid: {}", id.name());

        Ok(Self::ProgramV2(ProgramCoreV2 {
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
        }))
    }

    /// Initializes the credits program.
    #[inline]
    pub fn credits() -> Result<Self> {
        Self::from_str(include_str!("./resources/credits.aleo"))
    }

    /// Returns the ID of the program.
    pub const fn id(&self) -> &ProgramID<N> {
        match &self {
            Self::ProgramV1(program) => &program.id,
            Self::ProgramV2(program) => &program.id,
        }
    }

    /// Returns the identifiers in the program.
    pub const fn identifiers(&self) -> &IndexMap<Identifier<N>, ProgramDefinition> {
        match &self {
            Self::ProgramV1(program) => &program.identifiers,
            Self::ProgramV2(program) => &program.identifiers,
        }
    }

    /// Returns the imports in the program.
    pub const fn imports(&self) -> &IndexMap<ProgramID<N>, Import<N>> {
        match &self {
            Self::ProgramV1(program) => &program.imports,
            Self::ProgramV2(program) => &program.imports,
        }
    }

    /// Returns the mappings in the program.
    pub const fn mappings(&self) -> &IndexMap<Identifier<N>, Mapping<N>> {
        match &self {
            Self::ProgramV1(program) => &program.mappings,
            Self::ProgramV2(program) => &program.mappings,
        }
    }

    /// Returns the structs in the program.
    pub const fn structs(&self) -> &IndexMap<Identifier<N>, StructType<N>> {
        match &self {
            Self::ProgramV1(program) => &program.structs,
            Self::ProgramV2(program) => &program.structs,
        }
    }

    /// Returns the records in the program.
    pub const fn records(&self) -> &IndexMap<Identifier<N>, RecordType<N>> {
        match &self {
            Self::ProgramV1(program) => &program.records,
            Self::ProgramV2(program) => &program.records,
        }
    }

    /// Returns the closures in the program.
    pub const fn closures(&self) -> &IndexMap<Identifier<N>, ClosureCore<N, Instruction>> {
        match &self {
            Self::ProgramV1(program) => &program.closures,
            Self::ProgramV2(program) => &program.closures,
        }
    }

    /// Returns the functions in the program.
    pub const fn functions(&self) -> &IndexMap<Identifier<N>, FunctionCore<N, Instruction, Command>> {
        match &self {
            Self::ProgramV1(program) => &program.functions,
            Self::ProgramV2(program) => &program.functions,
        }
    }

    /// Returns the constructor in the program.
    pub fn constructor(&self) -> Result<&Option<ConstructorCore<N, Command>>> {
        match &self {
            Self::ProgramV1(_) => bail!("Constructors are not supported in V1 programs"),
            Self::ProgramV2(program) => Ok(&program.constructor),
        }
    }

    /// Returns the metadata in the program.
    pub fn metadata(&self) -> Result<&IndexMap<Identifier<N>, Value<N>>> {
        match &self {
            Self::ProgramV1(_) => bail!("Metadata is not supported in V1 programs"),
            Self::ProgramV2(program) => Ok(&program.metadata),
        }
    }

    /// Returns `true` if the program contains an import with the given program ID.
    pub fn contains_import(&self, id: &ProgramID<N>) -> bool {
        self.imports().contains_key(id)
    }

    /// Returns `true` if the program contains a mapping with the given name.
    pub fn contains_mapping(&self, name: &Identifier<N>) -> bool {
        self.mappings().contains_key(name)
    }

    /// Returns `true` if the program contains a struct with the given name.
    pub fn contains_struct(&self, name: &Identifier<N>) -> bool {
        self.structs().contains_key(name)
    }

    /// Returns `true` if the program contains a record with the given name.
    pub fn contains_record(&self, name: &Identifier<N>) -> bool {
        self.records().contains_key(name)
    }

    /// Returns `true` if the program contains a closure with the given name.
    pub fn contains_closure(&self, name: &Identifier<N>) -> bool {
        self.closures().contains_key(name)
    }

    /// Returns `true` if the program contains a function with the given name.
    pub fn contains_function(&self, name: &Identifier<N>) -> bool {
        self.functions().contains_key(name)
    }

    /// Returns `true` if the program contains metadata with the given name.
    pub fn contains_metadata(&self, name: &Identifier<N>) -> Result<bool> {
        self.metadata().map(|metadata| metadata.contains_key(name))
    }

    /// Returns the mapping with the given name.
    pub fn get_mapping(&self, name: &Identifier<N>) -> Result<Mapping<N>> {
        // Attempt to retrieve the mapping.
        let mapping = self.mappings().get(name).cloned().ok_or_else(|| anyhow!("Mapping '{name}' is not defined."))?;
        // Ensure the mapping name matches.
        ensure!(mapping.name() == name, "Expected mapping '{name}', but found mapping '{}'", mapping.name());
        // Return the mapping.
        Ok(mapping)
    }

    /// Returns the struct with the given name.
    pub fn get_struct(&self, name: &Identifier<N>) -> Result<&StructType<N>> {
        // Attempt to retrieve the struct.
        let struct_ = self.structs().get(name).ok_or_else(|| anyhow!("Struct '{name}' is not defined."))?;
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
        let record = self.records().get(name).ok_or_else(|| anyhow!("Record '{name}' is not defined."))?;
        // Ensure the record name matches.
        ensure!(record.name() == name, "Expected record '{name}', but found record '{}'", record.name());
        // Return the record.
        Ok(record)
    }

    /// Returns the closure with the given name.
    pub fn get_closure(&self, name: &Identifier<N>) -> Result<ClosureCore<N, Instruction>> {
        // Attempt to retrieve the closure.
        let closure = self.closures().get(name).cloned().ok_or_else(|| anyhow!("Closure '{name}' is not defined."))?;
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
        let function = self.functions().get(name).ok_or(anyhow!("Function '{}/{name}' is not defined.", self.id()))?;
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
    pub fn get_metadata(&self, name: &Identifier<N>) -> Result<&Value<N>> {
        // Attempt to retrieve the metadata.
        let metadata = self.metadata()?.get(name).ok_or(anyhow!("Metadata '{name}' is not defined."))?;
        // Return the metadata.
        Ok(metadata)
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ProgramCore<N, Instruction, Command> {
    /// Adds a new import statement to the program.
    ///
    /// # Errors
    /// This method will halt if the import is already in use.
    #[inline]
    pub fn add_import(&mut self, import: Import<N>) -> Result<()> {
        // Retrieve the imported program name.
        let import_name = *import.name();

        // Ensure that the number of imports is within the allowed range.
        ensure!(self.imports().len() < N::MAX_IMPORTS, "Program exceeds the maximum number of imports");

        // Ensure the import name is new.
        ensure!(self.is_unique_name(&import_name), "'{import_name}' is already in use.");
        // Ensure the import name is not a reserved opcode.
        ensure!(!Self::is_reserved_opcode(&import_name.to_string()), "'{import_name}' is a reserved opcode.");
        // Ensure the import name is not a reserved keyword.
        ensure!(!Self::is_reserved_keyword(&import_name), "'{import_name}' is a reserved keyword.");

        // Ensure the import is new.
        ensure!(
            !self.imports().contains_key(import.program_id()),
            "Import '{}' is already defined.",
            import.program_id()
        );

        // Add the import statement to the program.
        match self {
            Self::ProgramV1(program) => {
                if program.imports.insert(*import.program_id(), import.clone()).is_some() {
                    bail!("'{}' already exists in the program.", import.program_id())
                }
            }
            Self::ProgramV2(program) => {
                if program.imports.insert(*import.program_id(), import.clone()).is_some() {
                    bail!("'{}' already exists in the program.", import.program_id())
                }
            }
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
        ensure!(self.mappings().len() < N::MAX_MAPPINGS, "Program exceeds the maximum number of mappings");

        // Ensure the mapping name is new.
        ensure!(self.is_unique_name(&mapping_name), "'{mapping_name}' is already in use.");
        // Ensure the mapping name is not a reserved keyword.
        ensure!(!Self::is_reserved_keyword(&mapping_name), "'{mapping_name}' is a reserved keyword.");
        // Ensure the mapping name is not a reserved opcode.
        ensure!(!Self::is_reserved_opcode(&mapping_name.to_string()), "'{mapping_name}' is a reserved opcode.");

        // Add the mapping name to the identifiers.
        // Add the mapping to the program.
        match self {
            Self::ProgramV1(program) => {
                if program.identifiers.insert(mapping_name, ProgramDefinition::Mapping).is_some() {
                    bail!("'{mapping_name}' already exists in the program.")
                }
                if program.mappings.insert(mapping_name, mapping).is_some() {
                    bail!("'{mapping_name}' already exists in the program.")
                }
            }
            Self::ProgramV2(program) => {
                if program.identifiers.insert(mapping_name, ProgramDefinition::Mapping).is_some() {
                    bail!("'{mapping_name}' already exists in the program.")
                }
                if program.mappings.insert(mapping_name, mapping).is_some() {
                    bail!("'{mapping_name}' already exists in the program.")
                }
            }
        }
        Ok(())
    }

    /// Adds a new struct to the program.
    ///
    /// # Errors
    /// This method will halt if the struct name is already in use in the program.
    /// This method will halt if the struct name is a reserved opcode or keyword.
    /// This method will halt if any structs in the struct's members are not already defined.
    #[inline]
    pub fn add_struct(&mut self, struct_: StructType<N>) -> Result<()> {
        // Retrieve the struct name.
        let struct_name = *struct_.name();

        // Ensure the program has not exceeded the maximum number of structs.
        ensure!(self.structs().len() < N::MAX_STRUCTS, "Program exceeds the maximum number of structs.");

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
                    if !self.structs().contains_key(member_identifier) {
                        bail!("'{member_identifier}' in struct '{}' is not defined.", struct_name)
                    }
                }
                PlaintextType::Array(array_type) => {
                    if let PlaintextType::Struct(struct_name) = array_type.base_element_type() {
                        // Ensure the member struct name exists in the program.
                        if !self.structs().contains_key(struct_name) {
                            bail!("'{struct_name}' in array '{array_type}' is not defined.")
                        }
                    }
                }
            }
        }

        // Add the struct name to the identifiers.
        // Add the struct to the program.
        match self {
            Self::ProgramV1(program) => {
                if program.identifiers.insert(struct_name, ProgramDefinition::Struct).is_some() {
                    bail!("'{struct_name}' already exists in the program.")
                }
                if program.structs.insert(struct_name, struct_).is_some() {
                    bail!("'{struct_name}' already exists in the program.")
                }
            }
            Self::ProgramV2(program) => {
                if program.identifiers.insert(struct_name, ProgramDefinition::Struct).is_some() {
                    bail!("'{struct_name}' already exists in the program.")
                }
                if program.structs.insert(struct_name, struct_).is_some() {
                    bail!("'{struct_name}' already exists in the program.")
                }
            }
        }
        Ok(())
    }

    /// Adds a new record to the program.
    ///
    /// # Errors
    /// This method will halt if the record name is already in use in the program.
    /// This method will halt if the record name is a reserved opcode or keyword.
    /// This method will halt if any records in the record's members are not already defined.
    #[inline]
    pub fn add_record(&mut self, record: RecordType<N>) -> Result<()> {
        // Retrieve the record name.
        let record_name = *record.name();

        // Ensure the program has not exceeded the maximum number of records.
        ensure!(self.records().len() < N::MAX_RECORDS, "Program exceeds the maximum number of records.");

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
                    if !self.structs().contains_key(identifier) {
                        bail!("Struct '{identifier}' in record '{record_name}' is not defined.")
                    }
                }
                PlaintextType::Array(array_type) => {
                    if let PlaintextType::Struct(struct_name) = array_type.base_element_type() {
                        // Ensure the member struct name exists in the program.
                        if !self.structs().contains_key(struct_name) {
                            bail!("'{struct_name}' in array '{array_type}' is not defined.")
                        }
                    }
                }
            }
        }

        // Add the record name to the identifiers.
        // Add the record to the program.
        match self {
            Self::ProgramV1(program) => {
                if program.identifiers.insert(record_name, ProgramDefinition::Record).is_some() {
                    bail!("'{record_name}' already exists in the program.")
                }
                if program.records.insert(record_name, record).is_some() {
                    bail!("'{record_name}' already exists in the program.")
                }
            }
            Self::ProgramV2(program) => {
                if program.identifiers.insert(record_name, ProgramDefinition::Record).is_some() {
                    bail!("'{record_name}' already exists in the program.")
                }
                if program.records.insert(record_name, record).is_some() {
                    bail!("'{record_name}' already exists in the program.")
                }
            }
        }
        Ok(())
    }

    /// Adds a new closure to the program.
    ///
    /// # Errors
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
        ensure!(self.closures().len() < N::MAX_CLOSURES, "Program exceeds the maximum number of closures.");

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
        // Ensure the number of instructions are within the allowed range.
        ensure!(!closure.instructions().is_empty(), "Cannot evaluate a closure without instructions");
        ensure!(closure.instructions().len() <= N::MAX_INSTRUCTIONS, "Closure exceeds maximum instructions");
        // Ensure the number of outputs is within the allowed range.
        ensure!(closure.outputs().len() <= N::MAX_OUTPUTS, "Closure exceeds maximum number of outputs");

        // Add the function name to the identifiers.
        // Add the closure to the program.
        match self {
            Self::ProgramV1(program) => {
                if program.identifiers.insert(closure_name, ProgramDefinition::Closure).is_some() {
                    bail!("'{closure_name}' already exists in the program.")
                }
                if program.closures.insert(closure_name, closure).is_some() {
                    bail!("'{closure_name}' already exists in the program.")
                }
            }
            Self::ProgramV2(program) => {
                if program.identifiers.insert(closure_name, ProgramDefinition::Closure).is_some() {
                    bail!("'{closure_name}' already exists in the program.")
                }
                if program.closures.insert(closure_name, closure).is_some() {
                    bail!("'{closure_name}' already exists in the program.")
                }
            }
        }
        Ok(())
    }

    /// Adds a new function to the program.
    ///
    /// # Errors
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
        ensure!(self.functions().len() < N::MAX_FUNCTIONS, "Program exceeds the maximum number of functions");

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
        // Add the function to the program.
        match self {
            Self::ProgramV1(program) => {
                if program.identifiers.insert(function_name, ProgramDefinition::Function).is_some() {
                    bail!("'{function_name}' already exists in the program.")
                }
                if program.functions.insert(function_name, function).is_some() {
                    bail!("'{function_name}' already exists in the program.")
                }
            }
            Self::ProgramV2(program) => {
                if program.identifiers.insert(function_name, ProgramDefinition::Function).is_some() {
                    bail!("'{function_name}' already exists in the program.")
                }
                if program.functions.insert(function_name, function).is_some() {
                    bail!("'{function_name}' already exists in the program.")
                }
            }
        }
        Ok(())
    }

    /// Updates the constructor in the program.
    ///
    /// # Errors
    /// This method will halt if a constructor has already been added.
    pub fn add_constructor(&mut self, constructor: ConstructorCore<N, Command>) -> Result<()> {
        match self {
            Self::ProgramV1(_) => bail!("Cannot add a constructor to a V1 program"),
            Self::ProgramV2(program) => {
                // Ensure the program has not exceeded the maximum number of constructors.
                ensure!(program.constructor.is_none(), "Cannot add multiple constructors to the program");
                // Add the constructor to the program.
                program.constructor = Some(constructor);
                Ok(())
            }
        }
    }

    /// Adds a new metadata value to the program.
    ///
    /// # Errors
    /// This method will halt if the metadata name is already in use in the program.
    /// This method will halt if the metadata name is a reserved opcode or keyword.
    pub fn add_metadata(&mut self, name: Identifier<N>, value: Value<N>) -> Result<()> {
        // Ensure the metadata name is new.
        ensure!(self.is_unique_name(&name), "'{name}' is already in use.");
        // Ensure the metadata name is not a reserved opcode.
        ensure!(!Self::is_reserved_opcode(&name.to_string()), "'{name}' is a reserved opcode.");
        // Ensure the metadata name is not a reserved keyword.
        ensure!(!Self::is_reserved_keyword(&name), "'{name}' is a reserved keyword.");

        // Add the metadata name to the program.
        match self {
            Self::ProgramV1(_) => bail!("Metadata is not supported in V1 programs"),
            Self::ProgramV2(program) => {
                if program.metadata.insert(name, value).is_some() {
                    bail!("'{name}' already exists in the program.")
                }
            }
        }
        Ok(())
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ProgramCore<N, Instruction, Command> {
    /// Removes an import from the program.
    ///
    /// # Errors
    /// This method will halt if the imported program is not in the program.
    #[inline]
    pub fn remove_import(&mut self, program_id: &ProgramID<N>) -> Result<Import<N>> {
        match self {
            Self::ProgramV1(program) => match program.imports.shift_remove(program_id) {
                Some(import) => Ok(import),
                None => bail!("Import '{}' not found.", program_id),
            },
            Self::ProgramV2(program) => match program.imports.shift_remove(program_id) {
                Some(import) => Ok(import),
                None => bail!("Import '{}' not found.", program_id),
            },
        }
    }

    /// Removes a mapping from the program.
    ///
    /// # Errors
    /// This method will halt if the mapping is not in the program.
    #[inline]
    pub fn remove_mapping(&mut self, mapping_name: &Identifier<N>) -> Result<Mapping<N>> {
        match self {
            Self::ProgramV1(program) => {
                // Remove the mapping from `identifiers`.
                program.identifiers.shift_remove(mapping_name);
                // Remove and return the mapping.
                match program.mappings.shift_remove(mapping_name) {
                    Some(mapping) => Ok(mapping),
                    None => bail!("Mapping '{}' not found.", mapping_name),
                }
            }
            Self::ProgramV2(program) => {
                // Remove the mapping from `identifiers`.
                program.identifiers.shift_remove(mapping_name);
                // Remove and return the mapping.
                match program.mappings.shift_remove(mapping_name) {
                    Some(mapping) => Ok(mapping),
                    None => bail!("Mapping '{}' not found.", mapping_name),
                }
            }
        }
    }

    /// Removes a struct from the program.
    ///
    /// # Errors
    /// This method will halt if the struct is not in the program.
    #[inline]
    pub fn remove_struct(&mut self, struct_name: &Identifier<N>) -> Result<StructType<N>> {
        match self {
            Self::ProgramV1(program) => {
                // Remove the struct from `identifiers`.
                program.identifiers.shift_remove(struct_name);
                // Remove and return the struct.
                match program.structs.shift_remove(struct_name) {
                    Some(struct_) => Ok(struct_),
                    None => bail!("Struct '{}' not found.", struct_name),
                }
            }
            Self::ProgramV2(program) => {
                // Remove the struct from `identifiers`.
                program.identifiers.shift_remove(struct_name);
                // Remove and return the struct.
                match program.structs.shift_remove(struct_name) {
                    Some(struct_) => Ok(struct_),
                    None => bail!("Struct '{}' not found.", struct_name),
                }
            }
        }
    }

    /// Removes a record from the program.
    ///
    /// # Errors
    /// This method will halt if the record is not in the program.
    #[inline]
    pub fn remove_record(&mut self, record_name: &Identifier<N>) -> Result<RecordType<N>> {
        match self {
            Self::ProgramV1(program) => {
                // Remove the record from `identifiers`.
                program.identifiers.shift_remove(record_name);
                // Remove and return the record.
                match program.records.shift_remove(record_name) {
                    Some(record) => Ok(record),
                    None => bail!("Record '{}' not found.", record_name),
                }
            }
            Self::ProgramV2(program) => {
                // Remove the record from `identifiers`.
                program.identifiers.shift_remove(record_name);
                // Remove and return the record.
                match program.records.shift_remove(record_name) {
                    Some(record) => Ok(record),
                    None => bail!("Record '{}' not found.", record_name),
                }
            }
        }
    }

    /// Removes a closure from the program.
    ///
    /// # Errors
    /// This method will halt if the closure is not in the program.
    #[inline]
    pub fn remove_closure(&mut self, closure_name: &Identifier<N>) -> Result<ClosureCore<N, Instruction>> {
        match self {
            Self::ProgramV1(program) => {
                // Remove the closure from `identifiers`.
                program.identifiers.shift_remove(closure_name);
                // Remove and return the closure.
                match program.closures.shift_remove(closure_name) {
                    Some(closure) => Ok(closure),
                    None => bail!("Closure '{}' not found.", closure_name),
                }
            }
            Self::ProgramV2(program) => {
                // Remove the closure from `identifiers`.
                program.identifiers.shift_remove(closure_name);
                // Remove and return the closure.
                match program.closures.shift_remove(closure_name) {
                    Some(closure) => Ok(closure),
                    None => bail!("Closure '{}' not found.", closure_name),
                }
            }
        }
    }

    /// Removes a function from the program.
    ///
    /// # Errors
    /// This method will halt if the function is not in the program.
    #[inline]
    pub fn remove_function(&mut self, function_name: &Identifier<N>) -> Result<FunctionCore<N, Instruction, Command>> {
        match self {
            Self::ProgramV1(program) => {
                // Remove the function from `identifiers`.
                program.identifiers.shift_remove(function_name);
                // Remove and return the function.
                match program.functions.shift_remove(function_name) {
                    Some(function) => Ok(function),
                    None => bail!("Function '{}' not found.", function_name),
                }
            }
            Self::ProgramV2(program) => {
                // Remove the function from `identifiers`.
                program.identifiers.shift_remove(function_name);
                // Remove and return the function.
                match program.functions.shift_remove(function_name) {
                    Some(function) => Ok(function),
                    None => bail!("Function '{}' not found.", function_name),
                }
            }
        }
    }

    /// Removes the constructor from the program.
    ///
    /// # Errors
    /// This method will halt if the constructor is not in the program.
    #[inline]
    pub fn remove_constructor(&mut self) -> Result<ConstructorCore<N, Command>> {
        match self {
            Self::ProgramV1(_) => bail!("Constructor not found."),
            Self::ProgramV2(program) => {
                // Remove the constructor from the program.
                match program.constructor.take() {
                    Some(constructor) => Ok(constructor),
                    None => bail!("Constructor not found."),
                }
            }
        }
    }

    /// Removes a metadata value from the program.
    ///
    /// # Errors
    /// This method will halt if the metadata is not in the program.
    #[inline]
    pub fn remove_metadata(&mut self, name: &Identifier<N>) -> Result<Value<N>> {
        match self {
            Self::ProgramV1(_) => bail!("Metadata is not supported in V1 programs"),
            Self::ProgramV2(program) => {
                // Remove and return the metadata.
                match program.metadata.shift_remove(name) {
                    Some(metadata) => Ok(metadata),
                    None => bail!("Metadata '{}' not found.", name),
                }
            }
        }
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ProgramCore<N, Instruction, Command> {
    #[rustfmt::skip]
    const KEYWORDS: &'static [&'static str] = &[
        // Mode
        "const",
        "constant",
        "public",
        "private",
        // Literals
        "address",
        "boolean",
        "field",
        "group",
        "i8",
        "i16",
        "i32",
        "i64",
        "i128",
        "u8",
        "u16",
        "u32",
        "u64",
        "u128",
        "scalar",
        "signature",
        "string",
        // Boolean
        "true",
        "false",
        // Statements
        "input",
        "output",
        "as",
        "into",
        // Record
        "record",
        "owner",
        // Program
        "transition",
        "import",
        "function",
        "struct",
        "closure",
        "program",
        "aleo",
        "self",
        "storage",
        "mapping",
        "key",
        "value",
        "async",
        "finalize",
        // Reserved (catch all)
        "global",
        "block",
        "return",
        "break",
        "assert",
        "continue",
        "let",
        "if",
        "else",
        "while",
        "for",
        "switch",
        "case",
        "default",
        "match",
        "enum",
        "struct",
        "union",
        "trait",
        "impl",
        "type",
        "future",
        "_init",
    ];

    /// Returns `true` if the given name does not already exist in the program.
    fn is_unique_name(&self, name: &Identifier<N>) -> bool {
        !self.identifiers().contains_key(name)
    }

    /// Returns `true` if the given name is a reserved opcode.
    pub fn is_reserved_opcode(name: &str) -> bool {
        Instruction::is_reserved_opcode(name)
    }

    /// Returns `true` if the given name uses a reserved keyword.
    pub fn is_reserved_keyword(name: &Identifier<N>) -> bool {
        // Convert the given name to a string.
        let name = name.to_string();
        // Check if the name is a keyword.
        Self::KEYWORDS.iter().any(|keyword| *keyword == name)
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ProgramCore<N, Instruction, Command> {
    /// Returns whether the program is a V1 program.
    #[inline]
    pub fn is_v1(&self) -> bool {
        matches!(self, Self::ProgramV1(_))
    }

    /// Returns the V1 type name as a string.
    #[inline]
    fn type_name_v1() -> &'static str {
        "program"
    }

    /// Returns the V2 type name as a string.
    #[inline]
    fn type_name_v2() -> &'static str {
        "program$2"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::{
        network::MainnetV0,
        program::{Locator, ValueType},
    };

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_program_mapping() -> Result<()> {
        // Create a new mapping.
        let mapping = Mapping::<CurrentNetwork>::from_str(
            r"
mapping message:
    key as field.public;
    value as field.public;",
        )?;

        // Initialize a new program.
        let program = Program::<CurrentNetwork>::from_str(&format!("program unknown.aleo; {mapping}"))?;
        // Ensure the mapping was added.
        assert!(program.contains_mapping(&Identifier::from_str("message")?));
        // Ensure the retrieved mapping matches.
        assert_eq!(mapping.to_string(), program.get_mapping(&Identifier::from_str("message")?)?.to_string());

        Ok(())
    }

    #[test]
    fn test_program_struct() -> Result<()> {
        // Create a new struct.
        let struct_ = StructType::<CurrentNetwork>::from_str(
            r"
struct message:
    first as field;
    second as field;",
        )?;

        // Initialize a new program.
        let program = Program::<CurrentNetwork>::from_str(&format!("program unknown.aleo; {struct_}"))?;
        // Ensure the struct was added.
        assert!(program.contains_struct(&Identifier::from_str("message")?));
        // Ensure the retrieved struct matches.
        assert_eq!(&struct_, program.get_struct(&Identifier::from_str("message")?)?);

        Ok(())
    }

    #[test]
    fn test_program_record() -> Result<()> {
        // Create a new record.
        let record = RecordType::<CurrentNetwork>::from_str(
            r"
record foo:
    owner as address.private;
    first as field.private;
    second as field.public;",
        )?;

        // Initialize a new program.
        let program = Program::<CurrentNetwork>::from_str(&format!("program unknown.aleo; {record}"))?;
        // Ensure the record was added.
        assert!(program.contains_record(&Identifier::from_str("foo")?));
        // Ensure the retrieved record matches.
        assert_eq!(&record, program.get_record(&Identifier::from_str("foo")?)?);

        Ok(())
    }

    #[test]
    fn test_program_function() -> Result<()> {
        // Create a new function.
        let function = Function::<CurrentNetwork>::from_str(
            r"
function compute:
    input r0 as field.public;
    input r1 as field.private;
    add r0 r1 into r2;
    output r2 as field.private;",
        )?;

        // Initialize a new program.
        let program = Program::<CurrentNetwork>::from_str(&format!("program unknown.aleo; {function}"))?;
        // Ensure the function was added.
        assert!(program.contains_function(&Identifier::from_str("compute")?));
        // Ensure the retrieved function matches.
        assert_eq!(function, program.get_function(&Identifier::from_str("compute")?)?);

        Ok(())
    }

    #[test]
    fn test_program_import() -> Result<()> {
        // Initialize a new program.
        let program = Program::<CurrentNetwork>::from_str(
            r"
import eth.aleo;
import usdc.aleo;

program swap.aleo;

// The `swap` function transfers ownership of the record
// for token A to the record owner of token B, and vice-versa.
function swap:
    // Input the record for token A.
    input r0 as eth.aleo/eth.record;
    // Input the record for token B.
    input r1 as usdc.aleo/usdc.record;

    // Send the record for token A to the owner of token B.
    call eth.aleo/transfer r0 r1.owner r0.amount into r2 r3;

    // Send the record for token B to the owner of token A.
    call usdc.aleo/transfer r1 r0.owner r1.amount into r4 r5;

    // Output the new record for token A.
    output r2 as eth.aleo/eth.record;
    // Output the new record for token B.
    output r4 as usdc.aleo/usdc.record;
    ",
        )
        .unwrap();

        // Ensure the program imports exist.
        assert!(program.contains_import(&ProgramID::from_str("eth.aleo")?));
        assert!(program.contains_import(&ProgramID::from_str("usdc.aleo")?));

        // Retrieve the 'swap' function.
        let function = program.get_function(&Identifier::from_str("swap")?)?;

        // Ensure there are two inputs.
        assert_eq!(function.inputs().len(), 2);
        assert_eq!(function.input_types().len(), 2);

        // Declare the expected input types.
        let expected_input_type_1 = ValueType::ExternalRecord(Locator::from_str("eth.aleo/eth")?);
        let expected_input_type_2 = ValueType::ExternalRecord(Locator::from_str("usdc.aleo/usdc")?);

        // Ensure the inputs are external records.
        assert_eq!(function.input_types()[0], expected_input_type_1);
        assert_eq!(function.input_types()[1], expected_input_type_2);

        // Ensure the input variants are correct.
        assert_eq!(function.input_types()[0].variant(), expected_input_type_1.variant());
        assert_eq!(function.input_types()[1].variant(), expected_input_type_2.variant());

        // Ensure there are two instructions.
        assert_eq!(function.instructions().len(), 2);

        // Ensure the instructions are calls.
        assert_eq!(function.instructions()[0].opcode(), Opcode::Call);
        assert_eq!(function.instructions()[1].opcode(), Opcode::Call);

        // Ensure there are two outputs.
        assert_eq!(function.outputs().len(), 2);
        assert_eq!(function.output_types().len(), 2);

        // Declare the expected output types.
        let expected_output_type_1 = ValueType::ExternalRecord(Locator::from_str("eth.aleo/eth")?);
        let expected_output_type_2 = ValueType::ExternalRecord(Locator::from_str("usdc.aleo/usdc")?);

        // Ensure the outputs are external records.
        assert_eq!(function.output_types()[0], expected_output_type_1);
        assert_eq!(function.output_types()[1], expected_output_type_2);

        // Ensure the output variants are correct.
        assert_eq!(function.output_types()[0].variant(), expected_output_type_1.variant());
        assert_eq!(function.output_types()[1].variant(), expected_output_type_2.variant());

        Ok(())
    }
}
