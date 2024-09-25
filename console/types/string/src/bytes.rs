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

impl<E: Environment> FromBytes for StringType<E> {
    /// Reads the string from a buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the number of bytes.
        let num_bytes = u16::read_le(&mut reader)?;
        // Ensure the number of bytes is within the allowed bounds.
        if num_bytes as u32 > E::MAX_STRING_BYTES {
            return Err(error(format!("String literal exceeds maximum length of {} bytes.", E::MAX_STRING_BYTES)));
        }
        // Read the bytes.
        let mut bytes = vec![0u8; num_bytes as usize];
        reader.read_exact(&mut bytes)?;
        // Return the string.
        Ok(Self::new(&String::from_utf8(bytes).map_err(|e| error(e.to_string()))?))
    }
}

impl<E: Environment> ToBytes for StringType<E> {
    /// Writes the string to a buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Ensure the number of bytes is within the allowed bounds.
        if self.string.len() > E::MAX_STRING_BYTES as usize {
            return Err(error(format!("String literal exceeds maximum length of {} bytes.", E::MAX_STRING_BYTES)));
        }
        // Write the number of bytes.
        u16::try_from(self.string.len()).or_halt_with::<E>("String exceeds u16::MAX bytes").write_le(&mut writer)?;
        // Write the bytes.
        self.string.as_bytes().write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 10_000;

    #[test]
    fn test_bytes() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a new string.
            let expected = StringType::<CurrentEnvironment>::rand(&mut rng);

            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(expected, StringType::read_le(&expected_bytes[..])?);
        }
        Ok(())
    }
}
