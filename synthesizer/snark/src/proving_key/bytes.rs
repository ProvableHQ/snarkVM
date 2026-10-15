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

/// Every point of the committer key is written out.
const VERSION_FULL: u8 = 1;
/// The committer key is written as a reference to the universal SRS, so the
/// key can only be read back with the SRS.
const VERSION_SRS_REFERENCE: u8 = 2;

impl<N: Network> FromBytes for ProvingKey<N> {
    /// Reads the proving key from a buffer.
    ///
    /// A proving key that refers to the universal SRS is rejected; read it
    /// with `ProvingKey::read_le_with_srs`.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version == VERSION_SRS_REFERENCE {
            return Err(error("This proving key refers to the universal SRS; read it with the SRS"));
        }
        if version != VERSION_FULL {
            return Err(error("Invalid proving key version"));
        }
        // Read the proving key.
        let proving_key = Arc::new(FromBytes::read_le(&mut reader)?);
        // Return the proving key.
        Ok(Self { proving_key })
    }
}

impl<N: Network> ToBytes for ProvingKey<N> {
    /// Writes the proving key to a buffer.
    ///
    /// A proving key whose committer key shares the universal SRS is written
    /// as a reference to the SRS; `ProvingKey::write_le_full` writes every point.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self.proving_key.committer_key.shares_srs() {
            true => {
                // Write the version.
                VERSION_SRS_REFERENCE.write_le(&mut writer)?;
                // Write the bytes.
                self.proving_key.write_le_srs_reference(&mut writer)
            }
            false => self.write_le_full(writer),
        }
    }
}

impl<N: Network> ProvingKey<N> {
    /// Writes the proving key with every point of its committer key, so that it
    /// can be read without the universal SRS.
    pub fn write_le_full<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        VERSION_FULL.write_le(&mut writer)?;
        // Write the bytes.
        self.proving_key.write_le(&mut writer)
    }

    /// Returns the proving key as bytes with every point of its committer key;
    /// see `write_le_full`.
    pub fn to_bytes_le_full(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.write_le_full(&mut bytes)?;
        Ok(bytes)
    }

    /// Reads a proving key of either version. A key that refers to the
    /// universal SRS is trimmed from `universal_srs`, and shares its points.
    pub fn read_le_with_srs<R: Read>(mut reader: R, universal_srs: &UniversalSRS<N>) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Read the proving key.
        let proving_key = match version {
            VERSION_FULL => FromBytes::read_le(&mut reader)?,
            VERSION_SRS_REFERENCE => varuna::CircuitProvingKey::read_le_srs_reference(&mut reader, universal_srs)?,
            _ => return Err(error("Invalid proving key version")),
        };
        // Return the proving key.
        Ok(Self { proving_key: Arc::new(proving_key) })
    }

    /// Reads a proving key of either version from bytes; see `read_le_with_srs`.
    pub fn from_bytes_le_with_srs(bytes: &[u8], universal_srs: &UniversalSRS<N>) -> Result<Self> {
        Ok(Self::read_le_with_srs(bytes, universal_srs)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{CurrentNetwork, sample_assignment, sample_keys};
    use circuit::environment::{Circuit, Environment};
    use snarkvm_algorithms::snark::varuna::VarunaVersion;

    #[test]
    fn test_bytes() -> Result<()> {
        // A key from circuit setup shares the universal SRS, so it is written as a reference to it.
        let (expected, verifying_key) = sample_keys();
        let expected_bytes = expected.to_bytes_le()?;
        let expected_full_bytes = expected.to_bytes_le_full()?;
        assert_eq!(expected_bytes[0], VERSION_SRS_REFERENCE);
        assert_eq!(expected_full_bytes[0], VERSION_FULL);
        assert!(expected_bytes.len() < expected_full_bytes.len());

        // Without the SRS, only the full key reads, and it is written in full again.
        assert!(ProvingKey::<CurrentNetwork>::read_le(&expected_bytes[..]).is_err());
        let full = ProvingKey::<CurrentNetwork>::read_le(&expected_full_bytes[..])?;
        assert!(!full.committer_key.shares_srs());
        assert_eq!(full.to_bytes_le()?, expected_full_bytes);

        // With the SRS, both read, as the same key.
        let srs = UniversalSRS::<CurrentNetwork>::load()?;
        let candidate = ProvingKey::from_bytes_le_with_srs(&expected_bytes, &srs)?;
        assert!(candidate.committer_key.shares_srs());
        assert_eq!(candidate.to_bytes_le()?, expected_bytes);
        assert_eq!(candidate.to_bytes_le_full()?, expected_full_bytes);
        let candidate_from_full = ProvingKey::from_bytes_le_with_srs(&expected_full_bytes, &srs)?;
        assert_eq!(candidate_from_full.to_bytes_le_full()?, expected_full_bytes);

        // A key read as a reference proves.
        let assignment = sample_assignment();
        let proof = candidate.prove("test", VarunaVersion::V2, &assignment, &mut TestRng::default())?;
        let one = <Circuit as Environment>::BaseField::one();
        assert!(verifying_key.verify("test", VarunaVersion::V2, &[one, one], &proof));

        Ok(())
    }
}
