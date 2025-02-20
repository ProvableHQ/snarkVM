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
use std::fmt::Pointer;

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> Parser
    for ProgramCore<N, Instruction, Command>
{
    /// Parses a string into a program.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        alt((map(ProgramCoreV1::parse, Self::ProgramV1), map(ProgramCoreV2::parse, Self::ProgramV2)))(string)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Program;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_program_parse() -> Result<()> {
        // Initialize a new program.
        let (string, program) = Program::<CurrentNetwork>::parse(
            r"
program to_parse.aleo;

struct message:
    first as field;
    second as field;

_init:
    add 1u8 1u8 into r0;

function compute:
    input r0 as message.private;
    add r0.first r0.second into r1;
    output r1 as field.private;",
        )
        .unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");

        // Ensure the program contains the struct.
        assert!(program.contains_struct(&Identifier::from_str("message")?));
        // Ensure the program contains the function.
        assert!(program.contains_function(&Identifier::from_str("compute")?));

        Ok(())
    }

    #[test]
    fn test_program_parse_function_zero_inputs() -> Result<()> {
        // Initialize a new program.
        let (string, program) = Program::<CurrentNetwork>::parse(
            r"
program to_parse.aleo;

function compute:
    add 1u32 2u32 into r0;
    output r0 as u32.private;",
        )
        .unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");

        // Ensure the program contains the function.
        assert!(program.contains_function(&Identifier::from_str("compute")?));

        Ok(())
    }

    #[test]
    fn test_program_display() -> Result<()> {
        let expected = r"program to_parse.aleo;

struct message:
    first as field;
    second as field;

function compute:
    input r0 as message.private;
    add r0.first r0.second into r1;
    output r1 as field.private;

_init:
    assert.eq 0u8 1u8;
";
        // Parse a new program.
        let program = Program::<CurrentNetwork>::from_str(expected)?;
        // Ensure the program string matches.
        assert_eq!(expected, format!("{program}"));

        Ok(())
    }

    #[test]
    fn test_program_size() {
        // Define variable name for easy experimentation with program sizes.
        let var_name = "a";

        // Helper function to generate imports.
        let gen_import_string = |n: usize| -> String {
            let mut s = String::new();
            for i in 0..n {
                s.push_str(&format!("import foo{i}.aleo;\n"));
            }
            s
        };

        // Helper function to generate large structs.
        let gen_struct_string = |n: usize| -> String {
            let mut s = String::with_capacity(CurrentNetwork::MAX_PROGRAM_SIZE);
            for i in 0..n {
                s.push_str(&format!("struct m{}:\n", i));
                for j in 0..10 {
                    s.push_str(&format!("    {}{} as u128;\n", var_name, j));
                }
            }
            s
        };

        // Helper function to generate large records.
        let gen_record_string = |n: usize| -> String {
            let mut s = String::with_capacity(CurrentNetwork::MAX_PROGRAM_SIZE);
            for i in 0..n {
                s.push_str(&format!("record r{}:\n    owner as address.private;\n", i));
                for j in 0..10 {
                    s.push_str(&format!("    {}{} as u128.private;\n", var_name, j));
                }
            }
            s
        };

        // Helper function to generate large mappings.
        let gen_mapping_string = |n: usize| -> String {
            let mut s = String::with_capacity(CurrentNetwork::MAX_PROGRAM_SIZE);
            for i in 0..n {
                s.push_str(&format!(
                    "mapping {}{}:\n    key as field.public;\n    value as field.public;\n",
                    var_name, i
                ));
            }
            s
        };

        // Helper function to generate large closures.
        let gen_closure_string = |n: usize| -> String {
            let mut s = String::with_capacity(CurrentNetwork::MAX_PROGRAM_SIZE);
            for i in 0..n {
                s.push_str(&format!("closure c{}:\n    input r0 as u128;\n", i));
                for j in 0..10 {
                    s.push_str(&format!("    add r0 r0 into r{};\n", j));
                }
                s.push_str(&format!("    output r{} as u128;\n", 4000));
            }
            s
        };

        // Helper function to generate large functions.
        let gen_function_string = |n: usize| -> String {
            let mut s = String::with_capacity(CurrentNetwork::MAX_PROGRAM_SIZE);
            for i in 0..n {
                s.push_str(&format!("function f{}:\n    add 1u128 1u128 into r0;\n", i));
                for j in 0..10 {
                    s.push_str(&format!("    add r0 r0 into r{j};\n"));
                }
            }
            s
        };

        // Helper function to generate constructors.
        let gen_constructor_string = |n: usize| -> String {
            let mut s = String::with_capacity(CurrentNetwork::MAX_PROGRAM_SIZE);
            for i in 0..n {
                s.push_str("_init:\n");
                s.push_str(&format!("    add {i}u8 {i}u8 into r0;\n"));
            }
            s
        };

        // Helper function to generate and parse a program.
        let test_parse = |imports: &str, body: &str, should_succeed: bool| {
            let program = format!("{imports}\nprogram to_parse.aleo;\n\n{body}");
            let result = Program::<CurrentNetwork>::from_str(&program);
            if result.is_ok() != should_succeed {
                println!("Program failed to parse: {program}");
            }
            assert_eq!(result.is_ok(), should_succeed);
        };

        // A program with MAX_IMPORTS should succeed.
        test_parse(&gen_import_string(CurrentNetwork::MAX_IMPORTS), &gen_struct_string(1), true);
        // A program with more than MAX_IMPORTS should fail.
        test_parse(&gen_import_string(CurrentNetwork::MAX_IMPORTS + 1), &gen_struct_string(1), false);
        // A program with MAX_STRUCTS should succeed.
        test_parse("", &gen_struct_string(CurrentNetwork::MAX_STRUCTS), true);
        // A program with more than MAX_STRUCTS should fail.
        test_parse("", &gen_struct_string(CurrentNetwork::MAX_STRUCTS + 1), false);
        // A program with MAX_RECORDS should succeed.
        test_parse("", &gen_record_string(CurrentNetwork::MAX_RECORDS), true);
        // A program with more than MAX_RECORDS should fail.
        test_parse("", &gen_record_string(CurrentNetwork::MAX_RECORDS + 1), false);
        // A program with MAX_MAPPINGS should succeed.
        test_parse("", &gen_mapping_string(CurrentNetwork::MAX_MAPPINGS), true);
        // A program with more than MAX_MAPPINGS should fail.
        test_parse("", &gen_mapping_string(CurrentNetwork::MAX_MAPPINGS + 1), false);
        // A program with MAX_CLOSURES should succeed.
        test_parse("", &gen_closure_string(CurrentNetwork::MAX_CLOSURES), true);
        // A program with more than MAX_CLOSURES should fail.
        test_parse("", &gen_closure_string(CurrentNetwork::MAX_CLOSURES + 1), false);
        // A program with MAX_FUNCTIONS should succeed.
        test_parse("", &gen_function_string(CurrentNetwork::MAX_FUNCTIONS), true);
        // A program with more than MAX_FUNCTIONS should fail.
        test_parse("", &gen_function_string(CurrentNetwork::MAX_FUNCTIONS + 1), false);
        // A program with no constructor should succeed.
        test_parse("", &format!("function dummy:\n\n{}", &gen_constructor_string(0)), true);
        // A program with a constructor should succeed.
        test_parse("", &format!("function dummy:\n\n{}", &gen_constructor_string(1)), true);
        // A program with two constructors should fail.
        test_parse("", &format!("function dummy:\n\n{}", &gen_constructor_string(2)), false);

        // Initialize a program which is too big.
        let program_too_big = format!(
            "{} {} {} {} {}",
            gen_struct_string(CurrentNetwork::MAX_STRUCTS),
            gen_record_string(CurrentNetwork::MAX_RECORDS),
            gen_mapping_string(CurrentNetwork::MAX_MAPPINGS),
            gen_closure_string(CurrentNetwork::MAX_CLOSURES),
            gen_function_string(CurrentNetwork::MAX_FUNCTIONS)
        );
        // A program which is too big should fail.
        test_parse("", &program_too_big, false);
    }
}
