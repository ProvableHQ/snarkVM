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

impl<N: Network> FromBytes for Access<N> {
    /// Reads the access from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let variant = u8::read_le(&mut reader)?;
        match variant {
            0 => Ok(Self::Member(Identifier::read_le(&mut reader)?)),
            1 => Ok(Self::Index(U32::read_le(&mut reader)?)),
            2.. => Err(error(format!("Failed to deserialize access variant {variant}"))),
        }
    }
}

impl<N: Network> ToBytes for Access<N> {
    /// Write the access to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Access::Member(identifier) => {
                0u8.write_le(&mut writer)?;
                identifier.write_le(&mut writer)
            }
            Access::Index(index) => {
                1u8.write_le(&mut writer)?;
                index.write_le(&mut writer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::identifier::tests::sample_identifier;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u32 = 1000;

    fn check_bytes(expected: Access<CurrentNetwork>) -> Result<()> {
        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, Access::read_le(&expected_bytes[..])?);
        Ok(())
    }

    #[test]
    fn test_bytes() -> Result<()> {
        let rng = &mut TestRng::default();

        for _ in 0..ITERATIONS {
            // Member
            let identifier = sample_identifier(rng)?;
            check_bytes(Access::Member(identifier))?;

            // Index
            let index = U32::<CurrentNetwork>::rand(rng);
            check_bytes(Access::Index(index))?;
        }
        Ok(())
    }
}
