// Copyright (c) 2019-2026 Provable Inc.
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

/// Writes a string to a writer with a `u16` length prefix.
fn write_string<W: Write>(s: &str, writer: &mut W) -> IoResult<()> {
    // Write the number of bytes. Use `&mut *writer` to reborrow rather than move.
    u16::try_from(s.len()).map_err(|_| error("String exceeds u16::MAX bytes"))?.write_le(&mut *writer)?;
    // Write the string bytes.
    writer.write_all(s.as_bytes())
}

/// Reads a string from a reader with a `u16` length prefix.
fn read_string<R: Read>(reader: &mut R) -> IoResult<String> {
    // Read the number of bytes. Use `&mut *reader` to reborrow rather than move.
    let num_bytes = u16::read_le(&mut *reader)? as usize;
    // Read the string bytes.
    let mut bytes = vec![0u8; num_bytes];
    reader.read_exact(&mut bytes)?;
    // Decode the UTF-8 string.
    String::from_utf8(bytes).map_err(|e| error(format!("Invalid UTF-8 string: {e}")))
}

impl FromBytes for RejectedReason {
    /// Reads the rejected reason from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the variant.
        let variant = u8::read_le(&mut reader)?;
        match variant {
            // DuplicateProgramID: locator string.
            0 => {
                let locator = read_string(&mut reader)?;
                Ok(Self::DuplicateProgramID(locator))
            }
            // Finalize: locator string, command index (u32), command string.
            1 => {
                let locator = read_string(&mut reader)?;
                let index = u32::read_le(&mut reader)? as usize;
                let command = read_string(&mut reader)?;
                Ok(Self::Finalize(locator, index, command))
            }
            // NonFinalize: locator string.
            2 => {
                let locator = read_string(&mut reader)?;
                Ok(Self::NonFinalize(locator))
            }
            3.. => Err(error(format!("Failed to decode rejected reason variant {variant}"))),
        }
    }
}

impl ToBytes for RejectedReason {
    /// Writes the rejected reason to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            // Write variant 0, then the locator.
            Self::DuplicateProgramID(locator) => {
                0u8.write_le(&mut writer)?;
                write_string(locator, &mut writer)
            }
            // Write variant 1, then locator, index (u32), and command.
            Self::Finalize(locator, index, command) => {
                1u8.write_le(&mut writer)?;
                write_string(locator, &mut writer)?;
                u32::try_from(*index).map_err(|_| error("Command index exceeds u32::MAX"))?.write_le(&mut writer)?;
                write_string(command, &mut writer)
            }
            // Write variant 2, then the locator.
            Self::NonFinalize(locator) => {
                2u8.write_le(&mut writer)?;
                write_string(locator, &mut writer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() {
        for expected in crate::transactions::rejected_reason::test_helpers::sample_rejected_reasons() {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, RejectedReason::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
