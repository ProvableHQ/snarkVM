// Copyright (c) 2019-2025 Provable Inc.
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

impl<N: Network> Serialize for ConfirmedTransaction<N> {
    /// Serializes the confirmed transaction into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => match self {
                Self::AcceptedDeploy(index, transaction, finalize_operations) => {
                    let mut object = serializer.serialize_struct("ConfirmedTransaction", 5)?;
                    object.serialize_field("status", "accepted")?;
                    object.serialize_field("type", "deploy")?;
                    object.serialize_field("index", index)?;
                    object.serialize_field("transaction", transaction)?;
                    object.serialize_field("finalize", finalize_operations)?;
                    object.end()
                }
                Self::AcceptedExecute(index, transaction, finalize_operations) => {
                    let mut object = serializer.serialize_struct("ConfirmedTransaction", 5)?;
                    object.serialize_field("status", "accepted")?;
                    object.serialize_field("type", "execute")?;
                    object.serialize_field("index", index)?;
                    object.serialize_field("transaction", transaction)?;
                    object.serialize_field("finalize", finalize_operations)?;
                    object.end()
                }
                Self::RejectedDeploy(index, transaction, rejected_deployment, finalize_operations) => {
                    let mut object = serializer.serialize_struct("ConfirmedTransaction", 6)?;
                    object.serialize_field("status", "rejected")?;
                    object.serialize_field("type", "deploy")?;
                    object.serialize_field("index", index)?;
                    object.serialize_field("transaction", transaction)?;
                    object.serialize_field("rejected", &rejected_deployment)?;
                    object.serialize_field("finalize", finalize_operations)?;
                    object.end()
                }
                Self::RejectedExecute(index, transaction, rejected_execution, finalize_operations) => {
                    let mut object = serializer.serialize_struct("ConfirmedTransaction", 6)?;
                    object.serialize_field("status", "rejected")?;
                    object.serialize_field("type", "execute")?;
                    object.serialize_field("index", index)?;
                    object.serialize_field("transaction", transaction)?;
                    object.serialize_field("rejected", &rejected_execution)?;
                    object.serialize_field("finalize", finalize_operations)?;
                    object.end()
                }
            },
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for ConfirmedTransaction<N> {
    /// Deserializes the confirmed transaction from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                // Parse the confirmed transaction from a string into a value.
                let mut object = serde_json::Value::deserialize(deserializer)?;

                // Parse the index.
                let index: u32 = DeserializeExt::take_from_value::<D>(&mut object, "index")?;
                // Parse the transaction.
                let transaction: Transaction<N> = DeserializeExt::take_from_value::<D>(&mut object, "transaction")?;

                // Parse the status and type.
                let status = object.get("status").and_then(|t| t.as_str());
                let type_ = object.get("type").and_then(|t| t.as_str());

                // Recover the confirmed transaction.
                match (status, type_) {
                    (Some("accepted"), Some("deploy")) => {
                        // Parse the finalize operations.
                        let finalize: Vec<_> = DeserializeExt::take_from_value::<D>(&mut object, "finalize")?;
                        // Return the accepted deploy transaction.
                        Self::accepted_deploy(index, transaction, finalize).map_err(de::Error::custom)
                    }
                    (Some("accepted"), Some("execute")) => {
                        // Parse the finalize operations.
                        let finalize: Vec<_> = DeserializeExt::take_from_value::<D>(&mut object, "finalize")?;
                        // Return the accepted execute transaction.
                        Self::accepted_execute(index, transaction, finalize).map_err(de::Error::custom)
                    }
                    (Some("rejected"), Some("deploy")) => {
                        // Parse the rejected deployment.
                        let rejected: Rejected<N> = DeserializeExt::take_from_value::<D>(&mut object, "rejected")?;
                        // Parse the finalize operations.
                        let finalize: Vec<_> = DeserializeExt::take_from_value::<D>(&mut object, "finalize")?;
                        // Return the rejected deploy transaction.
                        Self::rejected_deploy(index, transaction, rejected, finalize).map_err(de::Error::custom)
                    }
                    (Some("rejected"), Some("execute")) => {
                        // Parse the rejected execution.
                        let rejected: Rejected<N> = DeserializeExt::take_from_value::<D>(&mut object, "rejected")?;
                        // Parse the finalize operations.
                        let finalize: Vec<_> = DeserializeExt::take_from_value::<D>(&mut object, "finalize")?;
                        // Return the rejected execute transaction.
                        Self::rejected_execute(index, transaction, rejected, finalize).map_err(de::Error::custom)
                    }
                    _ => Err(de::Error::custom("Invalid confirmed transaction type")),
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
    use once_cell::sync::Lazy;

    type CurrentNetwork = MainnetV0;

    static SAMPLES: Lazy<Vec<ConfirmedTransaction<CurrentNetwork>>> =
        Lazy::new(crate::transactions::confirmed::test_helpers::sample_confirmed_transactions_for_serde);

    fn sample_limit() -> usize {
        std::env::var("SNARKVM_SERDE_SAMPLES").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(usize::MAX)
    }

    fn iter_samples<'a>() -> impl Iterator<Item = &'a ConfirmedTransaction<CurrentNetwork>> {
        let limit = sample_limit();
        SAMPLES.iter().take(limit)
    }

    fn check_display_fromstr_json_string_equivalence<T>(expected: &T)
    where
        T: Serialize + for<'a> Deserialize<'a> + Debug + Display + PartialEq + Eq + FromStr,
        <T as FromStr>::Err: Debug,
    {
        let expected_string = expected.to_string();

        let json_string = serde_json::to_string(expected).unwrap();
        assert_eq!(expected_string, json_string);

        let from_str = T::from_str(&expected_string).unwrap();
        assert_eq!(*expected, from_str);

        let from_json = serde_json::from_str::<T>(&json_string).unwrap();
        assert_eq!(*expected, from_json);
    }

    fn check_bincode_only<T>(expected: &T)
    where
        T: Serialize + for<'a> Deserialize<'a> + PartialEq + Eq + Debug,
    {
        let bytes = bincode::serialize(expected).unwrap();
        let decoded: T = bincode::deserialize(&bytes).unwrap();
        assert_eq!(*expected, decoded);
    }

    fn check_tobytes_matches_bincode_payload<T>(expected: &T)
    where
        T: Serialize + ToBytes,
    {
        let expected_bytes = expected.to_bytes_le().unwrap();
        let bincode_bytes = bincode::serialize(expected).unwrap();
        assert_eq!(&expected_bytes[..], &bincode_bytes[8..]);
    }

    fn check_display_roundtrip_via_json<T>(expected: &T)
    where
        T: for<'a> Deserialize<'a> + Display + PartialEq + Eq + Debug,
    {
        let s = expected.to_string();
        let from_display = serde_json::from_str::<T>(&s).unwrap();
        assert_eq!(*expected, from_display);
    }

    #[test]
    fn test_serde_json_roundtrip_fast() {
        for tx in iter_samples().take(3) {
            check_display_roundtrip_via_json(tx);
        }
    }

    #[test]
    fn test_serde_json_string_format_matches_display() {
        let all = std::env::var("SNARKVM_SERDE_DISPLAY_ALL")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let mut it = iter_samples();
        if all {
            for tx in it {
                check_display_fromstr_json_string_equivalence(tx);
            }
        } else {
            // Check only first few to keep the invariant covered.
            for tx in it.by_ref().take(5) {
                check_display_fromstr_json_string_equivalence(tx);
            }
        }
    }

    #[test]
    fn test_bincode_roundtrip() {
        for tx in iter_samples() {
            check_bincode_only(tx);
        }
    }

    #[test]
    fn test_bincode_payload_matches_tobytes_layout() {
        for tx in iter_samples().take(2) {
            check_tobytes_matches_bincode_payload(tx);
        }
    }
}
