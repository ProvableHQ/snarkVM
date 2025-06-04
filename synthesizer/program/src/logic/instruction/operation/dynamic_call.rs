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

use crate::{
    Opcode,
    Operand,
    traits::{RegistersLoad, RegistersLoadCircuit, StackMatches, StackProgram},
};
use console::{
    network::prelude::*,
    program::{Register, RegisterType},
};

/// Dynamically calls the operands into the declared type.
/// The first operand must resolve to a field element representing the program name.
/// The second operand must resolve to a field element representing the program network.
/// The third operand must resolve to a field element representing the function name.
/// The remaining operands are the arguments to the call.
/// The destination registers along with their expected types are specified after the `into` keyword.
/// i.e. `dcall r0 r1 r2 r0.owner 0u64 r1.amount into r1 r2 (as u64 dynamic.future);`
// TODO (@d0cd) Should we allow operands to be identifiers so that we can allow function names to be specified directly.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct DynamicCall<N: Network> {
    /// The program ID name.
    program_id_name: Operand<N>,
    /// The program ID network.
    program_id_network: Operand<N>,
    /// The function name.
    function_name: Operand<N>,
    /// The operands.
    operands: Vec<Operand<N>>,
    /// The destination registers.
    destinations: Vec<Register<N>>,
    /// The destination types.
    destination_types: Vec<RegisterType<N>>,
}

impl<N: Network> DynamicCall<N> {
    /// Returns the opcode.
    #[inline]
    pub const fn opcode() -> Opcode {
        Opcode::DynamicCall
    }

    /// Returns the program ID name.
    #[inline]
    pub const fn program_id_name(&self) -> &Operand<N> {
        &self.program_id_name
    }

    /// Returns the program ID network.
    #[inline]
    pub const fn program_id_network(&self) -> &Operand<N> {
        &self.program_id_network
    }

    /// Returns the function name.
    #[inline]
    pub const fn function_name(&self) -> &Operand<N> {
        &self.function_name
    }

    #[inline]
    pub fn operands(&self) -> &[Operand<N>] {
        &self.operands
    }

    /// Returns the destination registers.
    #[inline]
    pub fn destinations(&self) -> Vec<Register<N>> {
        self.destinations.clone()
    }

    /// Returns the destination types.
    #[inline]
    pub fn destination_types(&self) -> &Vec<RegisterType<N>> {
        &self.destination_types
    }
}

impl<N: Network> DynamicCall<N> {
    /// Returns `true` if the instruction is a function call.
    #[inline]
    pub fn is_function_call(&self, _stack: &impl StackProgram<N>) -> Result<bool> {
        Ok(true)
    }

    /// Evaluates the instruction.
    pub fn evaluate(&self, _stack: &impl StackProgram<N>, _registers: &mut impl RegistersLoad<N>) -> Result<()> {
        bail!("Forbidden operation: Evaluate cannot invoke a 'dcall' directly. Use 'dcall' in 'Stack' instead.")
    }

    /// Executes the instruction.
    pub fn execute<A: circuit::Aleo<Network = N>>(
        &self,
        _stack: &impl StackProgram<N>,
        _registers: &mut impl RegistersLoadCircuit<N, A>,
    ) -> Result<()> {
        bail!("Forbidden operation: Execute cannot invoke a 'dcall' directly. Use 'dcall' in 'Stack' instead.")
    }

    /// Finalizes the instruction.
    #[inline]
    pub fn finalize(
        &self,
        _stack: &(impl StackMatches<N> + StackProgram<N>),
        _registers: &mut impl RegistersLoad<N>,
    ) -> Result<()> {
        bail!("Forbidden operation: Finalize cannot invoke a 'dcall' directly. Use 'dcall' in 'Stack' instead.")
    }

    /// Returns the output type from the given program and input types.
    #[inline]
    pub fn output_types(
        &self,
        _stack: &impl StackProgram<N>,
        _input_types: &[RegisterType<N>],
    ) -> Result<Vec<RegisterType<N>>> {
        Ok(self.destination_types().clone())
    }
}

impl<N: Network> Parser for DynamicCall<N> {
    /// Parses a string into an operation.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        /// Parses an operand from the string.
        fn parse_operand<N: Network>(string: &str) -> ParserResult<Operand<N>> {
            // Parse the whitespace from the string.
            let (string, _) = Sanitizer::parse_whitespaces(string)?;
            // Parse the operand from the string.
            Operand::parse(string)
        }

        /// Parses a destination register from the string.
        fn parse_destination<N: Network>(string: &str) -> ParserResult<Register<N>> {
            // Parse the whitespace from the string.
            let (string, _) = Sanitizer::parse_whitespaces(string)?;
            // Parse the destination from the string.
            Register::parse(string)
        }

        /// Parses a destination type from the string.
        fn parse_destination_type<N: Network>(string: &str) -> ParserResult<RegisterType<N>> {
            // Parse the whitespace from the string.
            let (string, _) = Sanitizer::parse_whitespaces(string)?;
            // Parse the destination type from the string.
            RegisterType::parse(string)
        }

        // Parse the opcode from the string.
        let (string, _) = tag(*Self::opcode())(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the program ID name of the call from the string.
        let (string, program_id_name) = Operand::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the program ID network of the call from the string.
        let (string, program_id_network) = Operand::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the function name of the call from the string .
        let (string, function_name) = Operand::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the operands from the string.
        let (string, operands) = map_res(many0(complete(parse_operand)), |operands: Vec<Operand<N>>| {
            // Ensure the number of operands is within the bounds.
            match operands.len() <= N::MAX_OPERANDS {
                true => Ok(operands),
                false => Err(error("Failed to parse 'dcall' opcode: too many operands")),
            }
        })(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;

        // Optionally parse the "into" from the string.
        let (string, destinations, destination_types) = match opt(tag("into"))(string)? {
            // If the "into" was not parsed, return the string and an empty vector of destinations.
            (string, None) => (string, vec![], vec![]),
            // If the "into" was parsed, parse the destinations from the string.
            (string, Some(_)) => {
                // Parse the whitespace from the string.
                let (string, _) = Sanitizer::parse_whitespaces(string)?;
                // Parse the destinations from the string.
                let (string, destinations) =
                    map_res(many1(complete(parse_destination)), |destinations: Vec<Register<N>>| {
                        // Ensure the number of destinations is within the bounds.
                        match destinations.len() <= N::MAX_OPERANDS {
                            true => Ok(destinations),
                            false => Err(error("Failed to parse 'dcall' opcode: too many destinations")),
                        }
                    })(string)?;
                // Parse the destination types from the string.
                let (string, destination_types) =
                    map_res(many1(parse_destination_type), |destination_types: Vec<RegisterType<N>>| {
                        // Ensure the number of destination types is within the bounds.
                        match destination_types.len() <= N::MAX_OPERANDS {
                            true => Ok(destination_types),
                            false => Err(error("Failed to parse 'dcall' opcode: too many destination types")),
                        }
                    })(string)?;
                // Check that the number of destination registers and destination types match.
                if destinations.len() != destination_types.len() {
                    return map_res(take(0usize), |_| {
                        Err(error("The number of destination registers and destination types do not match".to_string()))
                    })(string);
                };
                // Return the string and the destinations.
                (string, destinations, destination_types)
            }
        };

        Ok((string, Self {
            program_id_name,
            program_id_network,
            function_name,
            operands,
            destinations,
            destination_types,
        }))
    }
}

impl<N: Network> FromStr for DynamicCall<N> {
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

impl<N: Network> Debug for DynamicCall<N> {
    /// Prints the operation as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for DynamicCall<N> {
    /// Prints the operation to a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // Ensure the number of operands is within the bounds.
        if self.operands.len() > N::MAX_OPERANDS {
            return Err(fmt::Error);
        }
        // Ensure the number of destinations is within the bounds.
        if self.destinations.len() > N::MAX_OPERANDS {
            return Err(fmt::Error);
        }
        // Ensure the number of destination types is within the bounds.
        if self.destination_types.len() > N::MAX_OPERANDS {
            return Err(fmt::Error);
        }
        // Ensure the number of destination registers and destination types match.
        if self.destinations.len() != self.destination_types.len() {
            return Err(fmt::Error);
        }
        // Print the operation.
        write!(f, "{} {} {} {}", Self::opcode(), self.program_id_name, self.program_id_network, self.function_name)?;
        self.operands.iter().try_for_each(|operand| write!(f, " {operand}"))?;
        if !self.destinations.is_empty() {
            write!(f, " into")?;
            self.destinations.iter().try_for_each(|destination| write!(f, " {destination}"))?;
            write!(f, " (as {})", self.destination_types.iter().join(" "))?;
        }
        Ok(())
    }
}

impl<N: Network> FromBytes for DynamicCall<N> {
    /// Reads the operation from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the program ID name.
        let program_id_name = Operand::read_le(&mut reader)?;
        // Read the program ID network.
        let program_id_network = Operand::read_le(&mut reader)?;
        // Read the function name.
        let function_name = Operand::read_le(&mut reader)?;
        // Read the number of operands.
        let num_operands = u8::read_le(&mut reader)? as usize;
        // Ensure the number of operands is within the bounds.
        if num_operands > N::MAX_OPERANDS {
            return Err(error(format!("The number of operands must be <= {}", N::MAX_OPERANDS)));
        }

        // Initialize the vector for the operands.
        let mut operands = Vec::with_capacity(num_operands);
        // Read the operands.
        for _ in 0..num_operands {
            operands.push(Operand::read_le(&mut reader)?);
        }

        // Read the number of destination registers.
        let num_destinations = u8::read_le(&mut reader)? as usize;
        // Ensure the number of destinations is within the bounds.
        if num_destinations > N::MAX_OPERANDS {
            return Err(error(format!("The number of destinations must be <= {}", N::MAX_OPERANDS)));
        }

        // Initialize the vector for the destinations.
        let mut destinations = Vec::with_capacity(num_destinations);
        // Read the destination registers.
        for _ in 0..num_destinations {
            destinations.push(Register::read_le(&mut reader)?);
        }

        // Initialize the vector for the destination types.
        let mut destination_types = Vec::with_capacity(num_destinations);
        for _ in 0..num_destinations {
            destination_types.push(RegisterType::read_le(&mut reader)?);
        }

        // Return the operation.
        Ok(Self { program_id_name, program_id_network, function_name, operands, destinations, destination_types })
    }
}

impl<N: Network> ToBytes for DynamicCall<N> {
    /// Writes the operation to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Ensure the number of operands is within the bounds.
        if self.operands.len() > N::MAX_OPERANDS {
            return Err(error(format!("The number of operands must be <= {}", N::MAX_OPERANDS)));
        }
        // Ensure the number of destinations is within the bounds.
        if self.destinations.len() > N::MAX_OPERANDS {
            return Err(error(format!("The number of destinations must be <= {}", N::MAX_OPERANDS)));
        }
        // Ensure the number of destinations and destination types match.
        if self.destinations.len() != self.destination_types.len() {
            return Err(error("The number of destination registers and destination types do not match".to_string()));
        }

        // Write the program ID name.
        self.program_id_name.write_le(&mut writer)?;
        // Write the program ID network.
        self.program_id_network.write_le(&mut writer)?;
        // Write the function name.
        self.function_name.write_le(&mut writer)?;
        // Write the number of operands.
        u8::try_from(self.operands.len()).map_err(|e| error(e.to_string()))?.write_le(&mut writer)?;
        // Write the operands.
        self.operands.iter().try_for_each(|operand| operand.write_le(&mut writer))?;
        // Write the number of destination register.
        u8::try_from(self.destinations.len()).map_err(|e| error(e.to_string()))?.write_le(&mut writer)?;
        // Write the destination registers.
        self.destinations.iter().try_for_each(|destination| destination.write_le(&mut writer))?;
        // Write the destination types.
        self.destination_types.iter().try_for_each(|destination| destination.write_le(&mut writer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::{
        network::MainnetV0,
        program::{Access, Identifier, LiteralType, PlaintextType},
    };

    type CurrentNetwork = MainnetV0;

    const TEST_CASES: &[&str] = &[
        "dcall r0 r1 r2",
        "dcall r0 r1 r2 r0",
        "dcall r0 r1 r2 r0.owner",
        "dcall r0 r1 r2 r0 r1",
        "dcall r0 r1 r2 into r0",
        "dcall r0 r1 r2 into r0 r1",
        "dcall r0 r1 r2 into r0 r1 r2",
        "dcall r0 r1 r2 r0 into r1",
        "dcall r0 r1 r2 r0 r1 into r2",
        "dcall r0 r1 r2 r0 r1 into r2 r3",
        "dcall r0 r1 r2 r0 r1 r2 into r3 r4",
        "dcall r0 r1 r2 r0 r1 r2 into r3 r4 r5",
    ];

    fn check_parser(
        string: &str,
        expected_program_id_name: Operand<CurrentNetwork>,
        expected_program_id_network: Operand<CurrentNetwork>,
        expected_function_name: Operand<CurrentNetwork>,
        expected_operands: Vec<Operand<CurrentNetwork>>,
        expected_destinations: Vec<Register<CurrentNetwork>>,
        exepcted_destination_types: Vec<RegisterType<CurrentNetwork>>,
    ) {
        // Check that the parser works.
        let (string, call) = DynamicCall::<CurrentNetwork>::parse(string).unwrap();

        // Check that the entire string was consumed.
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");

        // Check that the program operand is correct.
        assert_eq!(call.program_id_name, expected_program_id_name);
        assert_eq!(call.program_id_network, expected_program_id_network);

        // Check that the function operand is correct.
        assert_eq!(call.function_name, expected_function_name);

        // Check that the operands are correct.
        assert_eq!(call.operands.len(), expected_operands.len(), "The number of operands is incorrect");
        for (i, (given, expected)) in call.operands.iter().zip(expected_operands.iter()).enumerate() {
            assert_eq!(given, expected, "The {i}-th operand is incorrect");
        }

        // Check that the number of destinations and destination types match.
        assert_eq!(
            call.destinations.len(),
            call.destination_types.len(),
            "The number of destinations and destination types do not match"
        );

        // Check that the destinations are correct.
        assert_eq!(call.destinations.len(), expected_destinations.len(), "The number of destinations is incorrect");
        for (i, (given, expected)) in call.destinations.iter().zip(expected_destinations.iter()).enumerate() {
            assert_eq!(given, expected, "The {i}-th destination is incorrect");
        }

        // Check that the destination types are correct.
        assert_eq!(
            call.destination_types.len(),
            exepcted_destination_types.len(),
            "The number of destination types is incorrect"
        );
        for (i, (given, expected)) in call.destination_types.iter().zip(exepcted_destination_types.iter()).enumerate() {
            assert_eq!(given, expected, "The {i}-th destination type is incorrect");
        }
    }

    #[test]
    fn test_parse() {
        check_parser(
            "dcall r4 aleo r5 r0.owner r0.token_amount into r1 r2 r3 (as u64 u8 dynamic.future)",
            Operand::Register(Register::Locator(4)),
            Operand::Identifier(Identifier::from_str("aleo").unwrap()),
            Operand::Register(Register::Locator(5)),
            vec![
                Operand::Register(Register::Access(0, vec![Access::from(Identifier::from_str("owner").unwrap())])),
                Operand::Register(Register::Access(0, vec![Access::from(
                    Identifier::from_str("token_amount").unwrap(),
                )])),
            ],
            vec![Register::Locator(1), Register::Locator(2), Register::Locator(3)],
            vec![
                RegisterType::Plaintext(PlaintextType::Literal(LiteralType::U64)),
                RegisterType::Plaintext(PlaintextType::Literal(LiteralType::U8)),
                RegisterType::DynamicFuture,
            ],
        );

        // // TODO (@d0cd) Support for this test case.
        // check_parser(
        //     "dcall credits.aleo transfer_public aleo1wfyyj2uvwuqw0c0dqa5x70wrawnlkkvuepn4y08xyaqfqqwweqys39jayw 100u64 into r0 (as dynamic.future)",
        //     Operand::ProgramID(ProgramID::<CurrentNetwork>::from_str("credits.aleo").unwrap()),
        //     Operand::Identifier(Identifier::from_str("transfer_public").unwrap()),
        //     vec![
        //         Operand::Literal(Literal::Address(
        //             Address::from_str("aleo1wfyyj2uvwuqw0c0dqa5x70wrawnlkkvuepn4y08xyaqfqqwweqys39jayw").unwrap(),
        //         )),
        //         Operand::Literal(Literal::U64(U64::from_str("100u64").unwrap())),
        //     ],
        //     vec![Register::Locator(0)],
        //     vec![RegisterType::DynamicFuture],
        // );

        check_parser(
            "dcall r0 r1 r2",
            Operand::Register(Register::Locator(0)),
            Operand::Register(Register::Locator(1)),
            Operand::Register(Register::Locator(2)),
            vec![],
            vec![],
            vec![],
        )
    }

    #[test]
    fn test_display() {
        for expected in TEST_CASES {
            assert_eq!(DynamicCall::<CurrentNetwork>::from_str(expected).unwrap().to_string(), *expected);
        }
    }

    #[test]
    fn test_bytes() {
        for case in TEST_CASES {
            let expected = DynamicCall::<CurrentNetwork>::from_str(case).unwrap();

            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, DynamicCall::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
