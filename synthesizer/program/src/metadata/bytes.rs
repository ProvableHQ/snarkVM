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

impl<N: Network> FromBytes for ProgramMetadata<N> {
    /// Reads the metadata from a buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the metadata name.
        let name = Identifier::<N>::read_le(&mut reader)?;
        // Read the value statement.
        let value = FromBytes::read_le(&mut reader)?;
        // Return the new metadata.
        Ok(Self::new(name, value))
    }
}

impl<N: Network> ToBytes for ProgramMetadata<N> {
    /// Writes the metadata to a buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the metadata name.
        self.name.write_le(&mut writer)?;
        // Write the value statement.
        self.value.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_metadata_bytes() -> Result<()> {
        let metadata_string = r"
$metadata edition: 0u8;";

        let expected = ProgramMetadata::<CurrentNetwork>::from_str(metadata_string)?;
        let expected_bytes = expected.to_bytes_le()?;
        println!("String size: {:?}, Bytecode size: {:?}", metadata_string.as_bytes().len(), expected_bytes.len());

        let candidate = ProgramMetadata::<CurrentNetwork>::from_bytes_le(&expected_bytes)?;
        assert_eq!(expected.to_string(), candidate.to_string());
        assert_eq!(expected_bytes, candidate.to_bytes_le()?);
        Ok(())
    }
}
