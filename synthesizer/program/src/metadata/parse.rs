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

impl<N: Network> Parser for ProgramMetadata<N> {
    /// Parses a string into a metadata.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;
        // Parse the 'metadata' keyword from the string.
        let (string, _) = tag(Self::type_name())(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the metadata name from the string.
        let (string, name) = Identifier::<N>::parse(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the colon ':' keyword from the string.
        let (string, _) = tag(":")(string)?;

        // Parse the whitespaces from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the value from the string.
        let (string, value) = Plaintext::parse(string)?;
        // Parse the whitespaces from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;

        // Parse the semicolon ';' keyword from the string.
        let (string, _) = tag(";")(string)?;

        // Return the metadata.
        Ok((string, Self::new(name, value)))
    }
}

impl<N: Network> FromStr for ProgramMetadata<N> {
    type Err = Error;

    /// Returns a metadata from a string literal.
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

impl<N: Network> Debug for ProgramMetadata<N> {
    /// Prints the metadata as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for ProgramMetadata<N> {
    /// Prints the metadata as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // Write the metadata to a string.
        write!(f, "{} {}: {};", Self::type_name(), self.name, self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_metadata_parse() {
        let metadata = ProgramMetadata::<CurrentNetwork>::parse(
            r"
$metadata foo: 1u8;",
        )
        .unwrap()
        .1;
        assert_eq!("foo", metadata.name().to_string());
        assert_eq!("1u8", metadata.value().to_string());
    }

    #[test]
    fn test_metadata_display() {
        let expected = "$metadata foo: {\n  bar: 1u8\n};";
        let metadata = ProgramMetadata::<CurrentNetwork>::parse(expected).unwrap().1;
        assert_eq!(expected, format!("{metadata}"),);
    }
}
