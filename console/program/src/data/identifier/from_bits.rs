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

impl<N: Network> FromBits for Identifier<N> {
    /// Initializes a new identifier from a list of little-endian bits *without* trailing zeros.
    fn from_bits_le(bits_le: &[bool]) -> Result<Self> {
        // Ensure the number of bits does not exceed the size in bits of the field.
        // This check is not sufficient to ensure the identifier is of valid size,
        // the final step checks the byte-aligned field element is within the data capacity.
        ensure!(bits_le.len() <= Field::<N>::size_in_bits(), "Identifier exceeds the maximum bits allowed");

        // Convert the bits to bytes, and parse the bytes as a UTF-8 string.
        let bytes = bits_le.chunks(8).map(u8::from_bits_le).collect::<Result<Vec<u8>>>()?;

        // Recover the identifier length from the bits, by finding the first instance of a `0` byte,
        // which is the null character '\0' in UTF-8, and an invalid character in an identifier.
        let num_bytes = match bytes.iter().position(|&byte| byte == 0) {
            Some(index) => index, // `index` is 0-indexed, and we exclude the null character.
            None => bytes.len(),  // No null character found, so the identifier is the full length.
        };

        // Parse the bytes as a UTF-8 string.
        Self::from_str(str::from_utf8(&bytes[0..num_bytes])?)
    }

    /// Initializes a new identifier from a list of big-endian bits *without* leading zeros.
    fn from_bits_be(bits_be: &[bool]) -> Result<Self> {
        Self::from_bits_le(&bits_be.iter().rev().copied().collect::<Vec<bool>>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::identifier::tests::sample_identifier;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: usize = 100;

    #[test]
    fn test_from_bits_le() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random fixed-length alphanumeric identifier, that always starts with an alphabetic character.
            let identifier = sample_identifier::<CurrentNetwork>(&mut rng)?;
            assert_eq!(identifier, Identifier::from_bits_le(&identifier.to_bits_le())?);
        }
        Ok(())
    }

    #[test]
    fn test_from_bits_be() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random fixed-length alphanumeric identifier, that always starts with an alphabetic character.
            let identifier = sample_identifier::<CurrentNetwork>(&mut rng)?;
            assert_eq!(identifier, Identifier::from_bits_be(&identifier.to_bits_be())?);
        }
        Ok(())
    }
}
