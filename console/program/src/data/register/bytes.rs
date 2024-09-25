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

impl<N: Network> FromBytes for Register<N> {
    /// Reads the register from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let variant = u8::read_le(&mut reader)?;
        let locator = read_variable_length_integer(&mut reader)?;
        match variant {
            0 => Ok(Self::Locator(locator)),
            1 => {
                // Read the number of accesses.
                let num_accesses = u16::read_le(&mut reader)?;
                if num_accesses as usize > N::MAX_DATA_DEPTH {
                    return Err(error("Failed to deserialize register: Register access exceeds maximum depth"));
                }
                // Read the accesses.
                let accesses = (0..num_accesses).map(|_| Access::read_le(&mut reader)).collect::<IoResult<Vec<_>>>()?;
                Ok(Self::Access(locator, accesses))
            }
            2.. => Err(error(format!("Failed to deserialize register variant {variant}"))),
        }
    }
}

impl<N: Network> ToBytes for Register<N> {
    /// Writes the register to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Self::Locator(locator) => {
                u8::write_le(&0u8, &mut writer)?;
                variable_length_integer(locator).write_le(&mut writer)
            }
            Self::Access(locator, accesses) => {
                // Ensure the number of accesses is within the limit.
                if accesses.len() > N::MAX_DATA_DEPTH {
                    return Err(error("Failed to serialize register: too many accesses"));
                }

                u8::write_le(&1u8, &mut writer)?;
                variable_length_integer(locator).write_le(&mut writer)?;
                u16::try_from(accesses.len()).map_err(error)?.write_le(&mut writer)?;
                accesses.write_le(&mut writer)
            }
        }
    }
}
