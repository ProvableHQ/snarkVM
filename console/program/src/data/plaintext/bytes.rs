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

impl<N: Network> FromBytes for Plaintext<N> {
    /// Reads the plaintext from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the index.
        let index = u8::read_le(&mut reader)?;
        // Read the plaintext.
        let plaintext = match index {
            0 => Self::Literal(Literal::read_le(&mut reader)?, Default::default()),
            1 => {
                // Read the number of members in the struct.
                let num_members = u8::read_le(&mut reader)?;
                // Read the members.
                let mut members = IndexMap::with_capacity(num_members as usize);
                for _ in 0..num_members {
                    // Read the identifier.
                    let identifier = Identifier::<N>::read_le(&mut reader)?;
                    // Read the plaintext value (in 2 steps to prevent infinite recursion).
                    let num_bytes = u16::read_le(&mut reader)?;
                    // Read the plaintext bytes.
                    let mut bytes = Vec::new();
                    (&mut reader).take(num_bytes as u64).read_to_end(&mut bytes)?;
                    // Recover the plaintext value.
                    let plaintext = Plaintext::read_le(&mut bytes.as_slice())?;
                    // Add the member.
                    members.insert(identifier, plaintext);
                }
                // Return the struct.
                Self::Struct(members, Default::default())
            }
            2 => {
                // Read the length of the array.
                let num_elements = u32::read_le(&mut reader)?;
                if num_elements as usize > N::MAX_ARRAY_ELEMENTS {
                    return Err(error("Failed to deserialize plaintext: Array exceeds maximum length"));
                }
                // Read the elements.
                let mut elements = Vec::with_capacity(num_elements as usize);
                for _ in 0..num_elements {
                    // Read the plaintext value (in 2 steps to prevent infinite recursion).
                    let num_bytes = u16::read_le(&mut reader)?;
                    // Read the plaintext bytes.
                    let mut bytes = Vec::new();
                    (&mut reader).take(num_bytes as u64).read_to_end(&mut bytes)?;
                    // Recover the plaintext value.
                    let plaintext = Plaintext::read_le(&mut bytes.as_slice())?;
                    // Add the element.
                    elements.push(plaintext);
                }
                // Return the array.
                Self::Array(elements, Default::default())
            }
            3.. => return Err(error(format!("Failed to decode plaintext variant {index}"))),
        };
        Ok(plaintext)
    }
}

impl<N: Network> ToBytes for Plaintext<N> {
    /// Writes the plaintext to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Self::Literal(literal, ..) => {
                0u8.write_le(&mut writer)?;
                literal.write_le(&mut writer)
            }
            Self::Struct(struct_, ..) => {
                1u8.write_le(&mut writer)?;

                // Write the number of members in the struct.
                u8::try_from(struct_.len()).map_err(error)?.write_le(&mut writer)?;

                // Write each member.
                for (member_name, member_value) in struct_ {
                    // Write the member name.
                    member_name.write_le(&mut writer)?;

                    // Write the member value (performed in 2 steps to prevent infinite recursion).
                    let bytes = member_value.to_bytes_le().map_err(|e| error(e.to_string()))?;
                    // Write the number of bytes.
                    u16::try_from(bytes.len()).map_err(error)?.write_le(&mut writer)?;
                    // Write the bytes.
                    bytes.write_le(&mut writer)?;
                }
                Ok(())
            }
            Self::Array(array, ..) => {
                2u8.write_le(&mut writer)?;

                // Write the length of the array.
                u32::try_from(array.len()).map_err(error)?.write_le(&mut writer)?;

                // Write each element.
                for element in array {
                    // Write the element (performed in 2 steps to prevent infinite recursion).
                    let bytes = element.to_bytes_le().map_err(error)?;
                    // Write the number of bytes.
                    u16::try_from(bytes.len()).map_err(error)?.write_le(&mut writer)?;
                    // Write the bytes.
                    bytes.write_le(&mut writer)?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u32 = 1000;

    fn check_bytes(expected: Plaintext<CurrentNetwork>) -> Result<()> {
        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, Plaintext::read_le(&expected_bytes[..])?);
        Ok(())
    }

    #[test]
    fn test_bytes() -> Result<()> {
        let rng = &mut TestRng::default();

        for _ in 0..ITERATIONS {
            let private_key = snarkvm_console_account::PrivateKey::<CurrentNetwork>::new(rng)?;

            // Address
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::Address(Address::try_from(private_key)?),
                Default::default(),
            ))?;
            // Boolean
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::Boolean(Boolean::new(Uniform::rand(rng))),
                Default::default(),
            ))?;
            // Field
            check_bytes(Plaintext::Literal(Literal::<CurrentNetwork>::Field(Uniform::rand(rng)), Default::default()))?;
            // Group
            check_bytes(Plaintext::Literal(Literal::<CurrentNetwork>::Group(Uniform::rand(rng)), Default::default()))?;
            // I8
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::I8(I8::new(Uniform::rand(rng))),
                Default::default(),
            ))?;
            // I16
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::I16(I16::new(Uniform::rand(rng))),
                Default::default(),
            ))?;
            // I32
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::I32(I32::new(Uniform::rand(rng))),
                Default::default(),
            ))?;
            // I64
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::I64(I64::new(Uniform::rand(rng))),
                Default::default(),
            ))?;
            // I128
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::I128(I128::new(Uniform::rand(rng))),
                Default::default(),
            ))?;
            // U8
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::U8(U8::new(Uniform::rand(rng))),
                Default::default(),
            ))?;
            // U16
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::U16(U16::new(Uniform::rand(rng))),
                Default::default(),
            ))?;
            // U32
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::U32(U32::new(Uniform::rand(rng))),
                Default::default(),
            ))?;
            // U64
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::U64(U64::new(Uniform::rand(rng))),
                Default::default(),
            ))?;
            // U128
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::U128(U128::new(Uniform::rand(rng))),
                Default::default(),
            ))?;
            // Scalar
            check_bytes(Plaintext::Literal(Literal::<CurrentNetwork>::Scalar(Uniform::rand(rng)), Default::default()))?;
            // String
            check_bytes(Plaintext::Literal(
                Literal::<CurrentNetwork>::String(StringType::rand(rng)),
                Default::default(),
            ))?;
        }

        // Check the struct manually.
        let expected = Plaintext::<CurrentNetwork>::from_str(
            "{ owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah, token_amount: 100u64 }",
        )?;

        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, Plaintext::read_le(&expected_bytes[..])?);

        // Check the array manually.
        let expected = Plaintext::<CurrentNetwork>::from_str("[ 1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8, 8u8, 9u8, 10u8 ]")?;

        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, Plaintext::read_le(&expected_bytes[..])?);

        Ok(())
    }
}
