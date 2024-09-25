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

impl<N: Network> Serialize for FinalizeOperation<N> {
    /// Serializes the finalize operations to a JSON-string or buffer.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => {
                // Serialize the components.
                match self {
                    Self::InitializeMapping(mapping_id) => {
                        let mut operation = serializer.serialize_struct("FinalizeOperation", 2)?;
                        operation.serialize_field("type", "initialize_mapping")?;
                        operation.serialize_field("mapping_id", mapping_id)?;
                        operation.end()
                    }
                    Self::InsertKeyValue(mapping_id, key_id, value_id) => {
                        let mut operation = serializer.serialize_struct("FinalizeOperation", 4)?;
                        operation.serialize_field("type", "insert_key_value")?;
                        operation.serialize_field("mapping_id", mapping_id)?;
                        operation.serialize_field("key_id", key_id)?;
                        operation.serialize_field("value_id", value_id)?;
                        operation.end()
                    }
                    Self::UpdateKeyValue(mapping_id, key_id, value_id) => {
                        let mut operation = serializer.serialize_struct("FinalizeOperation", 4)?;
                        operation.serialize_field("type", "update_key_value")?;
                        operation.serialize_field("mapping_id", mapping_id)?;
                        operation.serialize_field("key_id", key_id)?;
                        operation.serialize_field("value_id", value_id)?;
                        operation.end()
                    }
                    Self::RemoveKeyValue(mapping_id, key_id) => {
                        let mut operation = serializer.serialize_struct("FinalizeOperation", 3)?;
                        operation.serialize_field("type", "remove_key_value")?;
                        operation.serialize_field("mapping_id", mapping_id)?;
                        operation.serialize_field("key_id", key_id)?;
                        operation.end()
                    }
                    Self::ReplaceMapping(mapping_id) => {
                        let mut operation = serializer.serialize_struct("FinalizeOperation", 2)?;
                        operation.serialize_field("type", "replace_mapping")?;
                        operation.serialize_field("mapping_id", mapping_id)?;
                        operation.end()
                    }
                    Self::RemoveMapping(mapping_id) => {
                        let mut operation = serializer.serialize_struct("FinalizeOperation", 2)?;
                        operation.serialize_field("type", "remove_mapping")?;
                        operation.serialize_field("mapping_id", mapping_id)?;
                        operation.end()
                    }
                }
            }
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for FinalizeOperation<N> {
    /// Deserializes the finalize operations from a JSON-string or buffer.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                let mut operation = serde_json::Value::deserialize(deserializer)?;
                // Recover the operation.
                let finalize_operation = match operation.get("type").and_then(|t| t.as_str()) {
                    Some("initialize_mapping") => {
                        // Deserialize the mapping ID.
                        let mapping_id = DeserializeExt::take_from_value::<D>(&mut operation, "mapping_id")?;
                        // Return the operation.
                        Self::InitializeMapping(mapping_id)
                    }
                    Some("insert_key_value") => {
                        // Deserialize the mapping ID.
                        let mapping_id = DeserializeExt::take_from_value::<D>(&mut operation, "mapping_id")?;
                        // Deserialize the key ID.
                        let key_id = DeserializeExt::take_from_value::<D>(&mut operation, "key_id")?;
                        // Deserialize the value ID.
                        let value_id = DeserializeExt::take_from_value::<D>(&mut operation, "value_id")?;
                        // Return the operation.
                        Self::InsertKeyValue(mapping_id, key_id, value_id)
                    }
                    Some("update_key_value") => {
                        // Deserialize the mapping ID.
                        let mapping_id = DeserializeExt::take_from_value::<D>(&mut operation, "mapping_id")?;
                        // Deserialize the key ID.
                        let key_id = DeserializeExt::take_from_value::<D>(&mut operation, "key_id")?;
                        // Deserialize the value ID.
                        let value_id = DeserializeExt::take_from_value::<D>(&mut operation, "value_id")?;
                        // Return the operation.
                        Self::UpdateKeyValue(mapping_id, key_id, value_id)
                    }
                    Some("remove_key_value") => {
                        // Deserialize the mapping ID.
                        let mapping_id = DeserializeExt::take_from_value::<D>(&mut operation, "mapping_id")?;
                        // Deserialize the key ID.
                        let key_id = DeserializeExt::take_from_value::<D>(&mut operation, "key_id")?;
                        // Return the operation.
                        Self::RemoveKeyValue(mapping_id, key_id)
                    }
                    Some("replace_mapping") => {
                        // Deserialize the mapping ID.
                        let mapping_id = DeserializeExt::take_from_value::<D>(&mut operation, "mapping_id")?;
                        // Return the operation.
                        Self::ReplaceMapping(mapping_id)
                    }
                    Some("remove_mapping") => {
                        // Deserialize the mapping ID.
                        let mapping_id = DeserializeExt::take_from_value::<D>(&mut operation, "mapping_id")?;
                        // Return the operation.
                        Self::RemoveMapping(mapping_id)
                    }
                    _ => return Err(de::Error::custom("Invalid finalize operation type")),
                };
                // Return the operation.
                Ok(finalize_operation)
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "finalize operations"),
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
        for finalize in crate::logic::finalize_operation::test_helpers::sample_finalize_operations() {
            check_serde_json(finalize);
        }
    }

    #[test]
    fn test_bincode() {
        for finalize in crate::logic::finalize_operation::test_helpers::sample_finalize_operations() {
            check_bincode(finalize);
        }
    }
}
