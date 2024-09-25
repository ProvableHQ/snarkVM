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

impl<N: Network> FromBytes for Output<N> {
    /// Reads the output from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let index = Variant::read_le(&mut reader)?;
        let literal = match index {
            0 => {
                let plaintext_hash: Field<N> = FromBytes::read_le(&mut reader)?;
                let plaintext_exists: bool = FromBytes::read_le(&mut reader)?;
                let plaintext = match plaintext_exists {
                    true => Some(FromBytes::read_le(&mut reader)?),
                    false => None,
                };

                Self::Constant(plaintext_hash, plaintext)
            }
            1 => {
                let plaintext_hash: Field<N> = FromBytes::read_le(&mut reader)?;
                let plaintext_exists: bool = FromBytes::read_le(&mut reader)?;
                let plaintext = match plaintext_exists {
                    true => Some(FromBytes::read_le(&mut reader)?),
                    false => None,
                };
                Self::Public(plaintext_hash, plaintext)
            }
            2 => {
                let ciphertext_hash: Field<N> = FromBytes::read_le(&mut reader)?;
                let ciphertext_exists: bool = FromBytes::read_le(&mut reader)?;
                let ciphertext = match ciphertext_exists {
                    true => Some(FromBytes::read_le(&mut reader)?),
                    false => None,
                };
                Self::Private(ciphertext_hash, ciphertext)
            }
            3 => {
                let commitment = FromBytes::read_le(&mut reader)?;
                let checksum = FromBytes::read_le(&mut reader)?;
                let record_ciphertext_exists: bool = FromBytes::read_le(&mut reader)?;
                let record_ciphertext = match record_ciphertext_exists {
                    true => Some(FromBytes::read_le(&mut reader)?),
                    false => None,
                };

                Self::Record(commitment, checksum, record_ciphertext)
            }
            4 => {
                let commitment = FromBytes::read_le(&mut reader)?;
                Self::ExternalRecord(commitment)
            }
            5 => {
                let future_hash: Field<N> = FromBytes::read_le(&mut reader)?;
                let future_exists: bool = FromBytes::read_le(&mut reader)?;
                let future = match future_exists {
                    true => Some(FromBytes::read_le(&mut reader)?),
                    false => None,
                };
                Self::Future(future_hash, future)
            }
            6.. => return Err(error(format!("Failed to decode output variant {index}"))),
        };
        Ok(literal)
    }
}

impl<N: Network> ToBytes for Output<N> {
    /// Writes the output to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Self::Constant(plaintext_hash, plaintext) => {
                (0 as Variant).write_le(&mut writer)?;
                plaintext_hash.write_le(&mut writer)?;
                match plaintext {
                    Some(plaintext) => {
                        true.write_le(&mut writer)?;
                        plaintext.write_le(&mut writer)
                    }
                    None => false.write_le(&mut writer),
                }
            }
            Self::Public(plaintext_hash, plaintext) => {
                (1 as Variant).write_le(&mut writer)?;
                plaintext_hash.write_le(&mut writer)?;
                match plaintext {
                    Some(plaintext) => {
                        true.write_le(&mut writer)?;
                        plaintext.write_le(&mut writer)
                    }
                    None => false.write_le(&mut writer),
                }
            }
            Self::Private(ciphertext_hash, ciphertext) => {
                (2 as Variant).write_le(&mut writer)?;
                ciphertext_hash.write_le(&mut writer)?;
                match ciphertext {
                    Some(ciphertext) => {
                        true.write_le(&mut writer)?;
                        ciphertext.write_le(&mut writer)
                    }
                    None => false.write_le(&mut writer),
                }
            }
            Self::Record(commitment, checksum, record_ciphertext) => {
                (3 as Variant).write_le(&mut writer)?;
                commitment.write_le(&mut writer)?;
                checksum.write_le(&mut writer)?;
                match record_ciphertext {
                    Some(record) => {
                        true.write_le(&mut writer)?;
                        record.write_le(&mut writer)
                    }
                    None => false.write_le(&mut writer),
                }
            }
            Self::ExternalRecord(commitment) => {
                (4 as Variant).write_le(&mut writer)?;
                commitment.write_le(&mut writer)
            }
            Self::Future(future_hash, future) => {
                (5 as Variant).write_le(&mut writer)?;
                future_hash.write_le(&mut writer)?;
                match future {
                    Some(future) => {
                        true.write_le(&mut writer)?;
                        future.write_le(&mut writer)
                    }
                    None => false.write_le(&mut writer),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() {
        for (_, expected) in crate::transition::output::test_helpers::sample_outputs() {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, Output::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
