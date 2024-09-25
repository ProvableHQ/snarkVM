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

impl<N: Network> Serialize for Ratifications<N> {
    /// Serializes the ratifications to a JSON-string or buffer.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => {
                let mut ratifications = serializer.serialize_seq(Some(self.ratifications.len()))?;
                for ratify in self.ratifications.values() {
                    ratifications.serialize_element(ratify)?;
                }
                ratifications.end()
            }
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Ratifications<N> {
    /// Deserializes the ratifications from a JSON-string or buffer.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                use core::marker::PhantomData;

                struct RatificationsDeserializer<N: Network>(PhantomData<N>);

                impl<'de, N: Network> Visitor<'de> for RatificationsDeserializer<N> {
                    type Value = Vec<Ratify<N>>;

                    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
                        formatter.write_str("Vec<Ratify> sequence.")
                    }

                    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                        let mut ratifications = Vec::new();
                        while let Some(ratify) = seq.next_element()? {
                            ratifications.push(ratify);
                        }
                        Ok(ratifications)
                    }
                }

                Self::try_from(deserializer.deserialize_seq(RatificationsDeserializer(PhantomData))?)
                    .map_err(de::Error::custom)
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "ratifications"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u32 = 100;

    #[test]
    fn test_serde_json() {
        let rng = &mut TestRng::default();

        let check_serde_json = |expected: Ratifications<CurrentNetwork>| {
            // Serialize
            let expected_string = &expected.to_string();
            let candidate_string = serde_json::to_string(&expected).unwrap();

            // Deserialize
            assert_eq!(expected, Ratifications::from_str(expected_string).unwrap());
            assert_eq!(expected, serde_json::from_str(&candidate_string).unwrap());
        };

        for _ in 0..ITERATIONS {
            // Check the serialization.
            check_serde_json(crate::ratifications::test_helpers::sample_block_ratifications(rng));
        }
    }

    #[test]
    fn test_bincode() {
        let rng = &mut TestRng::default();

        let check_bincode = |expected: Ratifications<CurrentNetwork>| {
            // Serialize
            let expected_bytes = expected.to_bytes_le().unwrap();
            let expected_bytes_with_size_encoding = bincode::serialize(&expected).unwrap();
            assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

            // Deserialize
            assert_eq!(expected, Ratifications::read_le(&expected_bytes[..]).unwrap());
            assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..]).unwrap());
        };

        for _ in 0..ITERATIONS {
            // Check the serialization.
            check_bincode(crate::ratifications::test_helpers::sample_block_ratifications(rng));
        }
    }
}
