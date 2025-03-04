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

mod metadata;
pub use metadata::*;

pub mod traits;
pub use traits::*;

mod v1;
pub use v1::*;

mod v2;
pub use v2::*;

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
        opt,
        tag,
        take,
    },
    program::{Identifier, Plaintext, ProgramID, RecordType, StructType},
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramVersion {
    /// The V1 program version.
    V1,
    /// The V2 program version.
    V2,
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
        Ok(Self::ProgramV1(ProgramCoreV1::new(id)?))
    }

    /// Initializes an empty V2 program.
    #[inline]
    pub fn new_v2(id: ProgramID<N>) -> Result<Self> {
        Ok(Self::ProgramV2(ProgramCoreV2::new(id)?))
    }

    /// Returns the program as a V1 program.
    #[inline]
    pub fn as_v1(&self) -> Result<&ProgramCoreV1<N, Instruction, Command>> {
        match self {
            Self::ProgramV1(program) => Ok(program),
            Self::ProgramV2(_) => bail!("Program is not a V1 program"),
        }
    }

    /// Returns the program as a V2 program.
    #[inline]
    pub fn as_v2(&self) -> Result<&ProgramCoreV2<N, Instruction, Command>> {
        match self {
            Self::ProgramV1(_) => bail!("Program is not a V2 program"),
            Self::ProgramV2(program) => Ok(program),
        }
    }

    /// Returns the version of the program.
    #[inline]
    pub fn version(&self) -> ProgramVersion {
        match self {
            Self::ProgramV1(_) => ProgramVersion::V1,
            Self::ProgramV2(_) => ProgramVersion::V2,
        }
    }

    /// Initializes the credits program.
    #[inline]
    pub fn credits() -> Result<Self> {
        Self::from_str(include_str!("./resources/credits.aleo"))
    }

    /// Returns the ID of the program.
    pub fn id(&self) -> &ProgramID<N> {
        match &self {
            Self::ProgramV1(program) => program.id(),
            Self::ProgramV2(program) => program.id(),
        }
    }

    /// Returns the imports in the program.
    pub fn imports(&self) -> &IndexMap<ProgramID<N>, Import<N>> {
        match &self {
            Self::ProgramV1(program) => program.imports(),
            Self::ProgramV2(program) => program.imports(),
        }
    }

    /// Returns the mappings in the program.
    pub fn mappings(&self) -> &IndexMap<Identifier<N>, Mapping<N>> {
        match &self {
            Self::ProgramV1(program) => program.mappings(),
            Self::ProgramV2(program) => program.mappings(),
        }
    }

    /// Returns the structs in the program.
    pub fn structs(&self) -> &IndexMap<Identifier<N>, StructType<N>> {
        match &self {
            Self::ProgramV1(program) => program.structs(),
            Self::ProgramV2(program) => program.structs(),
        }
    }

    /// Returns the records in the program.
    pub fn records(&self) -> &IndexMap<Identifier<N>, RecordType<N>> {
        match &self {
            Self::ProgramV1(program) => program.records(),
            Self::ProgramV2(program) => program.records(),
        }
    }

    /// Returns the closures in the program.
    pub fn closures(&self) -> &IndexMap<Identifier<N>, ClosureCore<N, Instruction>> {
        match &self {
            Self::ProgramV1(program) => program.closures(),
            Self::ProgramV2(program) => program.closures(),
        }
    }

    /// Returns the functions in the program.
    pub fn functions(&self) -> &IndexMap<Identifier<N>, FunctionCore<N, Instruction, Command>> {
        match &self {
            Self::ProgramV1(program) => program.functions(),
            Self::ProgramV2(program) => program.functions(),
        }
    }

    /// Returns the constructor in the program.
    pub fn constructor(&self) -> Result<&Option<ConstructorCore<N, Command>>> {
        match &self {
            Self::ProgramV1(_) => bail!("Constructors are not supported in V1 programs"),
            Self::ProgramV2(program) => Ok(program.constructor()),
        }
    }

    /// Returns the metadata in the program.
    pub fn metadata(&self) -> Result<&IndexMap<Identifier<N>, ProgramMetadata<N>>> {
        match &self {
            Self::ProgramV1(_) => bail!("Metadata is not supported in V1 programs"),
            Self::ProgramV2(program) => Ok(program.metadata()),
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
        match self {
            Self::ProgramV1(program) => program.get_mapping(name),
            Self::ProgramV2(program) => program.get_mapping(name),
        }
    }

    /// Returns the struct with the given name.
    pub fn get_struct(&self, name: &Identifier<N>) -> Result<&StructType<N>> {
        match self {
            Self::ProgramV1(program) => program.get_struct(name),
            Self::ProgramV2(program) => program.get_struct(name),
        }
    }

    /// Returns the record with the given name.
    pub fn get_record(&self, name: &Identifier<N>) -> Result<&RecordType<N>> {
        match self {
            Self::ProgramV1(program) => program.get_record(name),
            Self::ProgramV2(program) => program.get_record(name),
        }
    }

    /// Returns the closure with the given name.
    pub fn get_closure(&self, name: &Identifier<N>) -> Result<ClosureCore<N, Instruction>> {
        match self {
            Self::ProgramV1(program) => program.get_closure(name),
            Self::ProgramV2(program) => program.get_closure(name),
        }
    }

    /// Returns the function with the given name.
    pub fn get_function(&self, name: &Identifier<N>) -> Result<FunctionCore<N, Instruction, Command>> {
        match self {
            Self::ProgramV1(program) => program.get_function(name),
            Self::ProgramV2(program) => program.get_function(name),
        }
    }

    /// Returns a reference to the function with the given name.
    pub fn get_function_ref(&self, name: &Identifier<N>) -> Result<&FunctionCore<N, Instruction, Command>> {
        match self {
            Self::ProgramV1(program) => program.get_function_ref(name),
            Self::ProgramV2(program) => program.get_function_ref(name),
        }
    }

    /// Returns the metadata value with the given name.
    pub fn get_metadata(&self, name: &Identifier<N>) -> Result<&ProgramMetadata<N>> {
        match self {
            Self::ProgramV1(_) => bail!("Metadata is not supported in V1 programs"),
            Self::ProgramV2(program) => program.get_metadata(name),
        }
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ProgramCore<N, Instruction, Command> {
    /// Adds a new import statement to the program.
    #[inline]
    pub fn add_import(&mut self, import: Import<N>) -> Result<()> {
        match self {
            Self::ProgramV1(program) => program.add_import(import),
            Self::ProgramV2(program) => program.add_import(import),
        }
    }

    /// Adds a new mapping to the program.
    #[inline]
    pub fn add_mapping(&mut self, mapping: Mapping<N>) -> Result<()> {
        match self {
            Self::ProgramV1(program) => program.add_mapping(mapping),
            Self::ProgramV2(program) => program.add_mapping(mapping),
        }
    }

    /// Adds a new struct to the program.
    #[inline]
    pub fn add_struct(&mut self, struct_: StructType<N>) -> Result<()> {
        match self {
            Self::ProgramV1(program) => program.add_struct(struct_),
            Self::ProgramV2(program) => program.add_struct(struct_),
        }
    }

    /// Adds a new record to the program.
    #[inline]
    pub fn add_record(&mut self, record: RecordType<N>) -> Result<()> {
        match self {
            Self::ProgramV1(program) => program.add_record(record),
            Self::ProgramV2(program) => program.add_record(record),
        }
    }

    /// Adds a new closure to the program.
    #[inline]
    pub fn add_closure(&mut self, closure: ClosureCore<N, Instruction>) -> Result<()> {
        match self {
            Self::ProgramV1(program) => program.add_closure(closure),
            Self::ProgramV2(program) => program.add_closure(closure),
        }
    }

    /// Adds a new function to the program.
    #[inline]
    pub fn add_function(&mut self, function: FunctionCore<N, Instruction, Command>) -> Result<()> {
        match self {
            Self::ProgramV1(program) => program.add_function(function),
            Self::ProgramV2(program) => program.add_function(function),
        }
    }

    /// Updates the constructor in the program.
    pub fn add_constructor(&mut self, constructor: ConstructorCore<N, Command>) -> Result<()> {
        match self {
            Self::ProgramV1(_) => bail!("Cannot add a constructor to a V1 program"),
            Self::ProgramV2(program) => program.add_constructor(constructor),
        }
    }

    /// Adds a new metadata value to the program.
    pub fn add_metadata(&mut self, metadata: ProgramMetadata<N>) -> Result<()> {
        match self {
            Self::ProgramV1(_) => bail!("Metadata is not supported in V1 programs"),
            Self::ProgramV2(program) => program.add_metadata(metadata),
        }
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ProgramReserved<N, Instruction>
    for ProgramCore<N, Instruction, Command>
{
}
