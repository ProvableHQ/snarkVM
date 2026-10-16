// Copyright (c) 2019-2026 Provable Inc.
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

mod bytes;
mod equal;
mod from_bits;
mod from_field;
mod parse;
mod serialize;
mod size_in_bits;
mod to_bits;
mod to_field;

use snarkvm_console_network::Network;
use snarkvm_console_types::{Field, prelude::*};

/// An identifier is an **immutable** UTF-8 string,
/// represented as a **constant** field element in the CurrentNetwork.
///
/// # Requirements
/// The identifier must not be an empty string.
/// The identifier must not start with a number.
/// The identifier must be alphanumeric, and may include underscores.
/// The identifier must not consist solely of underscores.
/// The identifier must fit within the data capacity of a base field element.
#[derive(Copy, Clone)]
pub struct Identifier<N: Network>(Field<N>, u8); // Number of bytes in the identifier.

impl<N: Network> Identifier<N> {
    /// The member that reads a record's nonce, as in `r0._nonce`.
    const RECORD_NONCE: &'static str = "_nonce";

    /// Returns the `_nonce` member of a record access.
    /// `_nonce` fails `Identifier::from_str`, so programs cannot declare it as a name.
    pub fn record_nonce() -> Result<Self> {
        let field = Field::<N>::from_bits_le(&Self::RECORD_NONCE.as_bytes().to_bits_le())?;
        Ok(Self(field, u8::try_from(Self::RECORD_NONCE.len())?))
    }

    /// Returns `true` when this identifier is the `_nonce` member of a record access.
    pub fn is_record_nonce(&self) -> bool {
        // `_nonce` is six ASCII bytes, so the field conversion cannot fail.
        self.1 as usize == Self::RECORD_NONCE.len()
            && self.0 == Self::record_nonce().expect("`_nonce` fits in an identifier field").0
    }
}

impl<N: Network> From<&Identifier<N>> for Identifier<N> {
    /// Returns a copy of the identifier.
    fn from(identifier: &Identifier<N>) -> Self {
        *identifier
    }
}

impl<N: Network> TryFrom<String> for Identifier<N> {
    type Error = Error;

    /// Initializes an identifier from a string.
    fn try_from(identifier: String) -> Result<Self> {
        Self::from_str(&identifier)
    }
}

impl<N: Network> TryFrom<&String> for Identifier<N> {
    type Error = Error;

    /// Initializes an identifier from a string.
    fn try_from(identifier: &String) -> Result<Self> {
        Self::from_str(identifier)
    }
}

impl<N: Network> TryFrom<&str> for Identifier<N> {
    type Error = Error;

    /// Initializes an identifier from a string.
    fn try_from(identifier: &str) -> Result<Self> {
        Self::from_str(identifier)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: usize = 100;

    /// Samples a random identifier.
    pub(crate) fn sample_identifier<N: Network>(rng: &mut TestRng) -> Result<Identifier<N>> {
        // Sample a random fixed-length alphanumeric string, that always starts with an alphabetic character.
        let string = sample_identifier_as_string::<N>(rng)?;
        // Recover the field element from the bits.
        let field = Field::<N>::from_bits_le(&string.as_bytes().to_bits_le())?;
        // Return the identifier.
        Ok(Identifier(field, u8::try_from(string.len()).or_halt_with::<CurrentNetwork>("Invalid identifier length")))
    }

    /// Samples a random identifier as a string.
    pub(crate) fn sample_identifier_as_string<N: Network>(rng: &mut TestRng) -> Result<String> {
        // Sample a random fixed-length alphanumeric string, that always starts with an alphabetic character.
        let string = "a".to_string()
            + &rng
                .sample_iter(&Alphanumeric)
                .take(Field::<N>::size_in_data_bits() / (8 * 2))
                .map(char::from)
                .collect::<String>();
        // Ensure identifier fits within the data capacity of the base field.
        let max_bytes = Field::<N>::size_in_data_bits() / 8; // Note: This intentionally rounds down.
        match string.len() <= max_bytes {
            // Return the identifier.
            true => Ok(string),
            false => bail!("Identifier exceeds the maximum capacity allowed"),
        }
    }

    /// Samples a random lowercase identifier as a string.
    pub(crate) fn sample_lowercase_identifier_as_string<N: Network>(rng: &mut TestRng) -> Result<String> {
        // Sample a random identifier.
        let string = sample_identifier_as_string::<N>(rng)?;
        // Return the identifier as lowercase.
        Ok(string.to_lowercase())
    }

    #[test]
    fn test_try_from() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random fixed-length alphanumeric string, that always starts with an alphabetic character.
            let expected_string = sample_identifier_as_string::<CurrentNetwork>(&mut rng)?;
            // Recover the field element from the bits.
            let expected_field = Field::<CurrentNetwork>::from_bits_le(&expected_string.as_bytes().to_bits_le())?;

            // Try to initialize an identifier from the string.
            let candidate = Identifier::<CurrentNetwork>::try_from(expected_string.as_str())?;
            assert_eq!(expected_field, candidate.0);
            assert_eq!(expected_string.len(), candidate.1 as usize);
        }
        Ok(())
    }

    #[test]
    fn test_identifier_try_from_illegal() {
        assert!(Identifier::<CurrentNetwork>::try_from("123").is_err());
        assert!(Identifier::<CurrentNetwork>::try_from("abc\x08def").is_err());
        assert!(Identifier::<CurrentNetwork>::try_from("abc\u{202a}def").is_err());
    }

    #[test]
    fn test_is_record_nonce() -> Result<()> {
        let nonce = Identifier::<CurrentNetwork>::record_nonce()?;
        assert!(nonce.is_record_nonce());
        assert_eq!(nonce.to_string(), "_nonce");

        // `amount` has the same length as `_nonce`.
        assert!(!Identifier::<CurrentNetwork>::from_str("amount")?.is_record_nonce());
        assert!(!Identifier::<CurrentNetwork>::from_str("owner")?.is_record_nonce());
        Ok(())
    }
}
