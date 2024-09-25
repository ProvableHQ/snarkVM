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

impl<N: Network> FromBytes for Instruction<N> {
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        /// Creates a match statement that produces the `FromBytes` implementation for the given instruction.
        ///
        /// ## Example
        /// ```ignore
        /// instruction_from_bytes_le!(self, |reader| {}, { Add, Sub, Mul, Div })
        /// ```
        macro_rules! instruction_from_bytes_le {
            ($object:expr, |$reader:ident| $_operation:block, { $( $variant:ident, )+ }) => {{
                // Read the opcode index.
                let index = u16::read_le(&mut $reader)?;

                // Build the cases for all instructions.
                if index as usize >= Instruction::<N>::OPCODES.len() {
                    return Err(error(format!("Failed to deserialize an instruction: invalid opcode index ({index})")));
                }
                $(if Instruction::<N>::OPCODES[index as usize] == $variant::<N>::opcode() {
                    // Read the instruction.
                    let instruction = $variant::read_le(&mut $reader)?;
                    // Return the instruction.
                    return Ok(Self::$variant(instruction));
                })+
                // If the index is out of bounds, return an error.
                Err(error(format!("Failed to deserialize an instruction of opcode index '{index}'")))
            }};
        }
        // Execute the `from_bytes_le` method.
        crate::instruction!(instruction_from_bytes_le!(self, reader))
    }
}

impl<N: Network> ToBytes for Instruction<N> {
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        /// Creates a match statement that produces the `ToBytes` implementation for the given instruction.
        ///
        /// ## Example
        /// ```ignore
        /// instruction_to_bytes_le!(self, |writer| {}, { Add, Sub, Mul, Div })
        /// ```
        macro_rules! instruction_to_bytes_le {
            ($object:expr, |$writer:ident| $_operation:block, { $( $variant:ident, )+ }) => {{
                // Build the match cases.
                match $object {
                    $(Self::$variant(instruction) => {
                        // Retrieve the opcode index.
                        // Note: This unwrap is guaranteed to succeed because the opcode variant is known to exist.
                        let index = Instruction::<N>::OPCODES.iter().position(|&opcode| $variant::<N>::opcode() == opcode).unwrap();

                        // Serialize the instruction.
                        u16::write_le(&(index as u16),&mut $writer)?;
                        instruction.write_le(&mut $writer)?;
                    }),+
                }
                Ok(())
            }};
        }
        // Execute the `to_bytes_le` method.
        crate::instruction!(instruction_to_bytes_le!(self, writer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_bytes() -> Result<()> {
        let instruction = "add r0 r1 into r2;";
        let expected = Instruction::<CurrentNetwork>::from_str(instruction)?;
        let expected_bytes = expected.to_bytes_le()?;

        let candidate = Instruction::<CurrentNetwork>::from_bytes_le(&expected_bytes)?;
        assert_eq!(expected_bytes, candidate.to_bytes_le()?);
        Ok(())
    }
}
