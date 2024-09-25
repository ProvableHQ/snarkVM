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

impl<N: Network> FromBytes for FinalizeOperation<N> {
    /// Reads the finalize operation from buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the variant.
        let variant = u8::read_le(&mut reader)?;
        // Read the finalize operation.
        match variant {
            0 => {
                // Read the mapping ID.
                let mapping_id = Field::read_le(&mut reader)?;
                // Return the finalize operation.
                Ok(Self::InitializeMapping(mapping_id))
            }
            1 => {
                // Read the mapping ID.
                let mapping_id = Field::read_le(&mut reader)?;
                // Read the key ID.
                let key_id = Field::read_le(&mut reader)?;
                // Read the value ID.
                let value_id = Field::read_le(&mut reader)?;
                // Return the finalize operation.
                Ok(Self::InsertKeyValue(mapping_id, key_id, value_id))
            }
            2 => {
                // Read the mapping ID.
                let mapping_id = Field::read_le(&mut reader)?;
                // Read the key ID.
                let key_id = Field::read_le(&mut reader)?;
                // Read the value ID.
                let value_id = Field::read_le(&mut reader)?;
                // Return the finalize operation.
                Ok(Self::UpdateKeyValue(mapping_id, key_id, value_id))
            }
            3 => {
                // Read the mapping ID.
                let mapping_id = Field::read_le(&mut reader)?;
                // Read the key ID.
                let key_id = Field::read_le(&mut reader)?;
                // Return the finalize operation.
                Ok(Self::RemoveKeyValue(mapping_id, key_id))
            }
            4 => {
                // Read the mapping ID.
                let mapping_id = Field::read_le(&mut reader)?;
                // Return the finalize operation.
                Ok(Self::ReplaceMapping(mapping_id))
            }
            5 => {
                // Read the mapping ID.
                let mapping_id = Field::read_le(&mut reader)?;
                // Return the finalize operation.
                Ok(Self::RemoveMapping(mapping_id))
            }
            6.. => Err(error(format!("Failed to decode finalize operation variant {variant}"))),
        }
    }
}

impl<N: Network> ToBytes for FinalizeOperation<N> {
    /// Writes the finalize operation to buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Self::InitializeMapping(mapping_id) => {
                // Write the variant.
                0u8.write_le(&mut writer)?;
                // Write the mapping ID.
                mapping_id.write_le(&mut writer)?;
            }
            Self::InsertKeyValue(mapping_id, key_id, value_id) => {
                // Write the variant.
                1u8.write_le(&mut writer)?;
                // Write the mapping ID.
                mapping_id.write_le(&mut writer)?;
                // Write the key ID.
                key_id.write_le(&mut writer)?;
                // Write the value ID.
                value_id.write_le(&mut writer)?;
            }
            Self::UpdateKeyValue(mapping_id, key_id, value_id) => {
                // Write the variant.
                2u8.write_le(&mut writer)?;
                // Write the mapping ID.
                mapping_id.write_le(&mut writer)?;
                // Write the key ID.
                key_id.write_le(&mut writer)?;
                // Write the value ID.
                value_id.write_le(&mut writer)?;
            }
            Self::RemoveKeyValue(mapping_id, key_id) => {
                // Write the variant.
                3u8.write_le(&mut writer)?;
                // Write the mapping ID.
                mapping_id.write_le(&mut writer)?;
                // Write the key ID.
                key_id.write_le(&mut writer)?;
            }
            Self::ReplaceMapping(mapping_id) => {
                // Write the variant.
                4u8.write_le(&mut writer)?;
                // Write the mapping ID.
                mapping_id.write_le(&mut writer)?;
            }
            Self::RemoveMapping(mapping_id) => {
                // Write the variant.
                5u8.write_le(&mut writer)?;
                // Write the mapping ID.
                mapping_id.write_le(&mut writer)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() {
        for expected in crate::logic::finalize_operation::test_helpers::sample_finalize_operations() {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, FinalizeOperation::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
