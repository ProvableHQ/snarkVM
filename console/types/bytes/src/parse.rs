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

impl<E: Environment> Parser for BytesType<E> {
    /// Parses hex-encoded bytes into a bytes type.
    #[inline]
    fn parse(s: &str) -> ParserResult<Self> {
        let (rest, value) = hex_parser::parse_hex_encoded_bytes(s)?;
        // Hex-encoded bytes are always even in length.
        if value.len() % 2 != 0 {
            return Err(Err::Error(VerboseError { errors: vec![(s, VerboseErrorKind::Nom(ErrorKind::LengthValue))] }));
        }
        // At this point we know that we're dealing with valid hex characters.
        let bytes = hex::decode(&value).unwrap();

        Ok((rest, Self::new(bytes)))
    }
}

impl<E: Environment> FromStr for BytesType<E> {
    type Err = Error;

    /// Parses a string into a bytes type.
    #[inline]
    fn from_str(string: &str) -> Result<Self> {
        match Self::parse(string) {
            Ok((remainder, object)) => {
                // Ensure the remainder is empty.
                ensure!(
                    remainder.is_empty(),
                    "Failed to parse hex-encoded bytes. Found invalid character in: \"{remainder}\""
                );
                // Return the object.
                Ok(object)
            }
            Err(error) => bail!("Failed to parse hex-encoded bytes. {error}"),
        }
    }
}

impl<E: Environment> Debug for BytesType<E> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<E: Environment> Display for BytesType<E> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Fill;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u32 = 100;

    #[test]
    fn test_display() -> Result<()> {
        // Ensure type and empty value fails.
        assert!(BytesType::<CurrentEnvironment>::parse(BytesType::<CurrentEnvironment>::type_name()).is_err());
        assert!(BytesType::<CurrentEnvironment>::parse("").is_err());

        let rng = &mut TestRng::default();
        let mut buffer = vec![0u8; CurrentEnvironment::MAX_DECODED_BYTES as usize];

        for _ in 0..ITERATIONS {
            // Sample random bytes.
            buffer.try_fill(rng).unwrap();
            // Hex-encode them.
            let encoded = hex::encode(&buffer);
            assert_eq!(encoded.len(), CurrentEnvironment::MAX_ENCODED_BYTES as usize);

            let candidate = BytesType::<CurrentEnvironment>::new(buffer.clone());
            assert_eq!(candidate.len(), CurrentEnvironment::MAX_DECODED_BYTES as usize);
            assert_eq!(candidate.to_string(), encoded);

            let candidate_recovered = BytesType::<CurrentEnvironment>::from_str(&encoded).unwrap();
            assert_eq!(candidate, candidate_recovered);
        }
        Ok(())
    }
}
