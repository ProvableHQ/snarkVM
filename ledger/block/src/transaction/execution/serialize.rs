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

impl<N: Network> Serialize for Execution<N> {
    /// Serializes the execution into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => {
                let mut execution = serializer.serialize_struct("Execution", 2 + self.proof.is_some() as usize)?;
                execution
                    .serialize_field("transitions", &self.transitions.values().collect::<Vec<&Transition<N>>>())?;
                execution.serialize_field("global_state_root", &self.global_state_root)?;
                if let Some(proof) = &self.proof {
                    execution.serialize_field("proof", proof)?;
                }
                execution.end()
            }
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Execution<N> {
    /// Deserializes the execution from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                // Parse the execution from a string into a value.
                let mut execution = serde_json::Value::deserialize(deserializer)?;
                // Retrieve the transitions.
                let transitions: Vec<_> = DeserializeExt::take_from_value::<D>(&mut execution, "transitions")?;
                // Retrieve the global state root.
                let global_state_root = DeserializeExt::take_from_value::<D>(&mut execution, "global_state_root")?;
                // Retrieve the proof.
                let proof =
                    serde_json::from_value(execution.get_mut("proof").unwrap_or(&mut serde_json::Value::Null).take())
                        .map_err(de::Error::custom)?;
                // Recover the execution.
                Self::from(transitions.into_iter(), global_state_root, proof).map_err(de::Error::custom)
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "execution"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_json() -> Result<()> {
        let rng = &mut TestRng::default();

        // Sample the execution.
        let expected = crate::transaction::execution::test_helpers::sample_execution(rng);

        // Serialize
        let expected_string = &expected.to_string();
        let candidate_string = serde_json::to_string(&expected)?;
        assert_eq!(expected, serde_json::from_str(&candidate_string)?);

        // Deserialize
        assert_eq!(expected, Execution::from_str(expected_string)?);
        assert_eq!(expected, serde_json::from_str(&candidate_string)?);

        Ok(())
    }

    #[test]
    fn test_bincode() -> Result<()> {
        let rng = &mut TestRng::default();

        // Sample the execution.
        let expected = crate::transaction::execution::test_helpers::sample_execution(rng);

        // Serialize
        let expected_bytes = expected.to_bytes_le()?;
        let expected_bytes_with_size_encoding = bincode::serialize(&expected)?;
        assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

        // Deserialize
        assert_eq!(expected, Execution::read_le(&expected_bytes[..])?);
        assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..])?);

        Ok(())
    }
}
