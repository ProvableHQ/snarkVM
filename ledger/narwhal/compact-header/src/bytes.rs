// Copyright (c) 2019-2025 Provable Inc.
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

use bit_vec::BitVec;

use super::*;

impl<N: Network> FromBytes for CompactHeader<N> {
    /// Reads the compact header from the buffer.
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

        // Read the number of transaction indices.
        let num_transaction_indices = u16::read_le(&mut reader)?;
        let mut transaction_indices = Vec::with_capacity(num_transaction_indices as usize);
        // Read the transaction indices.
        for _ in 0..num_transaction_indices {
            transaction_indices.push(u8::read_le(&mut reader)?);
        }
        let transaction_indices = BitSet::from_bit_vec(BitVec::from_bytes(&transaction_indices));

        // Read the number of solution indices.
        let num_solution_indices = u16::read_le(&mut reader)?;
        let mut solution_indices = Vec::with_capacity(num_solution_indices as usize);
        // Read the transaction indices.
        for _ in 0..num_solution_indices {
            solution_indices.push(u8::read_le(&mut reader)?);
        }
        let solution_indices = BitSet::from_bit_vec(BitVec::from_bytes(&solution_indices));

        // Read the number of previous certificate IDs.
        let num_previous_certificate_ids = u16::read_le(&mut reader)?;
        // Ensure the number of previous certificate IDs is within bounds.
        if num_previous_certificate_ids > N::LATEST_MAX_CERTIFICATES().unwrap() {
            return Err(error(format!(
                "Number of previous certificate IDs ({num_previous_certificate_ids}) exceeds the maximum ({})",
                N::LATEST_MAX_CERTIFICATES().unwrap()
            )));
        }
        // Read the previous certificate IDs.
        let mut previous_certificate_ids = IndexSet::new();
        for _ in 0..num_previous_certificate_ids {
            // Read the certificate ID.
            previous_certificate_ids.insert(Field::read_le(&mut reader)?);
        }

        // Read the signature.
        let signature = Signature::read_le(&mut reader)?;

        // Construct the batch.
        let batch = Self::from(
            batch_id,
            author,
            round,
            timestamp,
            committee_id,
            transaction_indices,
            solution_indices,
            previous_certificate_ids,
            signature,
        )
        .map_err(|e| error(e.to_string()))?;

        // Return the batch.
        match batch.batch_id == batch_id {
            true => Ok(batch),
            false => Err(error("Invalid batch ID")),
        }
    }
}

impl<N: Network> ToBytes for CompactHeader<N> {
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
        // Get transaction indices vector.
        let transaction_indices = self.transaction_indices.get_ref().to_bytes();
        // Write the number of transaction indices.
        u16::try_from(transaction_indices.len()).map_err(error)?.write_le(&mut writer)?;
        // Write the transaction indices.
        transaction_indices.into_iter().try_for_each(|b| b.write_le(&mut writer))?;
        // Get solution indices vector.
        let solution_indices = self.solution_indices.get_ref().to_bytes();
        // Write the number of solution indices.
        u16::try_from(solution_indices.len()).map_err(error)?.write_le(&mut writer)?;
        // Write the solution indices.
        solution_indices.into_iter().try_for_each(|b| b.write_le(&mut writer))?;
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

        for expected in crate::test_helpers::sample_compact_headers(rng) {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, CompactHeader::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
