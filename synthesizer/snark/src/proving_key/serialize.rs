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

impl<N: Network> Serialize for ProvingKey<N> {
    /// Serializes the proving key into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => serializer.collect_str(self),
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for ProvingKey<N> {
    /// Deserializes the proving key from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => FromStr::from_str(&String::deserialize(deserializer)?).map_err(de::Error::custom),
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "proving key"),
        }
    }
}

/// Deserializes a proving key of either version, as `ProvingKey::read_le_with_srs`
/// reads it, from the string or bytes that `ProvingKey` serializes into.
pub struct ProvingKeySeed<'a, N: Network>(pub &'a UniversalSRS<N>);

impl<'de, N: Network> de::DeserializeSeed<'de> for ProvingKeySeed<'_, N> {
    type Value = ProvingKey<N>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                ProvingKey::from_str_with_srs(&String::deserialize(deserializer)?, self.0).map_err(de::Error::custom)
            }
            false => deserializer.deserialize_bytes(self),
        }
    }
}

impl<'de, N: Network> Visitor<'de> for ProvingKeySeed<'_, N> {
    type Value = ProvingKey<N>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid proving key")
    }

    fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<Self::Value, E> {
        ProvingKey::read_le_with_srs(bytes, self.0).map_err(de::Error::custom)
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Self::Value, S::Error> {
        let mut bytes = Vec::new();
        while let Some(byte) = seq.next_element()? {
            bytes.push(byte);
        }
        self.visit_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{CurrentNetwork, sample_keys};
    use bincode::Options;
    use de::DeserializeSeed;

    #[test]
    fn test_serde_json() -> Result<()> {
        let (expected, _) = sample_keys();
        let srs = UniversalSRS::<CurrentNetwork>::load()?;

        // Serialize
        let expected_string = &expected.to_string();
        let candidate_string = serde_json::to_string(&expected)?;
        assert_eq!(expected_string, serde_json::Value::from_str(&candidate_string)?.as_str().unwrap());

        // Deserialize
        assert!(serde_json::from_str::<ProvingKey<CurrentNetwork>>(&candidate_string).is_err());
        let candidate = ProvingKeySeed(&srs).deserialize(&mut serde_json::Deserializer::from_str(&candidate_string))?;
        assert_eq!(expected_string, &candidate.to_string());

        Ok(())
    }

    #[test]
    fn test_bincode() -> Result<()> {
        let (expected, _) = sample_keys();
        let srs = UniversalSRS::<CurrentNetwork>::load()?;

        // Serialize
        let expected_bytes = expected.to_bytes_le()?;
        let expected_bytes_with_size_encoding = bincode::serialize(&expected)?;
        assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

        // Deserialize, with the options `bincode::deserialize` uses.
        assert!(bincode::deserialize::<ProvingKey<CurrentNetwork>>(&expected_bytes_with_size_encoding).is_err());
        let options = bincode::DefaultOptions::new().with_fixint_encoding().allow_trailing_bytes();
        let candidate = options.deserialize_seed(ProvingKeySeed(&srs), &expected_bytes_with_size_encoding)?;
        assert_eq!(expected_bytes, candidate.to_bytes_le()?);

        Ok(())
    }
}
