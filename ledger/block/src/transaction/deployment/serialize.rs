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

impl<N: Network> Serialize for Deployment<N> {
    /// Serializes the deployment into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => {
                let mut deployment = serializer.serialize_struct("Deployment", 3)?;
                deployment.serialize_field("edition", &self.edition)?;
                deployment.serialize_field("program", &self.program)?;
                deployment.serialize_field("verifying_keys", &self.verifying_keys)?;
                deployment.end()
            }
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Deployment<N> {
    /// Deserializes the deployment from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                // Parse the deployment from a string into a value.
                let mut deployment = serde_json::Value::deserialize(deserializer)?;

                // Recover the deployment.
                let deployment = Self::new(
                    // Retrieve the edition.
                    DeserializeExt::take_from_value::<D>(&mut deployment, "edition")?,
                    // Retrieve the program.
                    DeserializeExt::take_from_value::<D>(&mut deployment, "program")?,
                    // Retrieve the verifying keys.
                    DeserializeExt::take_from_value::<D>(&mut deployment, "verifying_keys")?,
                )
                .map_err(de::Error::custom)?;

                Ok(deployment)
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "deployment"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_json() -> Result<()> {
        let rng = &mut TestRng::default();

        // Sample the deployment.
        let expected = test_helpers::sample_deployment(rng);

        // Serialize
        let expected_string = &expected.to_string();
        let candidate_string = serde_json::to_string(&expected)?;
        assert_eq!(expected, serde_json::from_str(&candidate_string)?);

        // Deserialize
        assert_eq!(expected, Deployment::from_str(expected_string)?);
        assert_eq!(expected, serde_json::from_str(&candidate_string)?);

        Ok(())
    }

    #[test]
    fn test_bincode() -> Result<()> {
        let rng = &mut TestRng::default();

        // Sample the deployment.
        let expected = test_helpers::sample_deployment(rng);

        // Serialize
        let expected_bytes = expected.to_bytes_le()?;
        let expected_bytes_with_size_encoding = bincode::serialize(&expected)?;
        assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

        // Deserialize
        assert_eq!(expected, Deployment::read_le(&expected_bytes[..])?);
        assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..])?);

        Ok(())
    }
}
