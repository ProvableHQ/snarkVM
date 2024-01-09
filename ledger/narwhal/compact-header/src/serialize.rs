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

impl<N: Network> Serialize for CompactHeader<N> {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => {
                let mut header = serializer.serialize_struct("CompactHeader", 9)?;
                header.serialize_field("batch_id", &self.batch_id)?;
                header.serialize_field("author", &self.author)?;
                header.serialize_field("round", &self.round)?;
                header.serialize_field("timestamp", &self.timestamp)?;
                header.serialize_field("committee_id", &self.committee_id)?;
                header.serialize_field("transaction_indices", &self.transaction_indices.iter().collect_vec())?;
                header.serialize_field("solution_indices", &self.solution_indices.iter().collect_vec())?;
                header.serialize_field("previous_certificate_ids", &self.previous_certificate_ids)?;
                header.serialize_field("signature", &self.signature)?;
                header.end()
            }
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for CompactHeader<N> {
    #[inline]
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                let mut header = serde_json::Value::deserialize(deserializer)?;
                let batch_id: Field<N> = DeserializeExt::take_from_value::<D>(&mut header, "batch_id")?;

                // Parse the transaction indices.
                let transaction_indices_vec: Vec<_> =
                    DeserializeExt::take_from_value::<D>(&mut header, "transaction_indices")?;
                let mut transaction_indices = BitSet::with_capacity(transaction_indices_vec.len());
                for index in transaction_indices_vec {
                    transaction_indices.insert(index);
                }
                // Parse the solution indices.
                let solution_indices_vec: Vec<_> =
                    DeserializeExt::take_from_value::<D>(&mut header, "solution_indices")?;
                let mut solution_indices = BitSet::with_capacity(solution_indices_vec.len());
                for index in solution_indices_vec {
                    solution_indices.insert(index);
                }

                // Recover the header.
                let batch_header = Self::from(
                    batch_id,
                    DeserializeExt::take_from_value::<D>(&mut header, "author")?,
                    DeserializeExt::take_from_value::<D>(&mut header, "round")?,
                    DeserializeExt::take_from_value::<D>(&mut header, "timestamp")?,
                    DeserializeExt::take_from_value::<D>(&mut header, "committee_id")?,
                    transaction_indices,
                    solution_indices,
                    DeserializeExt::take_from_value::<D>(&mut header, "previous_certificate_ids")?,
                    DeserializeExt::take_from_value::<D>(&mut header, "signature")?,
                )
                .map_err(de::Error::custom)?;

                Ok(batch_header)
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "compact header"),
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
        let expected_string = &expected.to_string();
        let candidate_string = serde_json::to_string(&expected).unwrap();
        assert_eq!(expected_string, &serde_json::Value::from_str(&candidate_string).unwrap().to_string());

        // Deserialize
        assert_eq!(expected, T::from_str(expected_string).unwrap_or_else(|_| panic!("FromStr: {expected_string}")));
        assert_eq!(expected, serde_json::from_str(&candidate_string).unwrap());
    }

    fn check_bincode<T: Serialize + for<'a> Deserialize<'a> + Debug + PartialEq + Eq + ToBytes + FromBytes>(
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
        let rng = &mut TestRng::default();

        for expected in crate::test_helpers::sample_compact_headers(rng) {
            check_serde_json(expected);
        }
    }

    #[test]
    fn test_bincode() {
        let rng = &mut TestRng::default();

        for expected in crate::test_helpers::sample_compact_headers(rng) {
            check_bincode(expected);
        }
    }
}
