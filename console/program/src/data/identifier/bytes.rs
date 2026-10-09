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

use super::*;

impl<N: Network> Identifier<N> {
    /// Reads the identifier string from a buffer.
    fn read_string_le<R: Read>(mut reader: R) -> IoResult<String> {
        // Read the number of bytes.
        let size = u8::read_le(&mut reader)?;

        // Read the identifier bytes.
        let mut buffer = vec![0u8; size as usize];
        reader.read_exact(&mut buffer)?;

        String::from_utf8(buffer).map_err(|e| error(format!("Failed to decode identifier: {e}")))
    }

    /// Reads in the identifier of a member access from a buffer. This also accepts `_nonce`.
    pub(crate) fn read_member_le<R: Read>(reader: R) -> IoResult<Self> {
        let string = Self::read_string_le(reader)?;
        match string == Self::RECORD_NONCE {
            true => Self::record_nonce(),
            false => Self::from_str(&string),
        }
        .map_err(|e| error(format!("{e}")))
    }
}

impl<N: Network> FromBytes for Identifier<N> {
    /// Reads in an identifier from a buffer.
    fn read_le<R: Read>(reader: R) -> IoResult<Self> {
        // from_str the identifier.
        // Note: `Self::from_str` ensures that the identifier string is not empty.
        Self::from_str(&Self::read_string_le(reader)?).map_err(|e| error(format!("{e}")))
    }
}

impl<N: Network> ToBytes for Identifier<N> {
    /// Writes an identifier to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Convert the identifier to a string.
        let string = self.to_string();
        if string.len() != self.1 as usize {
            return Err(error("Identifier length does not match expected size"));
        }

        // Ensure identifier fits within the data capacity of the base field.
        let max_bytes = Field::<N>::size_in_data_bits() / 8; // Note: This intentionally rounds down.
        if string.len() > max_bytes {
            return Err(error(format!("Identifier is too large. Identifiers must be <= {max_bytes} bytes long")));
        }

        // Write the identifier to a buffer.
        u8::try_from(string.len()).or_halt_with::<N>("Invalid identifier length").write_le(&mut writer)?;
        string.as_bytes().write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::identifier::tests::sample_identifier;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 1000;

    #[test]
    fn test_bytes() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random fixed-length alphanumeric identifier, that always starts with an alphabetic character.
            let expected = sample_identifier::<CurrentNetwork>(&mut rng)?;

            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(expected, Identifier::read_le(&expected_bytes[..])?);
        }
        Ok(())
    }

    #[test]
    fn test_zero_identifier_fails() {
        assert!(Identifier::<CurrentNetwork>::read_le(&[0u8; 1][..]).is_err())
    }

    #[test]
    fn test_record_nonce_fails() {
        let bytes = [&[6u8][..], b"_nonce"].concat();
        assert!(Identifier::<CurrentNetwork>::read_le(&bytes[..]).is_err());
    }

    #[test]
    fn test_read_member() -> Result<()> {
        let record_nonce = Identifier::<CurrentNetwork>::record_nonce()?;
        let bytes = record_nonce.to_bytes_le()?;
        assert_eq!(bytes, [&[6u8][..], b"_nonce"].concat());
        assert_eq!(record_nonce, Identifier::read_member_le(&bytes[..])?);
        assert!(Identifier::<CurrentNetwork>::read_member_le(&[&[4u8][..], b"_foo"].concat()[..]).is_err());

        let identifier = Identifier::<CurrentNetwork>::from_str("foo")?;
        assert_eq!(identifier, Identifier::read_member_le(&identifier.to_bytes_le()?[..])?);
        Ok(())
    }
}
