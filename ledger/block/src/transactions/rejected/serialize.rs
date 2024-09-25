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

impl<N: Network> Serialize for Rejected<N> {
    /// Serializes the rejected transaction into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => match self {
                Self::Deployment(program_owner, deployment) => {
                    let mut object = serializer.serialize_struct("Rejected", 3)?;
                    object.serialize_field("type", "deployment")?;
                    object.serialize_field("program_owner", program_owner)?;
                    object.serialize_field("deployment", deployment)?;
                    object.end()
                }
                Self::Execution(execution) => {
                    let mut object = serializer.serialize_struct("Rejected", 2)?;
                    object.serialize_field("type", "execution")?;
                    object.serialize_field("execution", execution)?;
                    object.end()
                }
            },
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Rejected<N> {
    /// Deserializes the confirmed transaction from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                // Parse the rejected transaction from a string into a value.
                let mut object = serde_json::Value::deserialize(deserializer)?;

                // Parse the type.
                let type_ = object.get("type").and_then(|t| t.as_str());

                // Recover the rejected transaction.
                match type_ {
                    Some("deployment") => {
                        // Parse the program owner.
                        let program_owner: ProgramOwner<N> =
                            DeserializeExt::take_from_value::<D>(&mut object, "program_owner")?;
                        // Parse the deployment.
                        let deployment: Deployment<N> =
                            DeserializeExt::take_from_value::<D>(&mut object, "deployment")?;
                        // Return the rejected deployment.
                        Ok(Self::new_deployment(program_owner, deployment))
                    }
                    Some("execution") => {
                        // Parse the execution.
                        let execution: Execution<N> = DeserializeExt::take_from_value::<D>(&mut object, "execution")?;
                        // Return the rejected execution.
                        Ok(Self::new_execution(execution))
                    }
                    _ => Err(de::Error::custom("Invalid rejected transaction type")),
                }
            }
            false => {
                FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "confirmed transaction")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_serde_json<
        T: Serialize + for<'a> Deserialize<'a> + Debug + Display + PartialEq + Eq + FromStr + ToBytes + FromBytes,
    >(
        expected: T,
    ) {
        // Serialize
        let expected_string = expected.to_string();
        let candidate_string = serde_json::to_string(&expected).unwrap();
        let candidate = serde_json::from_str::<T>(&candidate_string).unwrap();
        assert_eq!(expected, candidate);
        assert_eq!(expected_string, candidate_string);
        assert_eq!(expected_string, candidate.to_string());

        // Deserialize
        assert_eq!(expected, T::from_str(&expected_string).unwrap_or_else(|_| panic!("FromStr: {expected_string}")));
        assert_eq!(expected, serde_json::from_str(&candidate_string).unwrap());
    }

    fn check_bincode<
        T: Serialize + for<'a> Deserialize<'a> + Debug + Display + PartialEq + Eq + FromStr + ToBytes + FromBytes,
    >(
        expected: T,
    ) {
        // Serialize
        let expected_bytes = expected.to_bytes_le().unwrap();
        let expected_bytes_with_size_encoding = bincode::serialize(&expected).unwrap();
        assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

        // Deserialize
        assert_eq!(expected, T::read_le(&expected_bytes[..]).unwrap());
        assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..]).unwrap());
    }

    #[test]
    fn test_serde_json() {
        for rejected in crate::transactions::rejected::test_helpers::sample_rejected_transactions() {
            check_serde_json(rejected);
        }
    }

    #[test]
    fn test_bincode() {
        for rejected in crate::transactions::rejected::test_helpers::sample_rejected_transactions() {
            check_bincode(rejected);
        }
    }
}
