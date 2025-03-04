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
use console::program::{Address, Boolean, Literal, U16};

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
    metadata: IndexMap<Identifier<N>, ProgramMetadata<N>>,
}

impl_standard_program!(ProgramCoreV2);

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

    /// Returns the constructor for the program.
    pub fn constructor(&self) -> &Option<ConstructorCore<N, Command>> {
        &self.constructor
    }

    /// Returns the metadata for the program.
    pub fn metadata(&self) -> &IndexMap<Identifier<N>, ProgramMetadata<N>> {
        &self.metadata
    }

    /// Returns the metadata value with the given name.
    pub fn get_metadata(&self, name: &Identifier<N>) -> Result<&ProgramMetadata<N>> {
        // Attempt to retrieve the metadata value.
        let metadata = self.metadata.get(name).ok_or_else(|| anyhow!("Metadata '{name}' is not defined."))?;
        // Ensure the metadata name matches.
        ensure!(metadata.name() == name, "Expected metadata '{name}', but found metadata '{}'", metadata.name());
        // Return the metadata value.
        Ok(metadata)
    }

    /// Returns the `edition` metadata value.
    pub fn get_edition_metadata(&self) -> Result<&U16<N>> {
        // Attempt to retrieve the metadata value.
        let metadata = self
            .metadata
            .get(&Identifier::from_str("edition")?)
            .ok_or_else(|| anyhow!("Metadata 'edition' is not defined."))?;
        // Destructure the edition.
        let edition = match metadata.value() {
            Plaintext::Literal(Literal::U16(edition), _) => edition,
            _ => bail!("Metadata 'edition' is not a valid 'u16' value."),
        };
        // Return the edition.
        Ok(edition)
    }

    /// Returns the `owner` metadata value.
    pub fn get_owner_metadata(&self) -> Result<&Address<N>> {
        // Attempt to retrieve the metadata value.
        let metadata = self
            .metadata
            .get(&Identifier::from_str("program_owner")?)
            .ok_or_else(|| anyhow!("Metadata 'program_owner' is not defined."))?;
        // Destructure the owner.
        let owner = match metadata.value() {
            Plaintext::Literal(Literal::Address(owner), _) => owner,
            _ => bail!("Metadata 'program_owner' is not a valid 'address' value."),
        };
        // Return the owner.
        Ok(owner)
    }

    /// Returns the `upgradable` metadata value.
    pub fn get_upgradable_metadata(&self) -> Result<&Boolean<N>> {
        // Attempt to retrieve the metadata value.
        let metadata = self
            .metadata
            .get(&Identifier::from_str("upgradable")?)
            .ok_or_else(|| anyhow!("Metadata 'upgradable' is not defined."))?;
        // Destructure the upgradable.
        let upgradable = match metadata.value() {
            Plaintext::Literal(Literal::Boolean(upgradable), _) => upgradable,
            _ => bail!("Metadata 'upgradable' is not a valid 'boolean' value."),
        };
        // Return the upgradable.
        Ok(upgradable)
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ProgramCoreV2<N, Instruction, Command> {
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
    pub fn add_metadata(&mut self, metadata: ProgramMetadata<N>) -> Result<()> {
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

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> TypeName
    for ProgramCoreV2<N, Instruction, Command>
{
    /// Returns the type name as a string.
    #[inline]
    fn type_name() -> &'static str {
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
        let program = Program::<CurrentNetwork>::from_str(&format!("program$2 unknown.aleo; {mapping}"))?;
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
        let program = Program::<CurrentNetwork>::from_str(&format!("program$2 unknown.aleo; {struct_}"))?;
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
        let program = Program::<CurrentNetwork>::from_str(&format!("program$2 unknown.aleo; {record}"))?;
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
        let program = Program::<CurrentNetwork>::from_str(&format!("program$2 unknown.aleo; {function}"))?;
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

program$2 swap.aleo;

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

_init:
    assert.eq true true;

$metadata version: 0u8;
$metadata authority: aleo1rhgdu77hgyqd3xjj8ucu3jj9r2krwz6mnzyd80gncr5fxcwlh5rsvzp9px;
   ",
        )
        .unwrap();
        assert_eq!(program.version(), ProgramVersion::V2);

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
