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

impl<N: Network> FromBytes for Proof<N> {
    /// Reads the proof from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid proof version"));
        }
        // Read the proof.
        let proof = FromBytes::read_le(&mut reader)?;
        // Return the proof.
        Ok(Self { proof })
    }
}

impl<N: Network> ToBytes for Proof<N> {
    /// Writes the proof to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        1u8.write_le(&mut writer)?;
        // Write the bytes.
        self.proof.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() -> Result<()> {
        // Sample the proof.
        let expected = crate::test_helpers::sample_proof();

        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, Proof::read_le(&expected_bytes[..])?);

        Ok(())
    }
}
