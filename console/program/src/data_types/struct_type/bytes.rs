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

impl<N: Network> FromBytes for StructType<N> {
    /// Reads a struct type from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the name of the struct type.
        let name = Identifier::read_le(&mut reader)?;

        // Read the number of members.
        let num_members = u16::read_le(&mut reader)?;
        // Ensure the number of members is within the maximum limit.
        if num_members as usize > N::MAX_STRUCT_ENTRIES {
            return Err(error(format!(
                "StructType exceeds size: expected <= {}, found {num_members}",
                N::MAX_STRUCT_ENTRIES
            )));
        }
        // Read the members.
        let mut members = IndexMap::with_capacity(num_members as usize);
        for _ in 0..num_members {
            // Read the identifier.
            let identifier = Identifier::read_le(&mut reader)?;
            // Read the plaintext type.
            let plaintext_type = PlaintextType::read_le(&mut reader)?;
            // Insert the member, and ensure the member has no duplicate names.
            if members.insert(identifier, plaintext_type).is_some() {
                return Err(error(format!("Duplicate identifier in struct '{name}'")));
            };
        }

        Ok(Self { name, members })
    }
}

impl<N: Network> ToBytes for StructType<N> {
    /// Writes the struct type to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Ensure the number of members is within the maximum limit.
        if self.members.len() > N::MAX_STRUCT_ENTRIES {
            return Err(error("Failed to serialize struct: too many members"));
        }

        // Write the name of the struct.
        self.name.write_le(&mut writer)?;

        // Write the number of members.
        u16::try_from(self.members.len()).or_halt_with::<N>("Struct length exceeds u16").write_le(&mut writer)?;
        // Write the members as bytes.
        for (identifier, plaintext_type) in &self.members {
            // Write the identifier.
            identifier.write_le(&mut writer)?;
            // Write the plaintext type to the buffer.
            plaintext_type.write_le(&mut writer)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_bytes() -> Result<()> {
        let expected =
            StructType::<CurrentNetwork>::from_str("struct message:\n    first as field;\n    second as field;")?;
        let candidate = StructType::from_bytes_le(&expected.to_bytes_le().unwrap()).unwrap();
        assert_eq!(expected, candidate);
        Ok(())
    }
}
