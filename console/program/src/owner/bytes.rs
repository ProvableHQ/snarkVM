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

impl<N: Network> FromBytes for ProgramOwner<N> {
    /// Reads the program owner from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        match version {
            1 => {
                // Read the address.
                let address = Address::read_le(&mut reader)?;
                // Read the signature.
                let signature = Signature::read_le(&mut reader)?;

                // Return the program owner.
                Ok(Self::V1(ProgramOwnerV1::from(address, signature)))
            }
            2 => {
                // Read the address.
                let address = Address::read_le(&mut reader)?;
                // Read the authority.
                let authority = Address::read_le(&mut reader)?;
                // Read the signature.
                let signature = Signature::read_le(&mut reader)?;

                // Return the program owner.
                Ok(Self::V2(ProgramOwnerV2::from(address, authority, signature)))
            }
            _ => Err(error("Invalid program owner version")),
        }
    }
}

impl<N: Network> ToBytes for ProgramOwner<N> {
    /// Writes the program owner to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match &self {
            Self::V1(owner) => {
                // Write the version.
                1u8.write_le(&mut writer)?;
                // Write the address.
                owner.address.write_le(&mut writer)?;
                // Write the signature.
                owner.signature.write_le(&mut writer)
            }
            Self::V2(owner) => {
                // Write the version.
                2u8.write_le(&mut writer)?;
                // Write the address.
                owner.address.write_le(&mut writer)?;
                // Write the authority.
                owner.authority.write_le(&mut writer)?;
                // Write the signature.
                owner.signature.write_le(&mut writer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_bytes_v1() -> Result<()> {
        // Construct a new program owner.
        let expected = test_helpers::sample_program_owner_v1();

        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, ProgramOwner::read_le(&expected_bytes[..])?);
        assert!(ProgramOwner::<CurrentNetwork>::read_le(&expected_bytes[1..]).is_err());
        Ok(())
    }

    #[test]
    fn test_bytes_v2() -> Result<()> {
        // Construct a new program owner.
        let expected = test_helpers::sample_program_owner_v2();

        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, ProgramOwner::read_le(&expected_bytes[..])?);
        assert!(ProgramOwner::<CurrentNetwork>::read_le(&expected_bytes[1..]).is_err());
        Ok(())
    }
}
