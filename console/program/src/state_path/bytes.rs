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

impl<N: Network> FromBytes for StatePath<N> {
    /// Reads the path from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid state path version"));
        }

        // Read the state path.
        let global_state_root = N::StateRoot::read_le(&mut reader)?;

        let block_path = BlockPath::read_le(&mut reader)?;
        let block_hash = N::BlockHash::read_le(&mut reader)?;
        let previous_block_hash = N::BlockHash::read_le(&mut reader)?;
        let header_root = Field::read_le(&mut reader)?;
        let header_path = HeaderPath::read_le(&mut reader)?;
        let header_leaf = HeaderLeaf::read_le(&mut reader)?;
        let transactions_path = TransactionsPath::read_le(&mut reader)?;

        let transaction_id = FromBytes::read_le(&mut reader)?;
        let transaction_path = FromBytes::read_le(&mut reader)?;
        let transaction_leaf = FromBytes::read_le(&mut reader)?;
        let transition_root = Field::read_le(&mut reader)?;
        let tcm = FromBytes::read_le(&mut reader)?;
        let transition_path = FromBytes::read_le(&mut reader)?;
        let transition_leaf = FromBytes::read_le(&mut reader)?;

        // Construct the state path.
        Ok(Self::from(
            global_state_root,
            block_path,
            block_hash,
            previous_block_hash,
            header_root,
            header_path,
            header_leaf,
            transactions_path,
            transaction_id,
            transaction_path,
            transaction_leaf,
            transition_root,
            tcm,
            transition_path,
            transition_leaf,
        ))
    }
}

impl<N: Network> ToBytes for StatePath<N> {
    /// Writes the path to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        1u8.write_le(&mut writer)?;

        // Write the state path.
        self.global_state_root.write_le(&mut writer)?;

        self.block_path.write_le(&mut writer)?;
        self.block_hash.write_le(&mut writer)?;
        self.previous_block_hash.write_le(&mut writer)?;
        self.header_root.write_le(&mut writer)?;
        self.header_path.write_le(&mut writer)?;
        self.header_leaf.write_le(&mut writer)?;
        self.transactions_path.write_le(&mut writer)?;

        self.transaction_id.write_le(&mut writer)?;
        self.transaction_path.write_le(&mut writer)?;
        self.transaction_leaf.write_le(&mut writer)?;
        self.transition_root.write_le(&mut writer)?;
        self.tcm.write_le(&mut writer)?;
        self.transition_path.write_le(&mut writer)?;
        self.transition_leaf.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: usize = 100;

    #[test]
    fn test_bytes() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample the state path.
            let expected =
                crate::state_path::test_helpers::sample_global_state_path::<CurrentNetwork>(None, &mut rng).unwrap();

            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, StatePath::read_le(&expected_bytes[..]).unwrap());
            assert!(StatePath::<CurrentNetwork>::read_le(&expected_bytes[1..]).is_err());
        }
    }
}
