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

impl<N: Network> Parser for Future<N> {
    /// Parses a string into a future value.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        /// Parses an array of future arguments: `[arg_0, ..., arg_1]`.
        fn parse_arguments<N: Network>(string: &str) -> ParserResult<Vec<Argument<N>>> {
            // Parse the whitespace and comments from the string.
            let (string, _) = Sanitizer::parse(string)?;
            // Parse the "[" from the string.
            let (string, _) = tag("[")(string)?;
            // Parse the whitespace from the string.
            let (string, _) = Sanitizer::parse(string)?;
            // Parse the members.
            let (string, arguments) = separated_list0(
                pair(pair(Sanitizer::parse_whitespaces, tag(",")), Sanitizer::parse),
                alt((map(Future::parse, Argument::Future), map(Plaintext::parse, Argument::Plaintext))),
            )(string)?;
            // Parse the whitespace and comments from the string.
            let (string, _) = Sanitizer::parse(string)?;
            // Parse the ']' from the string.
            let (string, _) = tag("]")(string)?;
            // Output the plaintext.
            Ok((string, arguments))
        }

        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;
        // Parse the "{" from the string.
        let (string, _) = tag("{")(string)?;

        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;
        // Parse the "program_id" from the string.
        let (string, _) = tag("program_id")(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the ":" from the string.
        let (string, _) = tag(":")(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the program ID from the string.
        let (string, program_id) = ProgramID::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the "," from the string.
        let (string, _) = tag(",")(string)?;

        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;
        // Parse the "function_name" from the string.
        let (string, _) = tag("function_name")(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the ":" from the string.
        let (string, _) = tag(":")(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the function name from the string.
        let (string, function_name) = Identifier::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the "," from the string.
        let (string, _) = tag(",")(string)?;

        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;
        // Parse the "arguments" from the string.
        let (string, _) = tag("arguments")(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the ":" from the string.
        let (string, _) = tag(":")(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the arguments from the string.
        let (string, arguments) = parse_arguments(string)?;

        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;
        // Parse the "}" from the string.
        let (string, _) = tag("}")(string)?;

        Ok((string, Self::new(program_id, function_name, arguments)))
    }
}

impl<N: Network> FromStr for Future<N> {
    type Err = Error;

    /// Returns a future from a string literal.
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

impl<N: Network> Debug for Future<N> {
    /// Prints the future as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for Future<N> {
    /// Prints the future as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.fmt_internal(f, 0)
    }
}

impl<N: Network> Future<N> {
    /// Prints the future with the given indentation depth.
    fn fmt_internal(&self, f: &mut Formatter, depth: usize) -> fmt::Result {
        /// The number of spaces to indent.
        const INDENT: usize = 2;

        // Print the opening brace.
        write!(f, "{{")?;

        // Print the program ID.
        write!(
            f,
            "\n{:indent$}program_id: {program_id},",
            "",
            indent = (depth + 1) * INDENT,
            program_id = self.program_id()
        )?;
        // Print the function name.
        write!(
            f,
            "\n{:indent$}function_name: {function_name},",
            "",
            indent = (depth + 1) * INDENT,
            function_name = self.function_name()
        )?;
        // Print the arguments.
        // If the arguments are empty, print an empty array.
        if self.arguments.is_empty() {
            write!(f, "\n{:indent$}arguments: []", "", indent = (depth + 1) * INDENT)?;
        } else {
            write!(f, "\n{:indent$}arguments: [", "", indent = (depth + 1) * INDENT)?;
            self.arguments.iter().enumerate().try_for_each(|(i, argument)| {
                match argument {
                    Argument::Plaintext(plaintext) => match i == self.arguments.len() - 1 {
                        true => {
                            // Print the last argument without a comma.
                            write!(
                                f,
                                "\n{:indent$}{plaintext}",
                                "",
                                indent = (depth + 2) * INDENT,
                                plaintext = plaintext
                            )
                        }
                        // Print the argument with a comma.
                        false => {
                            write!(
                                f,
                                "\n{:indent$}{plaintext},",
                                "",
                                indent = (depth + 2) * INDENT,
                                plaintext = plaintext
                            )
                        }
                    },
                    Argument::Future(future) => {
                        // Print a newline.
                        write!(f, "\n{:indent$}", "", indent = (depth + 2) * INDENT)?;
                        // Print the argument.
                        future.fmt_internal(f, depth + 2)?;
                        // Print the closing brace.
                        match i == self.arguments.len() - 1 {
                            // Print the last member without a comma.
                            true => write!(f, "\n{:indent$}", "", indent = (depth + 1) * INDENT),
                            // Print the member with a comma.
                            false => write!(f, ","),
                        }
                    }
                }
            })?;
            // Print the closing bracket.
            write!(f, "\n{:indent$}]", "", indent = (depth + 1) * INDENT)?;
        }

        // Print the closing brace.
        write!(f, "\n{:indent$}}}", "", indent = depth * INDENT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_parse_future() -> Result<()> {
        // No argument case.
        let expected = r"{
  program_id: credits.aleo,
  function_name: transfer,
  arguments: []
}";
        let (remainder, candidate) =
            Future::<CurrentNetwork>::parse("{ program_id: credits.aleo, function_name: transfer, arguments: [] }")?;
        assert!(remainder.is_empty());
        assert_eq!(expected, candidate.to_string());
        assert_eq!("", remainder);

        // Literal arguments.
        let expected = r"{
  program_id: credits.aleo,
  function_name: transfer_public_to_private,
  arguments: [
    aleo1g8qul5a44vk22u9uuvaewdcjw4v6xg8wx0llru39nnjn7eu08yrscxe4e2,
    100000000u64
  ]
}";
        let (remainder, candidate) = Future::<CurrentNetwork>::parse(
            "{ program_id: credits.aleo, function_name: transfer_public_to_private, arguments: [ aleo1g8qul5a44vk22u9uuvaewdcjw4v6xg8wx0llru39nnjn7eu08yrscxe4e2, 100000000u64 ] }",
        )?;
        assert!(remainder.is_empty());
        assert_eq!(expected, candidate.to_string());
        assert_eq!("", remainder);

        Ok(())
    }
}
