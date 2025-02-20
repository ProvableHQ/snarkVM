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
    for ProgramCoreV1<N, Instruction, Command>
{
    /// Parses a string into a program.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        // A helper to parse a program.
        enum P<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> {
            M(Mapping<N>),
            I(StructType<N>),
            R(RecordType<N>),
            C(ClosureCore<N, Instruction>),
            F(FunctionCore<N, Instruction, Command>),
        }

        // Parse the imports from the string.
        let (string, imports) = many0(Import::parse)(string)?;
        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;
        // Parse the 'program' keyword from the string.
        let (string, _) = tag(Self::type_name())(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the program ID from the string.
        let (string, id) = ProgramID::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the semicolon ';' keyword from the string.
        let (string, _) = tag(";")(string)?;

        fn intermediate<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>>(
            string: &str,
        ) -> ParserResult<P<N, Instruction, Command>> {
            // Parse the whitespace and comments from the string.
            let (string, _) = Sanitizer::parse(string)?;

            if string.starts_with(Mapping::<N>::type_name()) {
                map(Mapping::parse, |mapping| P::<N, Instruction, Command>::M(mapping))(string)
            } else if string.starts_with(StructType::<N>::type_name()) {
                map(StructType::parse, |struct_| P::<N, Instruction, Command>::I(struct_))(string)
            } else if string.starts_with(RecordType::<N>::type_name()) {
                map(RecordType::parse, |record| P::<N, Instruction, Command>::R(record))(string)
            } else if string.starts_with(ClosureCore::<N, Instruction>::type_name()) {
                map(ClosureCore::parse, |closure| P::<N, Instruction, Command>::C(closure))(string)
            } else if string.starts_with(FunctionCore::<N, Instruction, Command>::type_name()) {
                map(FunctionCore::parse, |function| P::<N, Instruction, Command>::F(function))(string)
            } else {
                Err(Err::Error(make_error(string, ErrorKind::Alt)))
            }
        }

        // Parse the struct or function from the string.
        let (string, components) = many1(intermediate)(string)?;
        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;

        // Initialize a new program.
        let mut program = match ProgramCoreV1::<N, Instruction, Command>::new(id) {
            Ok(program) => program,
            Err(error) => {
                eprintln!("{error}");
                return map_res(take(0usize), Err)(string);
            }
        };
        // Construct the program with the parsed components.
        for component in components {
            let result = match component {
                P::M(mapping) => program.add_mapping(mapping),
                P::I(struct_) => program.add_struct(struct_),
                P::R(record) => program.add_record(record),
                P::C(closure) => program.add_closure(closure),
                P::F(function) => program.add_function(function),
            };

            match result {
                Ok(_) => (),
                Err(error) => {
                    eprintln!("{error}");
                    return map_res(take(0usize), Err)(string);
                }
            }
        }
        // Lastly, add the imports (if any) to the program.
        for import in imports {
            match program.add_import(import) {
                Ok(_) => (),
                Err(error) => {
                    eprintln!("{error}");
                    return map_res(take(0usize), Err)(string);
                }
            }
        }

        Ok((string, program))
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> FromStr
    for ProgramCoreV1<N, Instruction, Command>
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
    for ProgramCoreV1<N, Instruction, Command>
{
    /// Prints the program as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> Display
    for ProgramCoreV1<N, Instruction, Command>
{
    /// Prints the program as a string.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if !self.imports.is_empty() {
            // Print the imports.
            for import in self.imports.values() {
                writeln!(f, "{import}")?;
            }

            // Print a newline.
            writeln!(f)?;
        }

        // Print the program name.
        write!(f, "{} {};\n\n", Self::type_name(), self.id)?;

        let mut identifier_iter = self.identifiers.iter().peekable();
        while let Some((identifier, definition)) = identifier_iter.next() {
            match definition {
                ProgramDefinition::Mapping => match self.mappings.get(identifier) {
                    Some(mapping) => writeln!(f, "{mapping}")?,
                    None => return Err(fmt::Error),
                },
                ProgramDefinition::Struct => match self.structs.get(identifier) {
                    Some(struct_) => writeln!(f, "{struct_}")?,
                    None => return Err(fmt::Error),
                },
                ProgramDefinition::Record => match self.records.get(identifier) {
                    Some(record) => writeln!(f, "{record}")?,
                    None => return Err(fmt::Error),
                },
                ProgramDefinition::Closure => match self.closures.get(identifier) {
                    Some(closure) => writeln!(f, "{closure}")?,
                    None => return Err(fmt::Error),
                },
                ProgramDefinition::Function => match self.functions.get(identifier) {
                    Some(function) => writeln!(f, "{function}")?,
                    None => return Err(fmt::Error),
                },
            }
            // Omit the last newline.
            if identifier_iter.peek().is_some() {
                writeln!(f)?;
            }
        }

        Ok(())
    }
}
