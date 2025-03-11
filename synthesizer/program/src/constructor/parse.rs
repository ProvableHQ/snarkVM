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

impl<N: Network, Command: CommandTrait<N>> Parser for ConstructorCore<N, Command> {
    /// Parses a string into constructor.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        // Parse the whitespace and comments from the string.
        let (string, _) = Sanitizer::parse(string)?;
        // Parse the 'constructor' keyword from the string.
        let (string, _) = tag(Self::type_name())(string)?;
        // Parse the whitespace from the string.
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        // Parse the colon ':' keyword from the string.
        let (string, _) = tag(":")(string)?;

        // Parse the commands from the string.
        let (string, commands) = many0(Command::parse)(string)?;

        map_res(take(0usize), move |_| {
            // Initialize a new constructor.
            let mut constructor = Self::new();
            if let Err(error) = commands.iter().cloned().try_for_each(|command| constructor.add_command(command)) {
                eprintln!("{error}");
                return Err(error);
            }
            Ok::<_, Error>(constructor)
        })(string)
    }
}

impl<N: Network, Command: CommandTrait<N>> FromStr for ConstructorCore<N, Command> {
    type Err = Error;

    /// Returns a constructor from a string literal.
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

impl<N: Network, Command: CommandTrait<N>> Debug for ConstructorCore<N, Command> {
    /// Prints the constructor as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network, Command: CommandTrait<N>> Display for ConstructorCore<N, Command> {
    /// Prints the constructor as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // Write the constructor to a string.
        write!(f, "{}:", Self::type_name())?;
        self.commands.iter().try_for_each(|command| write!(f, "\n    {command}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Constructor;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_constructor_parse() {
        let constructor = Constructor::<CurrentNetwork>::parse(
            r"
_init:
    add r0 r1 into r2;",
        )
        .unwrap()
        .1;
        assert_eq!(1, constructor.commands().len());

        // Constructor with 0 inputs.
        let constructor = Constructor::<CurrentNetwork>::parse(
            r"
_init:
    add 1u32 2u32 into r0;",
        )
        .unwrap()
        .1;
        assert_eq!(1, constructor.commands().len());
    }

    #[test]
    fn test_constructor_parse_cast() {
        let constructor = Constructor::<CurrentNetwork>::parse(
            r"
_init:
    cast 1u8 2u8 into r1 as token;",
        )
        .unwrap()
        .1;
        assert_eq!(1, constructor.commands().len());
    }

    #[test]
    fn test_constructor_display() {
        let expected = r"_init:
    add r0 r1 into r2;";
        let constructor = Constructor::<CurrentNetwork>::parse(expected).unwrap().1;
        assert_eq!(expected, format!("{constructor}"),);
    }
}
