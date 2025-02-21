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
    CallOperator,
    Opcode,
    traits::{FinalizeStoreTrait, RegistersLoad, RegistersStore, StackMatches, StackProgram},
};
use console::{
    network::prelude::*,
    program::{Literal, Plaintext, PlaintextType, Register, Value},
    types::U16,
};

/// A command to get metadata about a program, e.g. `metadata.get owner into r1;`.
/// Gets the value stored at `global` and stores the result in `destination`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MetadataGet<N: Network> {
    /// The global ID.
    name: CallOperator<N>,
    /// The destination register.
    destination: Register<N>,
    /// The destination type.
    destination_type: PlaintextType<N>,
}

impl<N: Network> MetadataGet<N> {
    /// Returns the opcode.
    #[inline]
    pub const fn opcode() -> Opcode {
        Opcode::Command("metadata.get")
    }

    /// Returns the name.
    #[inline]
    pub const fn name(&self) -> &CallOperator<N> {
        &self.name
    }

    /// Returns the destination register.
    #[inline]
    pub const fn destination(&self) -> &Register<N> {
        &self.destination
    }

    /// Returns the destination type.
    #[inline]
    pub const fn destination_type(&self) -> &PlaintextType<N> {
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
        // Determine the program ID and global ID.
        let (external_stack, global_name) = match self.name {
            CallOperator::Locator(locator) => {
                (Some(stack.get_external_stack(locator.program_id())?), *locator.resource())
            }
            CallOperator::Resource(global_name) => (None, global_name),
        };
        // Get the value from the program metadata.
        let value = match external_stack {
            Some(external_stack) => external_stack.program().get_metadata(&global_name)?.value().clone(),
            None => stack.program().get_metadata(&global_name)?.value().clone(),
        };
        // Check that retrieved metadata is of the correct type.
        stack.matches_plaintext(&value, self.destination_type())?;
        // Assign the value to the destination register.
        registers.store(stack, &self.destination, Value::Plaintext(value))?;

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

        // Parse the name from the string.
        let (string, name) = CallOperator::parse(string)?;

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
        let (string, destination_type) = PlaintextType::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the ";" from the string.
        let (string, _) = tag(";")(string)?;

        Ok((string, Self { name, destination, destination_type }))
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
        // Print the command.
        write!(f, "{} ", Self::opcode())?;
        // Print the global ID.
        write!(f, "{} into ", self.name)?;
        // Print the destination register.
        write!(f, "{};", self.destination)
    }
}

impl<N: Network> FromBytes for MetadataGet<N> {
    /// Reads the command from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the name.
        let name = CallOperator::read_le(&mut reader)?;
        // Read the destination register.
        let destination = Register::read_le(&mut reader)?;
        // Read the destination type.
        let destination_type = PlaintextType::read_le(&mut reader)?;
        // Return the command.
        Ok(Self { name, destination, destination_type })
    }
}

impl<N: Network> ToBytes for MetadataGet<N> {
    /// Writes the command to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the name.
        self.name.write_le(&mut writer)?;
        // Write the destination register.
        self.destination.write_le(&mut writer)?;
        // Write the destination type.
        self.destination_type.write_le(&mut writer)
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
        assert_eq!(metadata_get.name(), &CallOperator::from_str("edition").unwrap());
        assert_eq!(metadata_get.destination, Register::Locator(1), "The destination is incorrect");
        assert_eq!(
            metadata_get.destination_type,
            PlaintextType::Literal(LiteralType::U16),
            "The destination type is incorrect"
        );

        let (string, metadata_get) =
            MetadataGet::<CurrentNetwork>::parse("metadata.get token.aleo/edition into r1 as u16;").unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");
        assert_eq!(metadata_get.name(), &CallOperator::from_str("token.aleo/edition").unwrap());
        assert_eq!(metadata_get.destination, Register::Locator(1), "The destination is incorrect");
        assert_eq!(
            metadata_get.destination_type,
            PlaintextType::Literal(LiteralType::U16),
            "The destination type is incorrect"
        );
    }

    #[test]
    fn test_from_bytes() {
        let (string, get) = MetadataGet::<CurrentNetwork>::parse("metadata.get edition into r1;").unwrap();
        assert!(string.is_empty());
        let bytes_le = get.to_bytes_le().unwrap();
        let result = MetadataGet::<CurrentNetwork>::from_bytes_le(&bytes_le[..]);
        assert!(result.is_ok())
    }
}
