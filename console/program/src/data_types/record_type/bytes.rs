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

impl<N: Network> FromBytes for RecordType<N> {
    /// Reads a record type from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the name of the record type.
        let name = Identifier::read_le(&mut reader)?;
        // Read the visibility for the owner.
        let owner = PublicOrPrivate::read_le(&mut reader)?;

        // Read the number of entries.
        let num_entries = u16::read_le(&mut reader)?;
        // Ensure the number of entries is within the maximum limit.
        if num_entries as usize > N::MAX_DATA_ENTRIES {
            return Err(error(format!(
                "RecordType exceeds size: expected <= {}, found {num_entries}",
                N::MAX_DATA_ENTRIES
            )));
        }
        // Read the entries.
        let mut entries = IndexMap::with_capacity(num_entries as usize);
        for _ in 0..num_entries {
            // Read the identifier.
            let identifier = Identifier::read_le(&mut reader)?;
            // Read the entry type.
            let entry = EntryType::read_le(&mut reader)?;
            // Insert the entry, and ensure the entries has no duplicate names.
            if entries.insert(identifier, entry).is_some() {
                return Err(error(format!("Duplicate identifier in record '{name}'")));
            };
        }

        // Prepare the reserved entry names.
        let reserved = [Identifier::from_str("owner").map_err(|e| error(e.to_string()))?];
        // Ensure the entries has no duplicate names.
        if has_duplicates(entries.iter().map(|(identifier, _)| identifier).chain(reserved.iter())) {
            return Err(error(format!("Duplicate entry type found in record '{name}'")));
        }
        // Ensure the number of members is within the maximum limit.
        if entries.len() > N::MAX_DATA_ENTRIES {
            return Err(error("Failed to parse record: too many entries"));
        }

        Ok(Self { name, owner, entries })
    }
}

impl<N: Network> ToBytes for RecordType<N> {
    /// Writes the record type to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Ensure the number of entries is within the maximum limit.
        if self.entries.len() > N::MAX_DATA_ENTRIES {
            return Err(error("Failed to serialize record: too many entries"));
        }

        // Write the name of the record type.
        self.name.write_le(&mut writer)?;
        // Write the visibility for the owner.
        self.owner.write_le(&mut writer)?;

        // Write the number of entries.
        u16::try_from(self.entries.len()).or_halt_with::<N>("Record length exceeds u16").write_le(&mut writer)?;
        // Write the entries as bytes.
        for (identifier, value_type) in &self.entries {
            // Write the identifier.
            identifier.write_le(&mut writer)?;
            // Write the value type to the buffer.
            value_type.write_le(&mut writer)?;
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
        let expected = RecordType::<CurrentNetwork>::from_str(
            "record message:\n    owner as address.public;\n    first as field.constant;\n    second as field.public;",
        )?;
        let candidate = RecordType::from_bytes_le(&expected.to_bytes_le().unwrap()).unwrap();
        assert_eq!(expected, candidate);
        Ok(())
    }
}
