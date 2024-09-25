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

impl<N: Network> FromBytes for BatchCertificate<N> {
    /// Reads the batch certificate from the buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid batch certificate version"));
        }

        // Read the batch header.
        let batch_header = BatchHeader::read_le(&mut reader)?;
        // Read the number of signatures.
        let num_signatures = u16::read_le(&mut reader)?;
        // Ensure the number of signatures is within bounds.
        if num_signatures > Self::MAX_SIGNATURES {
            return Err(error(format!(
                "Number of signatures ({num_signatures}) exceeds the maximum ({})",
                Self::MAX_SIGNATURES
            )));
        }
        // Read the signature bytes.
        let mut signature_bytes = vec![0u8; num_signatures as usize * Signature::<N>::size_in_bytes()];
        reader.read_exact(&mut signature_bytes)?;
        // Read the signatures.
        let signatures = cfg_chunks!(signature_bytes, Signature::<N>::size_in_bytes())
            .map(Signature::read_le)
            .collect::<Result<IndexSet<_>, _>>()?;
        // Return the batch certificate.
        Self::from(batch_header, signatures).map_err(error)
    }
}

impl<N: Network> ToBytes for BatchCertificate<N> {
    /// Writes the batch certificate to the buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        1u8.write_le(&mut writer)?;
        // Write the batch header.
        self.batch_header.write_le(&mut writer)?;
        // Write the number of signatures.
        u16::try_from(self.signatures.len()).map_err(error)?.write_le(&mut writer)?;
        // Write the signatures.
        for signature in self.signatures.iter() {
            // Write the signature.
            signature.write_le(&mut writer)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() {
        let rng = &mut TestRng::default();

        for expected in crate::test_helpers::sample_batch_certificates(rng) {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, BatchCertificate::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
