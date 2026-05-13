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

use crate::{FinalizeOperation, Opcode, Operand, RegistersTrait, StackTrait};
use console::network::prelude::*;

/// An emit command, e.g. `emit r0;`
/// Appends the plaintext value of `r0` as an event to the transition's finalize operations.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Emit<N: Network> {
    /// The operand whose plaintext value is emitted.
    operands: [Operand<N>; 1],
}

impl<N: Network> Emit<N> {
    /// Returns the opcode.
    #[inline]
    pub const fn opcode() -> Opcode {
        Opcode::Command("emit")
    }

    /// Returns the operands in the operation.
    #[inline]
    pub fn operands(&self) -> &[Operand<N>] {
        &self.operands
    }

    /// Returns the operand whose value is emitted.
    #[inline]
    pub const fn value(&self) -> &Operand<N> {
        &self.operands[0]
    }

    /// Returns whether this command refers to an external struct.
    #[inline]
    pub fn contains_external_struct(&self) -> bool {
        false
    }
}

impl<N: Network> Emit<N> {
    /// Finalizes the command, returning the `EmitEvent` finalize operation.
    pub fn finalize(
        &self,
        stack: &impl StackTrait<N>,
        registers: &mut impl RegistersTrait<N>,
    ) -> Result<FinalizeOperation<N>> {
        // Load the operand as a plaintext (rejects records and futures).
        let plaintext = registers.load_plaintext(stack, self.value())?;
        // Return the emit-event finalize operation.
        Ok(FinalizeOperation::EmitEvent(Box::new(plaintext)))
    }
}

impl<N: Network> Parser for Emit<N> {
    /// Parses a string into an operation.
    fn parse(string: &str) -> ParserResult<Self> {
        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;
        // Parse the opcode from the string.
        let (string, _) = tag(*Self::opcode())(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the operand from the string.
        let (string, value) = Operand::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the ";" from the string.
        let (string, _) = tag(";")(string)?;

        Ok((string, Self { operands: [value] }))
    }
}

impl<N: Network> FromStr for Emit<N> {
    type Err = Error;

    /// Parses a string into the command.
    #[inline]
    fn from_str(string: &str) -> Result<Self> {
        match Self::parse(string) {
            Ok((remainder, object)) => {
                ensure!(remainder.is_empty(), "Failed to parse string. Found invalid character in: \"{remainder}\"");
                Ok(object)
            }
            Err(error) => bail!("Failed to parse string. {error}"),
        }
    }
}

impl<N: Network> Debug for Emit<N> {
    /// Prints the command as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for Emit<N> {
    /// Prints the command to a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{} {};", Self::opcode(), self.value())
    }
}

impl<N: Network> FromBytes for Emit<N> {
    /// Reads the command from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let value = Operand::read_le(&mut reader)?;
        Ok(Self { operands: [value] })
    }
}

impl<N: Network> ToBytes for Emit<N> {
    /// Writes the operation to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.value().write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::{network::MainnetV0, program::Register};

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_parse() {
        let (string, emit) = Emit::<CurrentNetwork>::parse("emit r0;").unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");
        assert_eq!(emit.operands().len(), 1, "The number of operands is incorrect");
        assert_eq!(emit.value(), &Operand::Register(Register::Locator(0)), "The operand is incorrect");
    }

    #[test]
    fn test_display() {
        let emit = Emit::<CurrentNetwork>::from_str("emit r3;").unwrap();
        assert_eq!(emit.to_string(), "emit r3;");
    }

    #[test]
    fn test_bytes() {
        let emit = Emit::<CurrentNetwork>::from_str("emit r2;").unwrap();
        let bytes = emit.to_bytes_le().unwrap();
        let decoded = Emit::<CurrentNetwork>::read_le(&bytes[..]).unwrap();
        assert_eq!(emit, decoded);
    }
}
