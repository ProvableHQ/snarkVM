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

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> FromBytes
    for ProgramCore<N, Instruction, Command>
{
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;

        // Ensure the version is valid and initialize the program.
        let program = match version {
            1 => Self::ProgramV1(ProgramCoreV1::read_le(&mut reader).map_err(|e| error(e.to_string()))?),
            2 => Self::ProgramV2(ProgramCoreV2::read_le(&mut reader).map_err(|e| error(e.to_string()))?),
            _ => return Err(error("Invalid program version")),
        };

        // Return the program.
        Ok(program)
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ToBytes
    for ProgramCore<N, Instruction, Command>
{
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version and the variant.
        match self {
            Self::ProgramV1(program) => {
                1u8.write_le(&mut writer)?;
                program.write_le(&mut writer)
            }
            Self::ProgramV2(program) => {
                2u8.write_le(&mut writer)?;
                program.write_le(&mut writer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Program;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_bytes() -> Result<()> {
        let program = r"
program token.aleo;

record token:
    owner as address.private;
    token_amount as u64.private;

_init:
    assert.eq true false;

function compute:
    input r0 as token.record;
    add r0.token_amount r0.token_amount into r1;
    output r1 as u64.private;";

        // Initialize a new program.
        let (string, expected) = Program::<CurrentNetwork>::parse(program).unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");

        let expected_bytes = expected.to_bytes_le()?;

        let candidate = Program::<CurrentNetwork>::from_bytes_le(&expected_bytes)?;
        assert_eq!(expected, candidate);
        assert_eq!(expected_bytes, candidate.to_bytes_le()?);

        Ok(())
    }
}
