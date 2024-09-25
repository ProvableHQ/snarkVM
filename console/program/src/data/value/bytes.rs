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

impl<N: Network> FromBytes for Value<N> {
    /// Reads the entry from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the index.
        let index = u8::read_le(&mut reader)?;
        // Read the entry.
        let entry = match index {
            0 => Self::Plaintext(Plaintext::read_le(&mut reader)?),
            1 => Self::Record(Record::read_le(&mut reader)?),
            2 => Self::Future(Future::read_le(&mut reader)?),
            3.. => return Err(error(format!("Failed to decode value variant {index}"))),
        };
        Ok(entry)
    }
}

impl<N: Network> ToBytes for Value<N> {
    /// Writes the entry to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Self::Plaintext(plaintext) => {
                0u8.write_le(&mut writer)?;
                plaintext.write_le(&mut writer)
            }
            Self::Record(record) => {
                1u8.write_le(&mut writer)?;
                record.write_le(&mut writer)
            }
            Self::Future(future) => {
                2u8.write_le(&mut writer)?;
                future.write_le(&mut writer)
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
    fn test_value_plaintext_bytes() -> Result<()> {
        // Construct a new plaintext value.
        let expected = Value::Plaintext(Plaintext::<CurrentNetwork>::from_str(
            "{ owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah, token_amount: 100u64 }",
        )?);

        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, Value::read_le(&expected_bytes[..])?);
        Ok(())
    }

    #[test]
    fn test_value_record_bytes() -> Result<()> {
        // Construct a new record value.
        let expected = Value::Record(Record::<CurrentNetwork, Plaintext<CurrentNetwork>>::from_str(
            "{ owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah.private, token_amount: 100u64.private, _nonce: 0group.public }",
        )?);

        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, Value::read_le(&expected_bytes[..])?);
        Ok(())
    }
}
