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

use super::*;

impl<N: Network> Parser for DynamicFuture<N> {
    /// Parses a string into a future value.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
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
        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;
        // Parse the "function_name" from the string.
        let (string, _) = tag("commitment")(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the ":" from the string.
        let (string, _) = tag(":")(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the commitment from the string.
        let (string, commitment) = Field::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;

        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;
        // Parse the "}" from the string.
        let (string, _) = tag("}")(string)?;

        Ok((string, Self::new(program_id, function_name, commitment)))
    }
}

impl<N: Network> FromStr for DynamicFuture<N> {
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

impl<N: Network> Debug for DynamicFuture<N> {
    /// Prints the future as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for DynamicFuture<N> {
    /// Prints the future as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.fmt_internal(f, 0)
    }
}

impl<N: Network> DynamicFuture<N> {
    /// Prints the future with the given indentation depth.
    pub(crate) fn fmt_internal(&self, f: &mut Formatter, depth: usize) -> fmt::Result {
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
        // Print the commitment.
        write!(
            f,
            "\n{:indent$}commitment: {commitment},",
            "",
            indent = (depth + 1) * INDENT,
            commitment = self.commitment()
        )?;
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
        let (remainder, candidate) = DynamicFuture::<CurrentNetwork>::parse(
            "{ program_id: credits.aleo, function_name: transfer, arguments: [] }",
        )?;
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
        let (remainder, candidate) = DynamicFuture::<CurrentNetwork>::parse(
            "{ program_id: credits.aleo, function_name: transfer_public_to_private, arguments: [ aleo1g8qul5a44vk22u9uuvaewdcjw4v6xg8wx0llru39nnjn7eu08yrscxe4e2, 100000000u64 ] }",
        )?;
        assert!(remainder.is_empty());
        assert_eq!(expected, candidate.to_string());
        assert_eq!("", remainder);

        Ok(())
    }

    #[test]
    fn test_deeply_nested_future() {
        // A helper function to iteratively create a deeply nested future.
        fn create_nested_future(depth: usize) -> String {
            // Define the base case.
            let root = r"{
                program_id: foo.aleo,
                function_name: bar,
                arguments: []
            }";
            // Define the prefix and suffix for the nested future.
            let prefix = r"{
                program_id: foo.aleo,
                function_name: bar,
                arguments: ["
                .repeat(depth);
            let suffix = r"]}".repeat(depth);
            // Concatenate the prefix, root, and suffix to create the nested future.
            format!("{}{}{}", prefix, root, suffix)
        }

        // A helper function to test the parsing of a deeply nested future.
        fn run_test(depth: usize, expected_error: bool) {
            // Create the nested future string.
            let nested_future_string = create_nested_future(depth);
            // Parse the nested future.
            let result = DynamicFuture::<CurrentNetwork>::parse(&nested_future_string);
            // Check if the result is an error.
            match expected_error {
                true => {
                    assert!(result.is_err());
                    return;
                }
                false => assert!(result.is_ok()),
            };
            // Unwrap the result.
            let (remainder, candidate) = result.unwrap();
            // Ensure the remainder is empty.
            assert!(
                remainder.is_empty(),
                "Failed to parse deeply nested future. Found invalid character in: \"{remainder}\""
            );
            // Strip the expected string of whitespace.
            let expected = nested_future_string.replace("\n", "").replace(" ", "").replace("\t", "");
            // Strip the candidate string of whitespace.
            let candidate_str = candidate.to_string().replace("\n", "").replace(" ", "").replace("\t", "");
            // Ensure the expected and candidate strings are equal.
            assert_eq!(expected, candidate_str, "Expected: {expected}, Candidate: {candidate_str}");
        }

        // Initialize a set of depths to test.
        let mut depths = (0usize..100).collect_vec();
        depths.extend((100..1000).step_by(100));
        depths.extend((1000..10000).step_by(1000));
        depths.extend((10000..100000).step_by(10000));

        // For each depth, test the parsing of a deeply nested future.
        for depth in depths {
            run_test(depth, depth > CurrentNetwork::MAX_DATA_DEPTH);
        }
    }
}
