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

impl<N: Network> FromBytes for Metadata<N> {
    /// Reads the metadata from the buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid metadata version"));
        }

        // Read from the buffer.
        let network = u16::read_le(&mut reader)?;
        let round = u64::read_le(&mut reader)?;
        let height = u32::read_le(&mut reader)?;
        let cumulative_weight = u128::read_le(&mut reader)?;
        let cumulative_proof_target = u128::read_le(&mut reader)?;
        let coinbase_target = u64::read_le(&mut reader)?;
        let proof_target = u64::read_le(&mut reader)?;
        let last_coinbase_target = u64::read_le(&mut reader)?;
        let last_coinbase_timestamp = i64::read_le(&mut reader)?;
        let timestamp = i64::read_le(&mut reader)?;

        // Construct the metadata.
        Self::new(
            network,
            round,
            height,
            cumulative_weight,
            cumulative_proof_target,
            coinbase_target,
            proof_target,
            last_coinbase_target,
            last_coinbase_timestamp,
            timestamp,
        )
        .map_err(|e| error(e.to_string()))
    }
}

impl<N: Network> ToBytes for Metadata<N> {
    /// Writes the metadata to the buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        1u8.write_le(&mut writer)?;

        // Write to the buffer.
        self.network.write_le(&mut writer)?;
        self.round.write_le(&mut writer)?;
        self.height.write_le(&mut writer)?;
        self.cumulative_weight.write_le(&mut writer)?;
        self.cumulative_proof_target.write_le(&mut writer)?;
        self.coinbase_target.write_le(&mut writer)?;
        self.proof_target.write_le(&mut writer)?;
        self.last_coinbase_target.write_le(&mut writer)?;
        self.last_coinbase_timestamp.write_le(&mut writer)?;
        self.timestamp.write_le(&mut writer)
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

        for expected in [crate::header::metadata::test_helpers::sample_block_metadata(rng)].into_iter() {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(expected, Metadata::read_le(&expected_bytes[..])?);
            assert!(Metadata::<CurrentNetwork>::read_le(&expected_bytes[1..]).is_err());
        }
        Ok(())
    }
}
