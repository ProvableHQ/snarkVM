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
    program::{Identifier, Plaintext, PlaintextType, ProgramID, RecordType, StructType, Value},
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

    /// Initializes the credits program.
    #[inline]
    pub fn credits() -> Result<Self> {
        Self::from_str(include_str!("./resources/credits.aleo"))
    }

    /// Returns the ID of the program.
    pub const fn id(&self) -> &ProgramID<N> {
        match &self {
            Self::ProgramV1(program) => program.id(),
            Self::ProgramV2(program) => program.id(),
        }
    }

    /// Returns the imports in the program.
    pub const fn imports(&self) -> &IndexMap<ProgramID<N>, Import<N>> {
        match &self {
            Self::ProgramV1(program) => program.imports(),
            Self::ProgramV2(program) => program.imports(),
        }
    }

    /// Returns the mappings in the program.
    pub const fn mappings(&self) -> &IndexMap<Identifier<N>, Mapping<N>> {
        match &self {
            Self::ProgramV1(program) => program.mappings(),
            Self::ProgramV2(program) => program.mappings(),
        }
    }

    /// Returns the structs in the program.
    pub const fn structs(&self) -> &IndexMap<Identifier<N>, StructType<N>> {
        match &self {
            Self::ProgramV1(program) => program.structs(),
            Self::ProgramV2(program) => program.structs(),
        }
    }

    /// Returns the records in the program.
    pub const fn records(&self) -> &IndexMap<Identifier<N>, RecordType<N>> {
        match &self {
            Self::ProgramV1(program) => program.records(),
            Self::ProgramV2(program) => program.records(),
        }
    }

    /// Returns the closures in the program.
    pub const fn closures(&self) -> &IndexMap<Identifier<N>, ClosureCore<N, Instruction>> {
        match &self {
            Self::ProgramV1(program) => program.closures(),
            Self::ProgramV2(program) => program.closures(),
        }
    }

    /// Returns the functions in the program.
    pub const fn functions(&self) -> &IndexMap<Identifier<N>, FunctionCore<N, Instruction, Command>> {
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
    pub fn metadata(&self) -> Result<&IndexMap<Identifier<N>, Metadata<N>>> {
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
    pub fn get_metadata(&self, name: &Identifier<N>) -> Result<&Metadata<N>> {
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
    pub fn add_metadata(&mut self, metadata: Metadata<N>) -> Result<()> {
        match self {
            Self::ProgramV1(_) => bail!("Metadata is not supported in V1 programs"),
            Self::ProgramV2(program) => program.add_metadata(metadata),
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
