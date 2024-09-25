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

impl<N: Network> Parser for RegisterType<N> {
    /// Parses a string into a register type.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        // Parse the mode from the string (ordering matters).
        alt((
            map(pair(Locator::parse, tag(".future")), |(locator, _)| Self::Future(locator)),
            map(pair(Locator::parse, tag(".record")), |(locator, _)| Self::ExternalRecord(locator)),
            map(pair(Identifier::parse, tag(".record")), |(identifier, _)| Self::Record(identifier)),
            map(PlaintextType::parse, |plaintext_type| Self::Plaintext(plaintext_type)),
        ))(string)
    }
}

impl<N: Network> FromStr for RegisterType<N> {
    type Err = Error;

    /// Returns a register type from a string literal.
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

impl<N: Network> Debug for RegisterType<N> {
    /// Prints the register type as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for RegisterType<N> {
    /// Prints the register type as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            // Prints the plaintext type, i.e. signature
            Self::Plaintext(plaintext_type) => write!(f, "{plaintext_type}"),
            // Prints the record name, i.e. token.record
            Self::Record(record_name) => write!(f, "{record_name}.record"),
            // Prints the locator, i.e. token.aleo/token.record
            Self::ExternalRecord(locator) => write!(f, "{locator}.record"),
            // Prints the future type, i.e. future
            Self::Future(locator) => write!(f, "{locator}.future"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_parse() -> Result<()> {
        // Literal type.
        assert_eq!(
            Ok(("", RegisterType::<CurrentNetwork>::Plaintext(PlaintextType::from_str("field")?))),
            RegisterType::<CurrentNetwork>::parse("field")
        );

        // Struct type.
        assert_eq!(
            Ok(("", RegisterType::<CurrentNetwork>::Plaintext(PlaintextType::from_str("signature")?))),
            RegisterType::<CurrentNetwork>::parse("signature")
        );
        assert_eq!(
            Ok(("", RegisterType::<CurrentNetwork>::Plaintext(PlaintextType::from_str("u8kldsafj")?))),
            RegisterType::<CurrentNetwork>::parse("u8kldsafj")
        );

        // Record type.
        assert_eq!(
            Ok(("", RegisterType::<CurrentNetwork>::Record(Identifier::from_str("token")?))),
            RegisterType::<CurrentNetwork>::parse("token.record")
        );

        // ExternalRecord type.
        assert_eq!(
            Ok(("", RegisterType::<CurrentNetwork>::ExternalRecord(Locator::from_str("token.aleo/token")?))),
            RegisterType::<CurrentNetwork>::parse("token.aleo/token.record")
        );

        Ok(())
    }

    #[test]
    fn test_parse_fails() -> Result<()> {
        // Must be non-empty.
        assert!(RegisterType::<CurrentNetwork>::parse("").is_err());

        // Invalid characters.
        assert!(RegisterType::<CurrentNetwork>::parse("{}").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("_").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("__").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("___").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("-").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("--").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("---").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("*").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("**").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("***").is_err());

        // Must not start with a number.
        assert!(RegisterType::<CurrentNetwork>::parse("1").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("2").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("3").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("1foo").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("12").is_err());
        assert!(RegisterType::<CurrentNetwork>::parse("111").is_err());

        // Must fit within the data capacity of a base field element.
        let struct_ = RegisterType::<CurrentNetwork>::parse(
            "foo_bar_baz_qux_quux_quuz_corge_grault_garply_waldo_fred_plugh_xyzzy.private",
        );
        assert!(struct_.is_err());

        Ok(())
    }

    #[test]
    fn test_display() -> Result<()> {
        assert_eq!(RegisterType::<CurrentNetwork>::from_str("field")?.to_string(), "field");
        assert_eq!(RegisterType::<CurrentNetwork>::from_str("signature")?.to_string(), "signature");
        assert_eq!(RegisterType::<CurrentNetwork>::from_str("token.record")?.to_string(), "token.record");
        assert_eq!(
            RegisterType::<CurrentNetwork>::from_str("token.aleo/token.record")?.to_string(),
            "token.aleo/token.record"
        );
        Ok(())
    }
}
