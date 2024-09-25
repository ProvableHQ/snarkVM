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

impl<N: Network> Serialize for Authority<N> {
    /// Serializes the authority into string or bytes.
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => {
                let mut authority = serializer.serialize_struct("Authority", 2)?;
                match self {
                    Self::Beacon(signature) => {
                        authority.serialize_field("type", "beacon")?;
                        authority.serialize_field("signature", signature)?;
                    }
                    Self::Quorum(subdag) => {
                        authority.serialize_field("type", "quorum")?;
                        authority.serialize_field("subdag", subdag)?;
                    }
                }
                authority.end()
            }
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Authority<N> {
    /// Deserializes the authority from a string or bytes.
    #[inline]
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                let mut authority = serde_json::Value::deserialize(deserializer)?;
                let type_: String = DeserializeExt::take_from_value::<D>(&mut authority, "type")?;

                // Recover the authority.
                match type_.as_str() {
                    "beacon" => Ok(Self::from_beacon(
                        DeserializeExt::take_from_value::<D>(&mut authority, "signature").map_err(de::Error::custom)?,
                    )),
                    "quorum" => Ok(Self::from_quorum(
                        DeserializeExt::take_from_value::<D>(&mut authority, "subdag").map_err(de::Error::custom)?,
                    )),
                    _ => Err(de::Error::custom(error("Invalid authority type"))),
                }
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "authority"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::prelude::TestRng;

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

        for expected in crate::test_helpers::sample_authorities(rng) {
            check_serde_json(expected);
        }
    }

    #[test]
    fn test_bincode() {
        let rng = &mut TestRng::default();

        for expected in crate::test_helpers::sample_authorities(rng) {
            check_bincode(expected);
        }
    }
}
