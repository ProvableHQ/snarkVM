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

impl<N: Network> FromBytes for Operand<N> {
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        match u8::read_le(&mut reader)? {
            0 => Ok(Self::Literal(Literal::read_le(&mut reader)?)),
            1 => Ok(Self::Register(Register::read_le(&mut reader)?)),
            2 => Ok(Self::ProgramID(ProgramID::read_le(&mut reader)?)),
            3 => Ok(Self::Signer),
            4 => Ok(Self::Caller),
            5 => Ok(Self::BlockHeight),
            6 => Ok(Self::NetworkID),
            variant => Err(error(format!("Failed to deserialize operand variant {variant}"))),
        }
    }
}

impl<N: Network> ToBytes for Operand<N> {
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Self::Literal(literal) => {
                0u8.write_le(&mut writer)?;
                literal.write_le(&mut writer)
            }
            Self::Register(register) => {
                1u8.write_le(&mut writer)?;
                register.write_le(&mut writer)
            }
            Self::ProgramID(program_id) => {
                2u8.write_le(&mut writer)?;
                program_id.write_le(&mut writer)
            }
            Self::Signer => 3u8.write_le(&mut writer),
            Self::Caller => 4u8.write_le(&mut writer),
            Self::BlockHeight => 5u8.write_le(&mut writer),
            Self::NetworkID => 6u8.write_le(&mut writer),
        }
    }
}
