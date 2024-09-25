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

impl<N: Network, Private: Visibility> FromBytes for Entry<N, Private> {
    /// Reads the entry from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the index.
        let index = u8::read_le(&mut reader)?;
        // Read the entry.
        let entry = match index {
            0 => Self::Constant(Plaintext::read_le(&mut reader)?),
            1 => Self::Public(Plaintext::read_le(&mut reader)?),
            2 => Self::Private(Private::read_le(&mut reader)?),
            3.. => return Err(error(format!("Failed to decode entry variant {index}"))),
        };
        Ok(entry)
    }
}

impl<N: Network, Private: Visibility> ToBytes for Entry<N, Private> {
    /// Writes the entry to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Self::Constant(plaintext) => {
                0u8.write_le(&mut writer)?;
                plaintext.write_le(&mut writer)
            }
            Self::Public(plaintext) => {
                1u8.write_le(&mut writer)?;
                plaintext.write_le(&mut writer)
            }
            Self::Private(private) => {
                2u8.write_le(&mut writer)?;
                private.write_le(&mut writer)
            }
        }
    }
}
