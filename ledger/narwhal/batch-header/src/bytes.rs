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

impl<N: Network> FromBytes for BatchHeader<N> {
    /// Reads the batch header from the buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid batch header version"));
        }

        // Read the batch ID.
        let batch_id = Field::read_le(&mut reader)?;
        // Read the author.
        let author = Address::read_le(&mut reader)?;
        // Read the round number.
        let round = u64::read_le(&mut reader)?;
        // Read the timestamp.
        let timestamp = i64::read_le(&mut reader)?;
        // Read the committee ID.
        let committee_id = Field::read_le(&mut reader)?;

        // Read the number of transmission IDs.
        let num_transmission_ids = u32::read_le(&mut reader)?;
        // Ensure the number of transmission IDs is within bounds.
        if num_transmission_ids as usize > Self::MAX_TRANSMISSIONS_PER_BATCH {
            return Err(error(format!(
                "Number of transmission IDs ({num_transmission_ids}) exceeds the maximum ({})",
                Self::MAX_TRANSMISSIONS_PER_BATCH,
            )));
        }
        // Read the transmission IDs.
        let mut transmission_ids = IndexSet::new();
        for _ in 0..num_transmission_ids {
            // Insert the transmission ID.
            transmission_ids.insert(TransmissionID::read_le(&mut reader)?);
        }

        // Read the number of previous certificate IDs.
        let num_previous_certificate_ids = u16::read_le(&mut reader)?;
        // Ensure the number of previous certificate IDs is within bounds.
        if num_previous_certificate_ids > Self::MAX_CERTIFICATES {
            return Err(error(format!(
                "Number of previous certificate IDs ({num_previous_certificate_ids}) exceeds the maximum ({})",
                Self::MAX_CERTIFICATES
            )));
        }

        // Read the previous certificate ID bytes.
        let mut previous_certificate_id_bytes =
            vec![0u8; num_previous_certificate_ids as usize * Field::<N>::size_in_bytes()];
        reader.read_exact(&mut previous_certificate_id_bytes)?;
        // Read the previous certificate IDs.
        let previous_certificate_ids = cfg_chunks!(previous_certificate_id_bytes, Field::<N>::size_in_bytes())
            .map(Field::read_le)
            .collect::<Result<IndexSet<_>, _>>()?;

        // Read the signature.
        let signature = Signature::read_le(&mut reader)?;

        // Construct the batch.
        let batch =
            Self::from(author, round, timestamp, committee_id, transmission_ids, previous_certificate_ids, signature)
                .map_err(error)?;

        // Return the batch.
        match batch.batch_id == batch_id {
            true => Ok(batch),
            false => Err(error("Invalid batch ID")),
        }
    }
}

impl<N: Network> ToBytes for BatchHeader<N> {
    /// Writes the batch header to the buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        1u8.write_le(&mut writer)?;
        // Write the batch ID.
        self.batch_id.write_le(&mut writer)?;
        // Write the author.
        self.author.write_le(&mut writer)?;
        // Write the round number.
        self.round.write_le(&mut writer)?;
        // Write the timestamp.
        self.timestamp.write_le(&mut writer)?;
        // Write the committee ID.
        self.committee_id.write_le(&mut writer)?;
        // Write the number of transmission IDs.
        u32::try_from(self.transmission_ids.len()).map_err(|e| error(e.to_string()))?.write_le(&mut writer)?;
        // Write the transmission IDs.
        for transmission_id in &self.transmission_ids {
            // Write the transmission ID.
            transmission_id.write_le(&mut writer)?;
        }
        // Write the number of previous certificate IDs.
        u16::try_from(self.previous_certificate_ids.len()).map_err(|e| error(e.to_string()))?.write_le(&mut writer)?;
        // Write the previous certificate IDs.
        for certificate_id in &self.previous_certificate_ids {
            // Write the certificate ID.
            certificate_id.write_le(&mut writer)?;
        }
        // Write the signature.
        self.signature.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() {
        let rng = &mut TestRng::default();

        for expected in crate::test_helpers::sample_batch_headers(rng) {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, BatchHeader::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
