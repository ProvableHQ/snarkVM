// Copyright (c) 2019-2025 Provable Inc.
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

use circuit::{Eject, Inject};
use console::{
    network::prelude::*,
    program::{Literal, LiteralType, Plaintext, PlaintextType, Register, RegisterType, Value},
    types::Boolean,
};

use crate::{Opcode, Operand, RegistersCircuit, RegistersTrait, StackTrait};

/// Computes an equality operation on two operands, and stores the outcome in `destination`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct In<N: Network> {
    /// The operands.
    operands: [Operand<N>; 2],
    /// The destination register.
    destination: Register<N>,
}

impl<N: Network> In<N> {
    /// Initializes a new `in` instruction.
    #[inline]
    pub fn new(operands: [Operand<N>; 2], destination: Register<N>) -> Result<Self> {
        // Return the instruction.
        Ok(Self { operands, destination })
    }

    /// Returns the opcode.s
    #[inline]
    pub const fn opcode() -> Opcode {
        Opcode::In
    }

    /// Returns the operands in the operation.
    #[inline]
    pub fn operands(&self) -> &[Operand<N>] {
        // Return the operands.
        &self.operands
    }

    /// Returns the destination register.
    #[inline]
    pub fn destinations(&self) -> Vec<Register<N>> {
        vec![self.destination.clone()]
    }
}

impl<N: Network> In<N> {
    /// Evaluates the instruction.
    pub fn evaluate(&self, stack: &impl StackTrait<N>, registers: &mut impl RegistersTrait<N>) -> Result<()> {
        // Retrieve the inputs.
        let input_a = registers.load(stack, &self.operands[0])?;
        let input_b = registers.load(stack, &self.operands[1])?;

        // Make sure the second operand is an array.
        let Value::Plaintext(Plaintext::Array(array, _)) = &input_b else {
            bail!("Instruction '{}' requires second operand to be an array but found {}", Self::opcode(), input_b)
        };

        // Make sure the first operand is not an illegal type for this case.
        let Value::Plaintext(val) = &input_a else { bail!("Array cannot have records or futures as its elements.") };

        // Check if the array contains the value.
        let output = Literal::Boolean(Boolean::new(array.contains(val)));

        // Store the output.
        registers.store(stack, &self.destination, Value::Plaintext(Plaintext::from(output)))
    }

    /// Executes the instruction.
    pub fn execute<A: circuit::Aleo<Network = N>>(
        &self,
        stack: &impl StackTrait<N>,
        registers: &mut impl RegistersCircuit<N, A>,
    ) -> Result<()> {
        // Retrieve the inputs.
        let input_a = registers.load_circuit(stack, &self.operands[0])?;
        let input_b = registers.load_circuit(stack, &self.operands[1])?;

        // Make sure the second operand is an array.
        let circuit::Value::Plaintext(circuit::Plaintext::Array(array, _)) = &input_b else {
            bail!(
                "Instruction '{}' requires second operand to be an array but found {}",
                Self::opcode(),
                input_b.eject_value()
            )
        };

        // Make sure the first operand is not an illegal type for this case.
        let circuit::Value::Plaintext(val) = &input_a else {
            bail!("Array cannot have records or futures as its elements.")
        };

        // Check if the array contains the value.
        let mut output = circuit::Boolean::constant(false);
        for element in array {
            output = val.is_equal(element).bitor(output)
        }

        // Store the output.
        registers.store_literal_circuit(stack, &self.destination, circuit::Literal::from(output))
    }

    /// Finalizes the instruction.
    #[inline]
    pub fn finalize(&self, stack: &impl StackTrait<N>, registers: &mut impl RegistersTrait<N>) -> Result<()> {
        self.evaluate(stack, registers)
    }

    /// Returns the output type from the given program and input types.
    pub fn output_types(
        &self,
        _stack: &impl StackTrait<N>,
        input_types: &[RegisterType<N>],
    ) -> Result<Vec<RegisterType<N>>> {
        // Ensure the number of input types is correct.
        if input_types.len() != 2 {
            bail!("Instruction '{}' expects 2 inputs, found {} inputs", Self::opcode(), input_types.len())
        }

        let RegisterType::Plaintext(PlaintextType::Array(arr_type)) = &input_types[1] else {
            bail!("Instruction {} expects the second input to be an array got {}", Self::opcode(), input_types[1])
        };

        let element_type = RegisterType::Plaintext(arr_type.next_element_type().clone());

        // Ensure the operand are of the same type.
        if input_types[0] != element_type {
            bail!(
                "Instruction '{}' expects first input and element type of array to be of the same type. Found inputs of type '{}' and '{}'",
                Self::opcode(),
                input_types[0],
                element_type
            )
        }

        Ok(vec![RegisterType::Plaintext(PlaintextType::Literal(LiteralType::Boolean))])
    }
}

impl<N: Network> Parser for In<N> {
    /// Parses a string into an operation.
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

        Ok((string, Self { operands: [first, second], destination }))
    }
}

impl<N: Network> FromStr for In<N> {
    type Err = Error;

    /// Parses a string into an operation.
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

impl<N: Network> Debug for In<N> {
    /// Prints the operation as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for In<N> {
    /// Prints the operation to a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // Print the operation.
        write!(f, "{} ", Self::opcode())?;
        self.operands.iter().try_for_each(|operand| write!(f, "{operand} "))?;
        write!(f, "into {}", self.destination)
    }
}

impl<N: Network> FromBytes for In<N> {
    /// Reads the operation from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Initialize the array and read the operands.
        let operands = [Operand::read_le(&mut reader)?, Operand::read_le(&mut reader)?];

        // Read the destination register.
        let destination = Register::read_le(&mut reader)?;

        // Return the operation.
        Ok(Self { operands, destination })
    }
}

impl<N: Network> ToBytes for In<N> {
    /// Writes the operation to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the operands.
        self.operands.iter().try_for_each(|operand| operand.write_le(&mut writer))?;
        // Write the destination register.
        self.destination.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_parse() {
        let (string, in_) = In::<CurrentNetwork>::parse("in r0 r1 into r2").unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");
        assert_eq!(in_.operands.len(), 2, "The number of operands is incorrect");
        assert_eq!(in_.operands[0], Operand::Register(Register::Locator(0)), "The first operand is incorrect");
        assert_eq!(in_.operands[1], Operand::Register(Register::Locator(1)), "The second operand is incorrect");
        assert_eq!(in_.destination, Register::Locator(2), "The destination register is incorrect");
    }
}
