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

impl<N: Network> Serialize for Metadata<N> {
    /// Serializes the metadata to a JSON-string or buffer.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => {
                let mut metadata = serializer.serialize_struct("Metadata", 10)?;
                metadata.serialize_field("network", &self.network)?;
                metadata.serialize_field("round", &self.round)?;
                metadata.serialize_field("height", &self.height)?;
                metadata.serialize_field("cumulative_weight", &self.cumulative_weight.to_string())?;
                metadata.serialize_field("cumulative_proof_target", &self.cumulative_proof_target.to_string())?;
                metadata.serialize_field("coinbase_target", &self.coinbase_target)?;
                metadata.serialize_field("proof_target", &self.proof_target)?;
                metadata.serialize_field("last_coinbase_target", &self.last_coinbase_target)?;
                metadata.serialize_field("last_coinbase_timestamp", &self.last_coinbase_timestamp)?;
                metadata.serialize_field("timestamp", &self.timestamp)?;
                metadata.end()
            }
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Metadata<N> {
    /// Deserializes the metadata from a JSON-string or buffer.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                let mut metadata = serde_json::Value::deserialize(deserializer)?;
                let cumulative_weight: String =
                    DeserializeExt::take_from_value::<D>(&mut metadata, "cumulative_weight")?;
                let cumulative_proof_target: String =
                    DeserializeExt::take_from_value::<D>(&mut metadata, "cumulative_proof_target")?;
                let cumulative_weight = cumulative_weight.parse::<u128>().map_err(de::Error::custom)?;
                let cumulative_proof_target = cumulative_proof_target.parse::<u128>().map_err(de::Error::custom)?;
                Ok(Self::new(
                    DeserializeExt::take_from_value::<D>(&mut metadata, "network")?,
                    DeserializeExt::take_from_value::<D>(&mut metadata, "round")?,
                    DeserializeExt::take_from_value::<D>(&mut metadata, "height")?,
                    cumulative_weight,
                    cumulative_proof_target,
                    DeserializeExt::take_from_value::<D>(&mut metadata, "coinbase_target")?,
                    DeserializeExt::take_from_value::<D>(&mut metadata, "proof_target")?,
                    DeserializeExt::take_from_value::<D>(&mut metadata, "last_coinbase_target")?,
                    DeserializeExt::take_from_value::<D>(&mut metadata, "last_coinbase_timestamp")?,
                    DeserializeExt::take_from_value::<D>(&mut metadata, "timestamp")?,
                )
                .map_err(de::Error::custom)?)
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "metadata"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_json() -> Result<()> {
        let rng = &mut TestRng::default();

        for expected in [crate::header::metadata::test_helpers::sample_block_metadata(rng)].into_iter() {
            // Serialize
            let expected_string = &expected.to_string();
            let candidate_string = serde_json::to_string(&expected)?;

            // Deserialize
            assert_eq!(expected, Metadata::from_str(expected_string)?);
            assert_eq!(expected, serde_json::from_str(&candidate_string)?);
        }
        Ok(())
    }

    #[test]
    fn test_bincode() -> Result<()> {
        let rng = &mut TestRng::default();

        for expected in [crate::header::metadata::test_helpers::sample_block_metadata(rng)].into_iter() {
            // Serialize
            let expected_bytes = expected.to_bytes_le()?;
            let expected_bytes_with_size_encoding = bincode::serialize(&expected)?;
            assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

            // Deserialize
            assert_eq!(expected, Metadata::read_le(&expected_bytes[..])?);
            assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..])?);
        }
        Ok(())
    }
}
