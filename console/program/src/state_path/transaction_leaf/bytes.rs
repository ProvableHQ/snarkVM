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

impl<N: Network> FromBytes for TransactionLeaf<N> {
    /// Reads the transaction leaf from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the variant.
        let variant = FromBytes::read_le(&mut reader)?;
        // Read the index.
        let index = FromBytes::read_le(&mut reader)?;
        // Read the ID.
        let id = FromBytes::read_le(&mut reader)?;
        // Return the transaction leaf.
        Ok(Self::from(variant, index, id))
    }
}

impl<N: Network> ToBytes for TransactionLeaf<N> {
    /// Writes the transaction leaf to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the variant.
        self.variant.write_le(&mut writer)?;
        // Write the index.
        self.index.write_le(&mut writer)?;
        // Write the ID.
        self.id.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITERATIONS: u64 = 1000;

    #[test]
    fn test_bytes() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample the leaf.
            let expected = test_helpers::sample_leaf(&mut rng);

            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(expected, TransactionLeaf::read_le(&expected_bytes[..])?);
        }
        Ok(())
    }
}
