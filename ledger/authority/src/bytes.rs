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

impl<N: Network> FromBytes for Authority<N> {
    /// Reads the authority from the buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the variant.
        let variant = u8::read_le(&mut reader)?;
        // Match the variant.
        match variant {
            0 => Ok(Self::Beacon(FromBytes::read_le(&mut reader)?)),
            1 => Ok(Self::Quorum(FromBytes::read_le(&mut reader)?)),
            2.. => Err(error("Invalid authority variant")),
        }
    }
}

impl<N: Network> ToBytes for Authority<N> {
    /// Writes the authority to the buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the authority.
        match self {
            Self::Beacon(signature) => {
                // Write the variant.
                0u8.write_le(&mut writer)?;
                // Write the signature.
                signature.write_le(&mut writer)
            }
            Self::Quorum(subdag) => {
                // Write the variant.
                1u8.write_le(&mut writer)?;
                // Write the subdag.
                subdag.write_le(&mut writer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::prelude::TestRng;

    #[test]
    fn test_bytes() {
        let rng = &mut TestRng::default();

        for expected in crate::test_helpers::sample_authorities(rng) {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, Authority::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
