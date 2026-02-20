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

impl Serialize for RejectedReason {
    /// Serializes the rejected reason into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => match self {
                Self::DuplicateProgramID(locator) => {
                    let mut object = serializer.serialize_struct("RejectedReason", 2)?;
                    object.serialize_field("type", "duplicate_program_id")?;
                    object.serialize_field("locator", locator)?;
                    object.end()
                }
                Self::Finalize(locator, index, command) => {
                    let mut object = serializer.serialize_struct("RejectedReason", 4)?;
                    object.serialize_field("type", "finalize")?;
                    object.serialize_field("locator", locator)?;
                    object.serialize_field("index", index)?;
                    object.serialize_field("command", command)?;
                    object.end()
                }
                Self::NonFinalize(locator) => {
                    let mut object = serializer.serialize_struct("RejectedReason", 2)?;
                    object.serialize_field("type", "non_finalize")?;
                    object.serialize_field("locator", locator)?;
                    object.end()
                }
            },
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de> Deserialize<'de> for RejectedReason {
    /// Deserializes the rejected reason from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                // Parse the rejected reason from a string into a value.
                let mut object = serde_json::Value::deserialize(deserializer)?;
                // Parse the type.
                let type_ = object.get("type").and_then(|t| t.as_str());
                // Recover the rejected reason.
                match type_ {
                    Some("duplicate_program_id") => {
                        let locator: String = DeserializeExt::take_from_value::<D>(&mut object, "locator")?;
                        Ok(Self::DuplicateProgramID(locator))
                    }
                    Some("finalize") => {
                        let locator: String = DeserializeExt::take_from_value::<D>(&mut object, "locator")?;
                        let index: usize = DeserializeExt::take_from_value::<D>(&mut object, "index")?;
                        let command: String = DeserializeExt::take_from_value::<D>(&mut object, "command")?;
                        Ok(Self::Finalize(locator, index, command))
                    }
                    Some("non_finalize") => {
                        let locator: String = DeserializeExt::take_from_value::<D>(&mut object, "locator")?;
                        Ok(Self::NonFinalize(locator))
                    }
                    _ => Err(de::Error::custom("Invalid rejected reason type")),
                }
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "rejected reason"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_serde_json(expected: RejectedReason) {
        // Serialize
        let expected_string = expected.to_string();
        let candidate_string = serde_json::to_string(&expected).unwrap();
        let candidate = serde_json::from_str::<RejectedReason>(&candidate_string).unwrap();
        assert_eq!(expected, candidate);
        assert_eq!(expected_string, candidate_string);
        assert_eq!(expected_string, candidate.to_string());

        // Deserialize
        assert_eq!(expected, RejectedReason::from_str(&expected_string).unwrap());
        assert_eq!(expected, serde_json::from_str(&candidate_string).unwrap());
    }

    fn check_bincode(expected: RejectedReason) {
        // Serialize
        let expected_bytes = expected.to_bytes_le().unwrap();
        let expected_bytes_with_size_encoding = bincode::serialize(&expected).unwrap();
        assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

        // Deserialize
        assert_eq!(expected, RejectedReason::read_le(&expected_bytes[..]).unwrap());
        assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..]).unwrap());
    }

    #[test]
    fn test_serde_json() {
        for reason in crate::transactions::rejected_reason::test_helpers::sample_rejected_reasons() {
            check_serde_json(reason);
        }
    }

    #[test]
    fn test_bincode() {
        for reason in crate::transactions::rejected_reason::test_helpers::sample_rejected_reasons() {
            check_bincode(reason);
        }
    }
}
