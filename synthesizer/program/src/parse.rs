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

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> Parser
    for ProgramCore<N, Instruction, Command>
{
    /// Parses a string into a program.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        alt((map(ProgramCoreV1::parse_internal, Self::ProgramV1), map(ProgramCoreV2::parse_internal, Self::ProgramV2)))(
            string,
        )
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> FromStr
    for ProgramCore<N, Instruction, Command>
{
    type Err = Error;

    /// Returns a program from a string literal.
    fn from_str(string: &str) -> Result<Self> {
        // Ensure the raw program string is less than MAX_PROGRAM_SIZE.
        ensure!(
            string.len() <= N::MAX_PROGRAM_SIZE,
            "Program length '{}' exceeds '{}'.",
            string.len(),
            N::MAX_PROGRAM_SIZE
        );

        match Self::parse(string) {
            Ok((remainder, object)) => {
                // Ensure the remainder is empty.
                ensure!(remainder.is_empty(), "Failed to parse string. Remaining invalid string is: \"{remainder}\"");
                // Return the object.
                Ok(object)
            }
            Err(error) => bail!("Failed to parse string. {error}"),
        }
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> Debug
    for ProgramCore<N, Instruction, Command>
{
    /// Prints the program as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> Display
    for ProgramCore<N, Instruction, Command>
{
    /// Prints the program as a string.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self {
            Self::ProgramV1(program) => Display::fmt(program, f),
            Self::ProgramV2(program) => Display::fmt(program, f),
        }
    }
}
