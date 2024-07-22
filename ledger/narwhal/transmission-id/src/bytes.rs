// Copyright (C) 2019-2023 Aleo Systems Inc.
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

impl<N: Network> FromBytes for TransmissionID<N> {
    /// Reads the transmission ID from the buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the variant.
        let variant = u8::read_le(&mut reader)?;
        // Match the variant.
        match variant {
            0 => Ok(Self::Ratification),
            1 => Ok(Self::Solution(FromBytes::read_le(&mut reader)?, FromBytes::read_le(&mut reader)?)),
            2 => Ok(Self::Transaction(FromBytes::read_le(&mut reader)?, FromBytes::read_le(&mut reader)?)),
            3.. => Err(error("Invalid worker transmission ID variant")),
        }
    }
}

impl<N: Network> ToBytes for TransmissionID<N> {
    /// Writes the transmission ID to the buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the transmission.
        match self {
            Self::Ratification => 0u8.write_le(&mut writer),
            Self::Solution(id, checksum) => {
                1u8.write_le(&mut writer)?;
                id.write_le(&mut writer)?;
                checksum.write_le(&mut writer)
            }
            Self::Transaction(id, checksum) => {
                2u8.write_le(&mut writer)?;
                id.write_le(&mut writer)?;
                checksum.write_le(&mut writer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() {
        let rng = &mut TestRng::default();

        for expected in crate::test_helpers::sample_transmission_ids(rng) {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, TransmissionID::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
