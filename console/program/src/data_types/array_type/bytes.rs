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
use crate::{Identifier, LiteralType};

impl<N: Network> FromBytes for ArrayType<N> {
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the innermost element type.
        let variant = u8::read_le(&mut reader)?;
        let element_type = match variant {
            0 => PlaintextType::Literal(LiteralType::read_le(&mut reader)?),
            1 => PlaintextType::Struct(Identifier::read_le(&mut reader)?),
            2.. => return Err(error(format!("Failed to deserialize element type {variant}"))),
        };

        // Read the number of dimensions of the array.
        let dimensions = u8::read_le(&mut reader)? as usize;

        // Ensure the dimensions of the array are valid.
        match dimensions {
            0 => return Err(error("Array type must have at least one dimension.")),
            dimensions if dimensions <= N::MAX_DATA_DEPTH => (),
            _ => return Err(error(format!("Array type exceeds the maximum depth of {}.", N::MAX_DATA_DEPTH))),
        }

        // Read the lengths of the array.
        let mut lengths = Vec::with_capacity(dimensions);
        for _ in 0..dimensions {
            lengths.push(U32::read_le(&mut reader)?);
        }

        // Construct the array type.
        ArrayType::new(element_type, lengths).map_err(|e| error(format!("{e}")))
    }
}

impl<N: Network> ToBytes for ArrayType<N> {
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Initialize the components to serialize.
        let mut element_type = *self.element_type.clone();
        let mut lengths = Vec::new();
        lengths.push(*self.length());

        // Collect the each dimension of the array.
        // Note that the lengths are in the order of the outermost dimension to the innermost dimension.
        for _ in 1..N::MAX_DATA_DEPTH {
            element_type = match element_type {
                PlaintextType::Literal(_) | PlaintextType::Struct(_) => break,
                PlaintextType::Array(array_type) => {
                    lengths.push(*array_type.length());
                    array_type.next_element_type().clone()
                }
            };
        }

        // Check that the array type does not exceed the maximum depth.
        if let PlaintextType::Array(_) = element_type {
            return Err(error(format!("Array type exceeds the maximum depth of {}.", N::MAX_DATA_DEPTH)));
        }

        // Write the innermost element type.
        match element_type {
            PlaintextType::Literal(literal_type) => {
                0u8.write_le(&mut writer)?;
                literal_type.write_le(&mut writer)?;
            }
            PlaintextType::Struct(identifier) => {
                1u8.write_le(&mut writer)?;
                identifier.write_le(&mut writer)?;
            }
            PlaintextType::Array(_) => {
                // This is technically unreachable by definition, however we return an error
                // out of an abundance of caution.
                return Err(error(format!("Array type exceeds the maximum depth of {}.", N::MAX_DATA_DEPTH)));
            }
        }

        // Write the number of dimensions of the array.
        u8::try_from(lengths.len()).map_err(error)?.write_le(&mut writer)?;

        // Write the lengths of the array.
        for length in lengths {
            length.write_le(&mut writer)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type CurrentNetwork = snarkvm_console_network::MainnetV0;

    #[test]
    fn test_array_maximum_depth() {
        // Construct the array type.
        let array_type = {
            let mut string = "[u8; 1u32]".to_string();
            for i in 1..CurrentNetwork::MAX_DATA_DEPTH {
                string = format!("[{}; {}u32]", string, i + 1);
            }
            let array_type = ArrayType::<CurrentNetwork>::from_str(&string).unwrap();
            assert_eq!(array_type.length(), &U32::new(u32::try_from(CurrentNetwork::MAX_DATA_DEPTH).unwrap()));
            assert_eq!(array_type.base_element_type(), &PlaintextType::Literal(LiteralType::U8));
            array_type
        };

        // Serialize and deserialize the array type.
        let mut bytes = Vec::new();
        array_type.write_le(&mut bytes).unwrap();
        let array_type = ArrayType::<CurrentNetwork>::read_le(&bytes[..]).unwrap();
        assert_eq!(array_type.length(), &U32::new(u32::try_from(CurrentNetwork::MAX_DATA_DEPTH).unwrap()));
        assert_eq!(array_type.base_element_type(), &PlaintextType::Literal(LiteralType::U8))
    }
}
