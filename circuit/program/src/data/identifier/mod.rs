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

#[cfg(test)]
use snarkvm_circuit_types::environment::assert_scope;

mod equal;
mod from_bits;
mod from_field;
mod size_in_bits;
mod to_bits;
mod to_field;

use snarkvm_circuit_network::Aleo;
use snarkvm_circuit_types::{environment::prelude::*, Boolean, Field, U8};
use snarkvm_utilities::ToBits as TB;

/// An identifier is an **immutable** UTF-8 string,
/// represented as a **constant** field element in the circuit.
///
/// # Requirements
/// The identifier must not be an empty string.
/// The identifier must not start with a number.
/// The identifier must be alphanumeric, and may include underscores.
/// The identifier must not consist solely of underscores.
/// The identifier must fit within the data capacity of a base field element.
#[derive(Clone)]
pub struct Identifier<A: Aleo>(Field<A>, u8); // Number of bytes in the identifier.

#[cfg(console)]
impl<A: Aleo> Inject for Identifier<A> {
    type Primitive = console::Identifier<A::Network>;

    /// Initializes a new identifier from a string.
    /// Note: Identifiers are always `Mode::Constant`.
    fn new(_: Mode, identifier: Self::Primitive) -> Self {
        // Convert the identifier to a string to check its validity.
        let identifier = identifier.to_string();

        // Note: The string bytes themselves are **not** little-endian. Rather, they are order-preserving
        // for reconstructing the string when recovering the field element back into bytes.
        let field = Field::from_bits_le(&Vec::<Boolean<_>>::constant(identifier.to_bits_le()));

        // Return the identifier.
        Self(field, identifier.len() as u8)
    }
}

#[cfg(console)]
impl<A: Aleo> Eject for Identifier<A> {
    type Primitive = console::Identifier<A::Network>;

    /// Ejects the mode of the identifier.
    fn eject_mode(&self) -> Mode {
        debug_assert!(self.0.eject_mode() == Mode::Constant, "Identifier::eject_mode - Mode must be 'Constant'");
        Mode::Constant
    }

    /// Ejects the identifier as a string.
    fn eject_value(&self) -> Self::Primitive {
        match console::FromField::from_field(&self.0.eject_value()) {
            Ok(identifier) => identifier,
            Err(error) => A::halt(format!("Failed to convert an identifier to a string: {error}")),
        }
    }
}

#[cfg(console)]
impl<A: Aleo> Parser for Identifier<A> {
    /// Parses a UTF-8 string into an identifier.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        // Parse the identifier from the string.
        let (string, identifier) = console::Identifier::parse(string)?;

        Ok((string, Identifier::constant(identifier)))
    }
}

#[cfg(console)]
impl<A: Aleo> FromStr for Identifier<A> {
    type Err = Error;

    /// Parses a UTF-8 string into an identifier.
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

#[cfg(console)]
impl<A: Aleo> Debug for Identifier<A> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

#[cfg(console)]
impl<A: Aleo> Display for Identifier<A> {
    /// Prints the identifier as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.eject_value())
    }
}

impl<A: Aleo> Eq for Identifier<A> {}

impl<A: Aleo> PartialEq for Identifier<A> {
    /// Implements the `Eq` trait for the identifier.
    fn eq(&self, other: &Self) -> bool {
        self.0.eject_value() == other.0.eject_value()
    }
}

impl<A: Aleo> core::hash::Hash for Identifier<A> {
    /// Implements the `Hash` trait for the identifier.
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.eject_value().hash(state);
    }
}

impl<A: Aleo> From<Identifier<A>> for LinearCombination<A::BaseField> {
    /// Note: Identifier is always `Mode::Constant`.
    fn from(identifier: Identifier<A>) -> Self {
        From::from(&identifier)
    }
}

impl<A: Aleo> From<&Identifier<A>> for LinearCombination<A::BaseField> {
    /// Note: Identifier is always `Mode::Constant`.
    fn from(identifier: &Identifier<A>) -> Self {
        LinearCombination::from(&identifier.0)
    }
}

#[cfg(all(test, console))]
pub(crate) mod tests {
    use super::*;
    use crate::Circuit;
    use console::{Rng, TestRng};

    use anyhow::{bail, Result};
    use core::str::FromStr;
    use rand::distributions::Alphanumeric;

    /// Samples a random identifier.
    pub(crate) fn sample_console_identifier<A: Aleo>() -> Result<console::Identifier<A::Network>> {
        // Sample a random fixed-length alphanumeric string, that always starts with an alphabetic character.
        let string = sample_console_identifier_as_string::<A>()?;
        // Return the identifier.
        console::Identifier::from_str(&string)
    }

    /// Samples a random identifier as a string.
    pub(crate) fn sample_console_identifier_as_string<A: Aleo>() -> Result<String> {
        // Initialize a test RNG.
        let rng = &mut TestRng::default();
        // Sample a random fixed-length alphanumeric string, that always starts with an alphabetic character.
        let string = "a".to_string()
            + &rng
                .sample_iter(&Alphanumeric)
                .take(A::BaseField::size_in_data_bits() / (8 * 2))
                .map(char::from)
                .collect::<String>();
        // Ensure identifier fits within the data capacity of the base field.
        let max_bytes = A::BaseField::size_in_data_bits() / 8; // Note: This intentionally rounds down.
        match string.len() <= max_bytes {
            // Return the identifier.
            true => Ok(string),
            false => bail!("Identifier exceeds the maximum capacity allowed"),
        }
    }

    /// Samples a random identifier as a string.
    pub(crate) fn sample_lowercase_console_identifier_as_string<A: Aleo>() -> Result<String> {
        // Sample a random identifier.
        let string = sample_console_identifier_as_string::<A>()?;
        // Return the identifier as lowercase.
        Ok(string.to_lowercase())
    }

    #[test]
    fn test_identifier_parse() -> Result<()> {
        let candidate = Identifier::<Circuit>::parse("foo_bar").unwrap();
        assert_eq!("", candidate.0);
        assert_eq!(Identifier::<Circuit>::constant("foo_bar".try_into()?).eject(), candidate.1.eject());
        Ok(())
    }

    #[test]
    fn test_identifier_parse_fails() -> Result<()> {
        // Must be alphanumeric or underscore.
        let identifier = Identifier::<Circuit>::parse("foo_bar~baz").unwrap();
        assert_eq!(("~baz", Identifier::<Circuit>::from_str("foo_bar")?.eject()), (identifier.0, identifier.1.eject()));
        let identifier = Identifier::<Circuit>::parse("foo_bar-baz").unwrap();
        assert_eq!(("-baz", Identifier::<Circuit>::from_str("foo_bar")?.eject()), (identifier.0, identifier.1.eject()));

        // Must not be solely underscores.
        assert!(Identifier::<Circuit>::parse("_").is_err());
        assert!(Identifier::<Circuit>::parse("__").is_err());
        assert!(Identifier::<Circuit>::parse("___").is_err());
        assert!(Identifier::<Circuit>::parse("____").is_err());

        // Must not start with a number.
        assert!(Identifier::<Circuit>::parse("1").is_err());
        assert!(Identifier::<Circuit>::parse("2").is_err());
        assert!(Identifier::<Circuit>::parse("3").is_err());
        assert!(Identifier::<Circuit>::parse("1foo").is_err());
        assert!(Identifier::<Circuit>::parse("12").is_err());
        assert!(Identifier::<Circuit>::parse("111").is_err());

        // Must fit within the data capacity of a base field element.
        let identifier =
            Identifier::<Circuit>::parse("foo_bar_baz_qux_quux_quuz_corge_grault_garply_waldo_fred_plugh_xyzzy");
        assert!(identifier.is_err());
        Ok(())
    }

    #[test]
    fn test_identifier_display() -> Result<()> {
        let identifier = Identifier::<Circuit>::from_str("foo_bar")?;
        assert_eq!("foo_bar", format!("{identifier}"));
        Ok(())
    }

    #[test]
    fn test_identifier_bits() -> Result<()> {
        let identifier = Identifier::<Circuit>::from_str("foo_bar")?;
        assert_eq!(
            identifier.to_bits_le().eject(),
            Identifier::from_bits_le(&identifier.to_bits_le()).to_bits_le().eject()
        );
        assert_eq!(
            identifier.to_bits_be().eject(),
            Identifier::from_bits_be(&identifier.to_bits_be()).to_bits_be().eject()
        );
        Ok(())
    }
}
