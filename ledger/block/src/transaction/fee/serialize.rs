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

impl<N: Network> Serialize for Fee<N> {
    /// Serializes the fee into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => {
                let mut fee = serializer.serialize_struct("Fee", 2 + self.proof.is_some() as usize)?;
                fee.serialize_field("transition", &self.transition)?;
                fee.serialize_field("global_state_root", &self.global_state_root)?;
                if let Some(proof) = &self.proof {
                    fee.serialize_field("proof", proof)?;
                }
                fee.end()
            }
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Fee<N> {
    /// Deserializes the fee from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                // Parse the fee from a string into a value.
                let mut fee = serde_json::Value::deserialize(deserializer)?;
                // Retrieve the transitions.
                let transition = DeserializeExt::take_from_value::<D>(&mut fee, "transition")?;
                // Retrieve the global state root.
                let global_state_root = DeserializeExt::take_from_value::<D>(&mut fee, "global_state_root")?;
                // Retrieve the proof.
                let proof = DeserializeExt::take_from_value::<D>(&mut fee, "proof")?;
                // Recover the fee.
                Self::from(transition, global_state_root, proof).map_err(de::Error::custom)
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "fee"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_json() -> Result<()> {
        let rng = &mut TestRng::default();

        // Sample the private fee.
        let expected = crate::transaction::fee::test_helpers::sample_fee_private_hardcoded(rng);

        // Serialize
        let expected_string = &expected.to_string();
        let candidate_string = serde_json::to_string(&expected)?;
        assert_eq!(expected, serde_json::from_str(&candidate_string)?);

        // Deserialize
        assert_eq!(expected, Fee::from_str(expected_string)?);
        assert_eq!(expected, serde_json::from_str(&candidate_string)?);

        // Sample the public fee.
        let expected = crate::transaction::fee::test_helpers::sample_fee_public_hardcoded(rng);

        // Serialize
        let expected_string = &expected.to_string();
        let candidate_string = serde_json::to_string(&expected)?;
        assert_eq!(expected, serde_json::from_str(&candidate_string)?);

        // Deserialize
        assert_eq!(expected, Fee::from_str(expected_string)?);
        assert_eq!(expected, serde_json::from_str(&candidate_string)?);

        Ok(())
    }

    #[test]
    fn test_bincode() -> Result<()> {
        let rng = &mut TestRng::default();

        // Sample the private fee.
        let expected = crate::transaction::fee::test_helpers::sample_fee_private_hardcoded(rng);

        // Serialize
        let expected_bytes = expected.to_bytes_le()?;
        let expected_bytes_with_size_encoding = bincode::serialize(&expected)?;
        assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

        // Deserialize
        assert_eq!(expected, Fee::read_le(&expected_bytes[..])?);
        assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..])?);

        // Sample the public fee.
        let expected = crate::transaction::fee::test_helpers::sample_fee_public_hardcoded(rng);

        // Serialize
        let expected_bytes = expected.to_bytes_le()?;
        let expected_bytes_with_size_encoding = bincode::serialize(&expected)?;
        assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

        // Deserialize
        assert_eq!(expected, Fee::read_le(&expected_bytes[..])?);
        assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..])?);

        Ok(())
    }
}
