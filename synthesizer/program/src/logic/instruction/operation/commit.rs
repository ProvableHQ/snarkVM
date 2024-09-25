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
    traits::{RegistersLoad, RegistersLoadCircuit, RegistersStore, RegistersStoreCircuit, StackMatches, StackProgram},
    Opcode,
    Operand,
};
use console::{
    network::prelude::*,
    program::{Literal, LiteralType, Plaintext, PlaintextType, Register, RegisterType, Value},
};

/// BHP256 is a collision-resistant function that processes inputs in 256-bit chunks.
pub type CommitBHP256<N> = CommitInstruction<N, { Committer::CommitBHP256 as u8 }>;
/// BHP512 is a collision-resistant function that processes inputs in 512-bit chunks.
pub type CommitBHP512<N> = CommitInstruction<N, { Committer::CommitBHP512 as u8 }>;
/// BHP768 is a collision-resistant function that processes inputs in 768-bit chunks.
pub type CommitBHP768<N> = CommitInstruction<N, { Committer::CommitBHP768 as u8 }>;
/// BHP1024 is a collision-resistant function that processes inputs in 1024-bit chunks.
pub type CommitBHP1024<N> = CommitInstruction<N, { Committer::CommitBHP1024 as u8 }>;

/// Pedersen64 is a collision-resistant function that processes inputs in 64-bit chunks.
pub type CommitPED64<N> = CommitInstruction<N, { Committer::CommitPED64 as u8 }>;
/// Pedersen128 is a collision-resistant function that processes inputs in 128-bit chunks.
pub type CommitPED128<N> = CommitInstruction<N, { Committer::CommitPED128 as u8 }>;

enum Committer {
    CommitBHP256,
    CommitBHP512,
    CommitBHP768,
    CommitBHP1024,
    CommitPED64,
    CommitPED128,
}

/// Returns 'true' if the destination type is valid.
fn is_valid_destination_type(destination_type: LiteralType) -> bool {
    matches!(destination_type, LiteralType::Address | LiteralType::Field | LiteralType::Group)
}

/// Commits the operand into the declared type.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CommitInstruction<N: Network, const VARIANT: u8> {
    /// The operand as `input`.
    operands: Vec<Operand<N>>,
    /// The destination register.
    destination: Register<N>,
    /// The destination register type.
    destination_type: LiteralType,
}

impl<N: Network, const VARIANT: u8> CommitInstruction<N, VARIANT> {
    /// Initializes a new `commit` instruction.
    #[inline]
    pub fn new(operands: Vec<Operand<N>>, destination: Register<N>, destination_type: LiteralType) -> Result<Self> {
        // Sanity check that the operands is exactly two inputs.
        ensure!(operands.len() == 2, "Commit instructions must have two operands");
        // Sanity check the destination type.
        ensure!(is_valid_destination_type(destination_type), "Invalid destination type for 'commit' instruction");
        // Return the instruction.
        Ok(Self { operands, destination, destination_type })
    }

    /// Returns the opcode.
    #[inline]
    pub const fn opcode() -> Opcode {
        match VARIANT {
            0 => Opcode::Commit("commit.bhp256"),
            1 => Opcode::Commit("commit.bhp512"),
            2 => Opcode::Commit("commit.bhp768"),
            3 => Opcode::Commit("commit.bhp1024"),
            4 => Opcode::Commit("commit.ped64"),
            5 => Opcode::Commit("commit.ped128"),
            6.. => panic!("Invalid 'commit' instruction opcode"),
        }
    }

    /// Returns the operands in the operation.
    #[inline]
    pub fn operands(&self) -> &[Operand<N>] {
        // Sanity check that the operands is exactly two inputs.
        debug_assert!(self.operands.len() == 2, "Commit operations must have two operands");
        // Return the operands.
        &self.operands
    }

    /// Returns the destination register.
    #[inline]
    pub fn destinations(&self) -> Vec<Register<N>> {
        vec![self.destination.clone()]
    }

    /// Returns the destination register type.
    #[inline]
    pub const fn destination_type(&self) -> LiteralType {
        self.destination_type
    }
}

impl<N: Network, const VARIANT: u8> CommitInstruction<N, VARIANT> {
    /// Evaluates the instruction.
    #[inline]
    pub fn evaluate(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        registers: &mut (impl RegistersLoad<N> + RegistersStore<N>),
    ) -> Result<()> {
        // Ensure the number of operands is correct.
        if self.operands.len() != 2 {
            bail!("Instruction '{}' expects 2 operands, found {} operands", Self::opcode(), self.operands.len())
        }
        // Ensure the destination type is valid.
        ensure!(is_valid_destination_type(self.destination_type), "Invalid destination type in 'commit' instruction");

        // Retrieve the input and randomizer.
        let input = registers.load(stack, &self.operands[0])?;
        let randomizer = registers.load(stack, &self.operands[1])?;
        // Retrieve the randomizer.
        let randomizer = match randomizer {
            Value::Plaintext(Plaintext::Literal(Literal::Scalar(randomizer), ..)) => randomizer,
            _ => bail!("Invalid randomizer type for the commit evaluation, expected a scalar"),
        };

        // Commit the input.
        let output = match VARIANT {
            0 => Literal::Group(N::commit_to_group_bhp256(&input.to_bits_le(), &randomizer)?),
            1 => Literal::Group(N::commit_to_group_bhp512(&input.to_bits_le(), &randomizer)?),
            2 => Literal::Group(N::commit_to_group_bhp768(&input.to_bits_le(), &randomizer)?),
            3 => Literal::Group(N::commit_to_group_bhp1024(&input.to_bits_le(), &randomizer)?),
            4 => Literal::Group(N::commit_to_group_ped64(&input.to_bits_le(), &randomizer)?),
            5 => Literal::Group(N::commit_to_group_ped128(&input.to_bits_le(), &randomizer)?),
            6.. => bail!("Invalid 'commit' variant: {VARIANT}"),
        };
        // Cast the output to the destination type.
        let output = output.cast_lossy(self.destination_type)?;
        // Store the output.
        registers.store(stack, &self.destination, Value::Plaintext(Plaintext::from(output)))
    }

    /// Executes the instruction.
    #[inline]
    pub fn execute<A: circuit::Aleo<Network = N>>(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        registers: &mut (impl RegistersLoadCircuit<N, A> + RegistersStoreCircuit<N, A>),
    ) -> Result<()> {
        use circuit::traits::ToBits;

        // Ensure the number of operands is correct.
        if self.operands.len() != 2 {
            bail!("Instruction '{}' expects 2 operands, found {} operands", Self::opcode(), self.operands.len())
        }
        // Ensure the destination type is valid.
        ensure!(is_valid_destination_type(self.destination_type), "Invalid destination type in 'commit' instruction");

        // Retrieve the input and randomizer.
        let input = registers.load_circuit(stack, &self.operands[0])?;
        let randomizer = registers.load_circuit(stack, &self.operands[1])?;
        // Retrieve the randomizer.
        let randomizer = match randomizer {
            circuit::Value::Plaintext(circuit::Plaintext::Literal(circuit::Literal::Scalar(randomizer), ..)) => {
                randomizer
            }
            _ => bail!("Invalid randomizer type for the commit execution, expected a scalar"),
        };

        // Commits the input.
        let output = match VARIANT {
            0 => circuit::Literal::Group(A::commit_to_group_bhp256(&input.to_bits_le(), &randomizer)),
            1 => circuit::Literal::Group(A::commit_to_group_bhp512(&input.to_bits_le(), &randomizer)),
            2 => circuit::Literal::Group(A::commit_to_group_bhp768(&input.to_bits_le(), &randomizer)),
            3 => circuit::Literal::Group(A::commit_to_group_bhp1024(&input.to_bits_le(), &randomizer)),
            4 => circuit::Literal::Group(A::commit_to_group_ped64(&input.to_bits_le(), &randomizer)),
            5 => circuit::Literal::Group(A::commit_to_group_ped128(&input.to_bits_le(), &randomizer)),
            6.. => bail!("Invalid 'commit' variant: {VARIANT}"),
        };
        let output = output.cast_lossy(self.destination_type)?;
        // Convert the output to a stack value.
        let output = circuit::Value::Plaintext(circuit::Plaintext::Literal(output, Default::default()));
        // Store the output.
        registers.store_circuit(stack, &self.destination, output)
    }

    /// Finalizes the instruction.
    #[inline]
    pub fn finalize(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        registers: &mut (impl RegistersLoad<N> + RegistersStore<N>),
    ) -> Result<()> {
        self.evaluate(stack, registers)
    }

    /// Returns the output type from the given program and input types.
    #[inline]
    pub fn output_types(
        &self,
        _stack: &impl StackProgram<N>,
        input_types: &[RegisterType<N>],
    ) -> Result<Vec<RegisterType<N>>> {
        // Ensure the number of input types is correct.
        if input_types.len() != 2 {
            bail!("Instruction '{}' expects 2 inputs, found {} inputs", Self::opcode(), input_types.len())
        }
        // Ensure the number of operands is correct.
        if self.operands.len() != 2 {
            bail!("Instruction '{}' expects 2 operands, found {} operands", Self::opcode(), self.operands.len())
        }
        // Ensure the destination type is valid.
        ensure!(is_valid_destination_type(self.destination_type), "Invalid destination type in 'commit' instruction");

        // TODO (howardwu): If the operation is Pedersen, check that it is within the number of bits.

        match VARIANT {
            0..=5 => Ok(vec![RegisterType::Plaintext(PlaintextType::Literal(self.destination_type))]),
            6.. => bail!("Invalid 'commit' variant: {VARIANT}"),
        }
    }
}

impl<N: Network, const VARIANT: u8> Parser for CommitInstruction<N, VARIANT> {
    /// Parses a string into an operation.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        // Parse the opcode from the string.
        let (string, _) = tag(*Self::opcode())(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the first operand from the string.
        let (string, first) = Operand::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the second operand from the string.
        let (string, second) = Operand::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the "into" from the string.
        let (string, _) = tag("into")(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the destination register from the string.
        let (string, destination) = Register::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the "as" from the string.
        let (string, _) = tag("as")(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the destination register type from the string.
        let (string, destination_type) = LiteralType::parse(string)?;
        // Ensure the destination type is allowed.
        match destination_type {
            LiteralType::Address | LiteralType::Field | LiteralType::Group => {
                Ok((string, Self { operands: vec![first, second], destination, destination_type }))
            }
            _ => map_res(fail, |_: ParserResult<Self>| {
                Err(error(format!("Failed to parse 'commit': '{destination_type}' is invalid")))
            })(string),
        }
    }
}

impl<N: Network, const VARIANT: u8> FromStr for CommitInstruction<N, VARIANT> {
    type Err = Error;

    /// Parses a string into an operation.
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

impl<N: Network, const VARIANT: u8> Debug for CommitInstruction<N, VARIANT> {
    /// Prints the operation as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network, const VARIANT: u8> Display for CommitInstruction<N, VARIANT> {
    /// Prints the operation to a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // Ensure the number of operands is 2.
        if self.operands.len() != 2 {
            return Err(fmt::Error);
        }
        // Print the operation.
        write!(f, "{} ", Self::opcode())?;
        self.operands.iter().try_for_each(|operand| write!(f, "{operand} "))?;
        write!(f, "into {} as {}", self.destination, self.destination_type)
    }
}

impl<N: Network, const VARIANT: u8> FromBytes for CommitInstruction<N, VARIANT> {
    /// Reads the operation from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Initialize the vector for the operands.
        let mut operands = Vec::with_capacity(2);
        // Read the operands.
        for _ in 0..2 {
            operands.push(Operand::read_le(&mut reader)?);
        }
        // Read the destination register.
        let destination = Register::read_le(&mut reader)?;
        // Read the destination register type.
        let destination_type = LiteralType::read_le(&mut reader)?;

        // Return the operation.
        Self::new(operands, destination, destination_type).map_err(error)
    }
}

impl<N: Network, const VARIANT: u8> ToBytes for CommitInstruction<N, VARIANT> {
    /// Writes the operation to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Ensure the number of operands is 2.
        if self.operands.len() != 2 {
            return Err(error(format!("The number of operands must be 2, found {}", self.operands.len())));
        }
        // Write the operands.
        self.operands.iter().try_for_each(|operand| operand.write_le(&mut writer))?;
        // Write the destination register.
        self.destination.write_le(&mut writer)?;
        // Write the destination register type.
        self.destination_type.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    /// **Attention**: When changing this, also update in `tests/instruction/commit.rs`.
    fn valid_destination_types() -> &'static [LiteralType] {
        &[LiteralType::Address, LiteralType::Field, LiteralType::Group]
    }

    #[test]
    fn test_parse() {
        for destination_type in valid_destination_types() {
            let instruction = format!("commit.bhp512 r0 r1 into r2 as {destination_type}");
            let (string, commit) = CommitBHP512::<CurrentNetwork>::parse(&instruction).unwrap();
            assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");
            assert_eq!(commit.operands.len(), 2, "The number of operands is incorrect");
            assert_eq!(commit.operands[0], Operand::Register(Register::Locator(0)), "The first operand is incorrect");
            assert_eq!(commit.operands[1], Operand::Register(Register::Locator(1)), "The second operand is incorrect");
            assert_eq!(commit.destination, Register::Locator(2), "The destination register is incorrect");
            assert_eq!(commit.destination_type, *destination_type, "The destination type is incorrect");
        }
    }
}
