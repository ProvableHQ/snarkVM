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

use crate::{
    Opcode,
    traits::{FinalizeStoreTrait, RegistersLoad, RegistersStore, StackMatches, StackProgram},
};
use console::{
    network::prelude::*,
    program::{Identifier, Literal, LiteralType, Plaintext, PlaintextType, ProgramID, Register, Value},
};

/// The name of the metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MetadataName<N: Network> {
    /// The program checksum.
    Checksum,
    /// A declared metadata identifier.
    Identifier(Identifier<N>),
}

/// A command to get metadata about a program, e.g. `metadata.get program_owner into r1 as address;`.
/// Gets the value with the `name` from the program and stores it in the `destination` register.
/// The value is checked to be of the `destination_type`.
///
/// `metadata.get _checksum into r1 as u128;` is a special case where the metadata is not retrieved from the program.
/// Instead, the checksum of the program is calculated and stored in the destination register.
///
/// Note that other than the `checksum`, metadata can only be retrieved for V2 programs.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MetadataGet<N: Network> {
    /// The optional program name.
    program: Option<ProgramID<N>>,
    /// The name of the metadata.
    name: MetadataName<N>,
    /// The destination register.
    destination: Register<N>,
    /// The destination type.
    destination_type: LiteralType,
}

impl<N: Network> MetadataGet<N> {
    /// Returns the opcode.
    #[inline]
    pub const fn opcode() -> Opcode {
        Opcode::Command("metadata.get")
    }

    /// Returns the program.
    #[inline]
    pub const fn program(&self) -> Option<&ProgramID<N>> {
        self.program.as_ref()
    }

    /// Returns the name.
    #[inline]
    pub const fn name(&self) -> &MetadataName<N> {
        &self.name
    }

    /// Returns the destination register.
    #[inline]
    pub const fn destination(&self) -> &Register<N> {
        &self.destination
    }

    /// Returns the destination type.
    #[inline]
    pub const fn destination_type(&self) -> &LiteralType {
        &self.destination_type
    }
}

impl<N: Network> MetadataGet<N> {
    /// Finalizes the command.
    #[inline]
    pub fn finalize(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        _store: &impl FinalizeStoreTrait<N>,
        registers: &mut (impl RegistersLoad<N> + RegistersStore<N>),
    ) -> Result<()> {
        // Get the metadata literal.
        let literal = match &self.name {
            MetadataName::Checksum => Literal::U128(match &self.program {
                Some(program) => *stack.get_external_stack(program)?.program_checksum(),
                None => *stack.program_checksum(),
            }),
            MetadataName::Identifier(identifier) => match &self.program {
                Some(program) => stack.get_external_stack(program)?.program().get_metadata(identifier)?.value().clone(),
                None => stack.program().get_metadata(identifier)?.value().clone(),
            },
        };
        let plaintext = Plaintext::from(literal);
        // Check that retrieved metadata is of the correct type.
        stack.matches_plaintext(&plaintext, &PlaintextType::Literal(*self.destination_type()))?;
        // Assign the value to the destination register.
        registers.store(stack, &self.destination, Value::Plaintext(plaintext))?;

        Ok(())
    }
}

impl<N: Network> Parser for MetadataGet<N> {
    /// Parses a string into the command.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;
        // Parse the opcode from the string.
        let (string, _) = tag(*Self::opcode())(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;

        // Optionally parse the program and the "/" separator from the string.
        let (string, (program, _)) = pair(opt(ProgramID::parse), opt(tag("/")))(string)?;
        // Parse the metadata name from the string.
        let (string, name) = alt((
            map(tag("_checksum"), |_| MetadataName::Checksum),
            map(Identifier::parse, MetadataName::Identifier),
        ))(string)?;

        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the "into" keyword from the string.
        let (string, _) = tag("into")(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the destination register from the string.
        let (string, destination) = Register::parse(string)?;

        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the "as" keyword from the string.
        let (string, _) = tag("as")(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the destination type from the string.
        let (string, destination_type) = LiteralType::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the ";" from the string.
        let (string, _) = tag(";")(string)?;

        Ok((string, Self { program, name, destination, destination_type }))
    }
}

impl<N: Network> FromStr for MetadataGet<N> {
    type Err = Error;

    /// Parses a string into the command.
    #[inline]
    fn from_str(string: &str) -> Result<Self> {
        match Self::parse(string) {
            Ok((remainder, object)) => {
                // Ensure the remainder is empty.
                ensure!(remainder.is_empty(), "Failed to parse string. Found invalid character in: \"{remainder}\"");
                // Return the object.
                Ok(object)
            }
            Err(error) => bail!("Failed to parse string. {error}"),
        }
    }
}

impl<N: Network> Debug for MetadataGet<N> {
    /// Prints the command as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for MetadataGet<N> {
    /// Prints the command to a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // Format the program.
        let program = match &self.program {
            Some(program) => format!("{program}/"),
            None => String::new(),
        };
        // Format the name.
        let name = match &self.name {
            MetadataName::Checksum => "_checksum".to_string(),
            MetadataName::Identifier(identifier) => identifier.to_string(),
        };
        // Print the command.
        write!(f, "{} {program}{name} into {} as {};", Self::opcode(), self.destination, self.destination_type)
    }
}

impl<N: Network> FromBytes for MetadataGet<N> {
    /// Reads the command from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the program.
        let option = u8::read_le(&mut reader)?;
        let program = match option {
            0 => None,
            1 => Some(ProgramID::read_le(&mut reader)?),
            _ => return Err(error("Failed to read program ID. Invalid option.")),
        };
        // Read the name.
        let variant = u8::read_le(&mut reader)?;
        let name = match variant {
            0 => MetadataName::Checksum,
            1 => MetadataName::Identifier(Identifier::read_le(&mut reader)?),
            _ => return Err(error("Failed to read metadata name. Invalid variant: {variant}")),
        };
        // Read the destination register.
        let destination = Register::read_le(&mut reader)?;
        // Read the destination type.
        let destination_type = LiteralType::read_le(&mut reader)?;
        // Return the command.
        Ok(Self { program, name, destination, destination_type })
    }
}

impl<N: Network> ToBytes for MetadataGet<N> {
    /// Writes the command to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the program.
        match &self.program {
            None => 0u8.write_le(&mut writer)?,
            Some(program) => {
                1u8.write_le(&mut writer)?;
                program.write_le(&mut writer)?;
            }
        }
        // Write the name.
        match &self.name {
            MetadataName::Checksum => 0u8.write_le(&mut writer)?,
            MetadataName::Identifier(identifier) => {
                1u8.write_le(&mut writer)?;
                identifier.write_le(&mut writer)?;
            }
        }
        // Write the destination register.
        self.destination.write_le(&mut writer)?;
        // Write the destination type.
        self.destination_type.write_le(&mut writer)?;
        // Return success.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::{
        network::MainnetV0,
        program::{LiteralType, Register},
    };

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_parse() {
        let (string, metadata_get) =
            MetadataGet::<CurrentNetwork>::parse("metadata.get edition into r1 as u16;").unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");
        assert_eq!(metadata_get.program(), None);
        assert_eq!(metadata_get.name(), &MetadataName::Identifier(Identifier::from_str("edition").unwrap()));
        assert_eq!(metadata_get.destination, Register::Locator(1), "The destination is incorrect");
        assert_eq!(metadata_get.destination_type, LiteralType::U16, "The destination type is incorrect");

        let (string, metadata_get) =
            MetadataGet::<CurrentNetwork>::parse("metadata.get token.aleo/bar into r1 as u16;").unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");
        assert_eq!(metadata_get.program(), Some(&ProgramID::from_str("token.aleo").unwrap()));
        assert_eq!(metadata_get.name(), &MetadataName::Identifier(Identifier::from_str("bar").unwrap()));
        assert_eq!(metadata_get.destination, Register::Locator(1), "The destination is incorrect");
        assert_eq!(metadata_get.destination_type, LiteralType::U16, "The destination type is incorrect");

        let (string, metadata_get) =
            MetadataGet::<CurrentNetwork>::parse("metadata.get _checksum into r1 as u128;").unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");
        assert_eq!(metadata_get.program(), None);
        assert_eq!(metadata_get.name(), &MetadataName::Checksum);
        assert_eq!(metadata_get.destination, Register::Locator(1), "The destination is incorrect");
        assert_eq!(metadata_get.destination_type, LiteralType::U128, "The destination type is incorrect");
    }

    #[test]
    fn test_from_bytes() {
        let (string, get) = MetadataGet::<CurrentNetwork>::parse("metadata.get foo into r1 as u16;").unwrap();
        assert!(string.is_empty());
        let bytes_le = get.to_bytes_le().unwrap();
        let result = MetadataGet::<CurrentNetwork>::from_bytes_le(&bytes_le[..]);
        assert!(result.is_ok())
    }
}
