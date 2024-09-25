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

impl<N: Network> FromBytes for Ciphertext<N> {
    /// Reads the ciphertext from a buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the number of field elements.
        let num_fields = u16::read_le(&mut reader)?;
        // Ensure the number of field elements does not exceed the maximum allowed size.
        match num_fields as u32 <= N::MAX_DATA_SIZE_IN_FIELDS {
            // Read the field elements.
            true => {
                Ok(Ciphertext((0..num_fields).map(|_| Field::read_le(&mut reader)).collect::<Result<Vec<_>, _>>()?))
            }
            false => Err(error("Ciphertext is too large to encode in field elements.")),
        }
    }
}

impl<N: Network> ToBytes for Ciphertext<N> {
    /// Writes the ciphertext to a buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Ensure the number of field elements does not exceed the maximum allowed size.
        if u32::try_from(self.0.len()).or_halt::<N>() > N::MAX_DATA_SIZE_IN_FIELDS || self.0.len() > u16::MAX as usize {
            return Err(error("Ciphertext is too large to encode in field elements."));
        }
        // Write the number of ciphertext field elements.
        u16::try_from(self.0.len()).or_halt::<N>().write_le(&mut writer)?;
        // Write the ciphertext field elements.
        self.0.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u32 = 1000;

    #[test]
    fn test_bytes() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a new ciphertext.
            let expected = Ciphertext::<CurrentNetwork>((0..100).map(|_| Uniform::rand(&mut rng)).collect::<Vec<_>>());

            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(expected, Ciphertext::read_le(&expected_bytes[..])?);
        }
        Ok(())
    }
}
