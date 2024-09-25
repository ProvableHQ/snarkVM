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

impl<N: Network> FromBytes for Committee<N> {
    /// Reads the committee from the buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid committee version"));
        }

        // Read the committee ID.
        let id = Field::read_le(&mut reader)?;
        // Read the starting round.
        let starting_round = u64::read_le(&mut reader)?;
        // Read the number of members.
        let num_members = u16::read_le(&mut reader)?;
        // Ensure the number of members is within the allowed limit.
        if num_members > Self::MAX_COMMITTEE_SIZE {
            return Err(error(format!(
                "Committee cannot exceed {} members, found {num_members}",
                Self::MAX_COMMITTEE_SIZE,
            )));
        }

        // Calculate the number of bytes per member. Each member is a (address, stake, is_open, commission) tuple.
        let member_byte_size = Address::<N>::size_in_bytes() + 8 + 1 + 1;
        // Read the member bytes.
        let mut member_bytes = vec![0u8; num_members as usize * member_byte_size];
        reader.read_exact(&mut member_bytes)?;
        // Read the members.
        let members = cfg_chunks!(member_bytes, member_byte_size)
            .map(|mut bytes| {
                // Read the address.
                let member = Address::<N>::read_le(&mut bytes)?;
                // Read the stake.
                let stake = u64::read_le(&mut bytes)?;
                // Read the is_open flag.
                let is_open = bool::read_le(&mut bytes)?;
                // Read the commission.
                let commission = u8::read_le(&mut bytes)?;
                // Insert the member and (stake, is_open).
                Ok((member, (stake, is_open, commission)))
            })
            .collect::<Result<IndexMap<_, _>, std::io::Error>>()?;

        // Read the total stake.
        let total_stake = u64::read_le(&mut reader)?;
        // Construct the committee.
        let committee = Self::new(starting_round, members).map_err(|e| error(e.to_string()))?;
        // Ensure the committee ID matches.
        if committee.id() != id {
            return Err(error("Invalid committee ID during deserialization"));
        }
        // Ensure the total stake matches.
        match committee.total_stake() == total_stake {
            true => Ok(committee),
            false => Err(error("Invalid total stake in committee during deserialization")),
        }
    }
}

impl<N: Network> ToBytes for Committee<N> {
    /// Writes the committee to the buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        1u8.write_le(&mut writer)?;
        // Write the committee ID.
        self.id().write_le(&mut writer)?;
        // Write the starting round.
        self.starting_round.write_le(&mut writer)?;
        // Write the number of members.
        u16::try_from(self.members.len()).map_err(|e| error(e.to_string()))?.write_le(&mut writer)?;
        // Write the members.
        for (address, (stake, is_open, commission)) in &self.members {
            // Write the address.
            address.write_le(&mut writer)?;
            // Write the stake.
            stake.write_le(&mut writer)?;
            // Write the is_open flag.
            is_open.write_le(&mut writer)?;
            // Write the commission.
            commission.write_le(&mut writer)?;
        }
        // Write the total stake.
        self.total_stake.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() {
        let rng = &mut TestRng::default();

        for expected in crate::test_helpers::sample_committees(rng) {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, Committee::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
