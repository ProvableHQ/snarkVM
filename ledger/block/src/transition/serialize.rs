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

impl<N: Network> Serialize for Transition<N> {
    /// Serializes the transition into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => {
                let mut transition = serializer.serialize_struct("Transition", 8)?;
                transition.serialize_field("id", &self.id)?;
                transition.serialize_field("program", &self.program_id)?;
                transition.serialize_field("function", &self.function_name)?;
                transition.serialize_field("inputs", &self.inputs)?;
                transition.serialize_field("outputs", &self.outputs)?;
                transition.serialize_field("tpk", &self.tpk)?;
                transition.serialize_field("tcm", &self.tcm)?;
                transition.serialize_field("scm", &self.scm)?;
                transition.end()
            }
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Transition<N> {
    /// Deserializes the transition from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                // Parse the transition from a string into a value.
                let mut transition = serde_json::Value::deserialize(deserializer)?;
                // Retrieve the ID.
                let id: N::TransitionID = DeserializeExt::take_from_value::<D>(&mut transition, "id")?;

                // Recover the transition.
                let transition = Self::new(
                    // Retrieve the program ID.
                    DeserializeExt::take_from_value::<D>(&mut transition, "program")?,
                    // Retrieve the function name.
                    DeserializeExt::take_from_value::<D>(&mut transition, "function")?,
                    // Retrieve the inputs.
                    DeserializeExt::take_from_value::<D>(&mut transition, "inputs")?,
                    // Retrieve the outputs.
                    DeserializeExt::take_from_value::<D>(&mut transition, "outputs")?,
                    // Retrieve the `tpk`.
                    DeserializeExt::take_from_value::<D>(&mut transition, "tpk")?,
                    // Retrieve the `tcm`.
                    DeserializeExt::take_from_value::<D>(&mut transition, "tcm")?,
                    // Retrieve the `scm`.
                    DeserializeExt::take_from_value::<D>(&mut transition, "scm")?,
                )
                .map_err(de::Error::custom)?;

                // Ensure the transition ID is correct.
                match id == *transition.id() {
                    true => Ok(transition),
                    false => Err(de::Error::custom(error("Transition ID mismatch, possible data corruption"))),
                }
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "transition"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_json() -> Result<()> {
        let rng = &mut TestRng::default();

        // Sample the transition.
        let expected = crate::transition::test_helpers::sample_transition(rng);

        // Serialize
        let expected_string = &expected.to_string();
        let candidate_string = serde_json::to_string(&expected)?;
        assert_eq!(expected, serde_json::from_str(&candidate_string)?);

        // Deserialize
        assert_eq!(expected, Transition::from_str(expected_string)?);
        assert_eq!(expected, serde_json::from_str(&candidate_string)?);

        Ok(())
    }

    #[test]
    fn test_bincode() -> Result<()> {
        let rng = &mut TestRng::default();

        // Sample the transition.
        let expected = crate::transition::test_helpers::sample_transition(rng);

        // Serialize
        let expected_bytes = expected.to_bytes_le()?;
        let expected_bytes_with_size_encoding = bincode::serialize(&expected)?;
        assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

        // Deserialize
        assert_eq!(expected, Transition::read_le(&expected_bytes[..])?);
        assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..])?);

        Ok(())
    }
}
