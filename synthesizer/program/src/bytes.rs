// Copyright 2024-2025 Aleo Network Foundation
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
            1 => Self::ProgramV1(ProgramCoreV1::read_le_internal(&mut reader).map_err(|e| error(e.to_string()))?),
            2 => Self::ProgramV2(ProgramCoreV2::read_le_internal(&mut reader).map_err(|e| error(e.to_string()))?),
            invalid_version => return Err(error(format!("Invalid program version ({invalid_version})"))),
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
                program.write_le_internal(&mut writer)
            }
            Self::ProgramV2(program) => {
                2u8.write_le(&mut writer)?;
                program.write_le_internal(&mut writer)
            }
        }
    }
}
