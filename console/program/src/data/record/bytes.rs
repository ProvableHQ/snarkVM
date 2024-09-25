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

impl<N: Network, Private: Visibility> FromBytes for Record<N, Private> {
    /// Reads the record from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the owner.
        let owner = Owner::read_le(&mut reader)?;
        // Read the number of entries in the record data.
        let num_entries = u8::read_le(&mut reader)?;
        // Read the record data.
        let mut data = IndexMap::with_capacity(num_entries as usize);
        for _ in 0..num_entries {
            // Read the identifier.
            let identifier = Identifier::<N>::read_le(&mut reader)?;
            // Read the entry value (in 2 steps to prevent infinite recursion).
            let num_bytes = u16::read_le(&mut reader)?;
            // Read the entry bytes.
            let mut bytes = Vec::new();
            (&mut reader).take(num_bytes as u64).read_to_end(&mut bytes)?;
            // Recover the entry value.
            let entry = Entry::read_le(&mut bytes.as_slice())?;
            // Add the entry.
            data.insert(identifier, entry);
        }
        // Read the nonce.
        let nonce = Group::read_le(&mut reader)?;

        // Prepare the reserved entry names.
        let reserved = [Identifier::from_str("owner").map_err(|e| error(e.to_string()))?];
        // Ensure the entries has no duplicate names.
        if has_duplicates(data.keys().chain(reserved.iter())) {
            return Err(error("Duplicate entry type found in record"));
        }
        // Ensure the number of entries is within the maximum limit.
        if data.len() > N::MAX_DATA_ENTRIES {
            return Err(error("Failed to parse record: too many entries"));
        }

        Ok(Self { owner, data, nonce })
    }
}

impl<N: Network, Private: Visibility> ToBytes for Record<N, Private> {
    /// Writes the record to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the owner.
        self.owner.write_le(&mut writer)?;
        // Write the number of entries in the record data.
        u8::try_from(self.data.len()).or_halt_with::<N>("Record length exceeds u8::MAX").write_le(&mut writer)?;
        // Write each entry.
        for (entry_name, entry_value) in &self.data {
            // Write the entry name.
            entry_name.write_le(&mut writer)?;
            // Write the entry value (performed in 2 steps to prevent infinite recursion).
            let bytes = entry_value.to_bytes_le().map_err(|e| error(e.to_string()))?;
            // Write the number of bytes.
            u16::try_from(bytes.len())
                .or_halt_with::<N>("Record entry exceeds u16::MAX bytes")
                .write_le(&mut writer)?;
            // Write the bytes.
            bytes.write_le(&mut writer)?;
        }
        // Write the nonce.
        self.nonce.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_bytes() -> Result<()> {
        // Construct a new record.
        let expected = Record::<CurrentNetwork, Plaintext<CurrentNetwork>>::from_str(
            "{ owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah.private, token_amount: 100u64.private, _nonce: 0group.public }",
        )?;

        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, Record::read_le(&expected_bytes[..])?);
        Ok(())
    }
}
