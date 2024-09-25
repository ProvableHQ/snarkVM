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

impl<N: Network> FromBytes for Header<N> {
    /// Reads the block header from the buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid header version"));
        }

        // Read from the buffer.
        let previous_state_root = N::StateRoot::read_le(&mut reader)?;
        let transactions_root = Field::<N>::read_le(&mut reader)?;
        let finalize_root = Field::<N>::read_le(&mut reader)?;
        let ratifications_root = Field::<N>::read_le(&mut reader)?;
        let solutions_root = Field::<N>::read_le(&mut reader)?;
        let subdag_root = Field::<N>::read_le(&mut reader)?;
        let metadata = Metadata::read_le(&mut reader)?;

        // Construct the block header.
        Self::from(
            previous_state_root,
            transactions_root,
            finalize_root,
            ratifications_root,
            solutions_root,
            subdag_root,
            metadata,
        )
        .map_err(|e| error(e.to_string()))
    }
}

impl<N: Network> ToBytes for Header<N> {
    /// Writes the block header to the buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        1u8.write_le(&mut writer)?;

        // Write to the buffer.
        self.previous_state_root.write_le(&mut writer)?;
        self.transactions_root.write_le(&mut writer)?;
        self.finalize_root.write_le(&mut writer)?;
        self.ratifications_root.write_le(&mut writer)?;
        self.solutions_root.write_le(&mut writer)?;
        self.subdag_root.write_le(&mut writer)?;
        self.metadata.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_bytes() -> Result<()> {
        let rng = &mut TestRng::default();

        for expected in [crate::header::test_helpers::sample_block_header(rng)].into_iter() {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(expected, Header::read_le(&expected_bytes[..])?);
            assert!(Header::<CurrentNetwork>::read_le(&expected_bytes[1..]).is_err());
        }
        Ok(())
    }
}
