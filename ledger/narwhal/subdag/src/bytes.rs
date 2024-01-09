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

use super::*;

impl<N: Network> FromBytes for Subdag<N> {
    /// Reads the subdag from the buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if ![1, 2].contains(&version) {
            return Err(error(format!("Invalid subdag version ({version})")));
        }

        // Read the subdag type.
        let subdag_type = match version {
            2 => u8::read_le(&mut reader)?,
            _ => 1u8,
        };

        // Read the number of rounds.
        let num_rounds = u32::read_le(&mut reader)?;
        // Ensure the number of rounds is within bounds.
        if num_rounds as u64 > Self::MAX_ROUNDS {
            return Err(error(format!("Number of rounds ({num_rounds}) exceeds the maximum ({})", Self::MAX_ROUNDS)));
        }
        // Read the round certificates.
        let mut batch_subdag = BTreeMap::new();
        let mut compact_subdag = BTreeMap::new();
        for _ in 0..num_rounds {
            // Read the round.
            let round = u64::read_le(&mut reader)?;
            // Read the number of certificates.
            let num_certificates = u16::read_le(&mut reader)?;
            // Ensure the number of certificates is within bounds.
            if num_certificates > N::LATEST_MAX_CERTIFICATES().map_err(error)? {
                return Err(error(format!("Number of certificates ({num_certificates}) exceeds the maximum.",)));
            }
            // Read the certificates.
            match subdag_type {
                1 => {
                    let mut certificates = IndexSet::new();
                    for _ in 0..num_certificates {
                        // Read the certificate.
                        certificates.insert(BatchCertificate::read_le(&mut reader)?);
                    }
                    // Insert the round and certificates.
                    batch_subdag.insert(round, certificates);
                }
                2 => {
                    let mut certificates = IndexSet::new();
                    for _ in 0..num_certificates {
                        // Read the certificate.
                        certificates.insert(CompactCertificate::read_le(&mut reader)?);
                    }
                    // Insert the round and certificates.
                    compact_subdag.insert(round, certificates);
                }
                _ => {
                    return Err(error(format!("Found unexpected subdag type ({subdag_type})")));
                }
            }
        }

        // Return the subdag.
        match subdag_type {
            1 => Self::from_full(batch_subdag).map_err(error),
            2 => Self::from_compact(compact_subdag).map_err(error),
            _ => Err(error(format!("Found unexpected subdag type ({subdag_type})"))),
        }
    }
}

impl<N: Network> ToBytes for Subdag<N> {
    /// Writes the subdag to the buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        2u8.write_le(&mut writer)?;
        // Get the subdag type
        let (subdag_type, subdag_len) = match self {
            Self::Full { subdag, .. } => (1u8, subdag.len()),
            Self::Compact { subdag, .. } => (2u8, subdag.len()),
        };
        // Write the subdag type
        subdag_type.write_le(&mut writer)?;
        // Write the number of rounds.
        u32::try_from(subdag_len).map_err(error)?.write_le(&mut writer)?;
        // Write the round certificates.
        match self {
            Self::Full { subdag, .. } => {
                for (round, certificates) in subdag {
                    // Write the round.
                    round.write_le(&mut writer)?;
                    // Write the number of certificates.
                    u32::try_from(certificates.len()).map_err(error)?.write_le(&mut writer)?;
                    // Write the certificates.
                    for certificate in certificates {
                        // Write the certificate.
                        certificate.write_le(&mut writer)?;
                    }
                }
            }
            Self::Compact { subdag, .. } => {
                for (round, certificates) in subdag {
                    // Write the round.
                    round.write_le(&mut writer)?;
                    // Write the number of certificates.
                    u32::try_from(certificates.len()).map_err(error)?.write_le(&mut writer)?;
                    // Write the certificates.
                    for certificate in certificates {
                        // Write the certificate.
                        certificate.write_le(&mut writer)?;
                    }
                }
            }
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

        for expected in crate::test_helpers::sample_subdags(rng) {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, Subdag::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
