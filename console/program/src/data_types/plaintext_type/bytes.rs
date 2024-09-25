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

impl<N: Network> FromBytes for PlaintextType<N> {
    /// Reads a plaintext type from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let variant = u8::read_le(&mut reader)?;
        match variant {
            0 => Ok(Self::Literal(LiteralType::read_le(&mut reader)?)),
            1 => Ok(Self::Struct(Identifier::read_le(&mut reader)?)),
            2 => Ok(Self::Array(ArrayType::read_le(&mut reader)?)),
            3.. => Err(error(format!("Failed to deserialize annotation variant {variant}"))),
        }
    }
}

impl<N: Network> ToBytes for PlaintextType<N> {
    /// Writes a plaintext type to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Self::Literal(literal_type) => {
                0u8.write_le(&mut writer)?;
                literal_type.write_le(&mut writer)
            }
            Self::Struct(identifier) => {
                1u8.write_le(&mut writer)?;
                identifier.write_le(&mut writer)
            }
            Self::Array(array_type) => {
                2u8.write_le(&mut writer)?;
                array_type.write_le(&mut writer)
            }
        }
    }
}
