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

impl<N: Network> Serialize for Transmission<N> {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => match self {
                Self::Ratification => {
                    let mut transmission = serializer.serialize_struct("Transmission", 1)?;
                    transmission.serialize_field("type", "ratification")?;
                    transmission.end()
                }
                Self::Solution(solution) => {
                    let mut transmission = serializer.serialize_struct("Transmission", 2)?;
                    transmission.serialize_field("type", "solution")?;
                    transmission.serialize_field("transmission", solution)?;
                    transmission.end()
                }
                Self::Transaction(transaction) => {
                    let mut transmission = serializer.serialize_struct("Transmission", 2)?;
                    transmission.serialize_field("type", "transaction")?;
                    transmission.serialize_field("transmission", transaction)?;
                    transmission.end()
                }
            },
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Transmission<N> {
    #[inline]
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                let mut transmission = serde_json::Value::deserialize(deserializer)?;
                let type_: String = DeserializeExt::take_from_value::<D>(&mut transmission, "type")?;

                // Recover the transmission.
                match type_.as_str() {
                    "ratification" => Ok(Self::Ratification),
                    "solution" => Ok(Self::Solution(
                        DeserializeExt::take_from_value::<D>(&mut transmission, "transmission")
                            .map_err(de::Error::custom)?,
                    )),
                    "transaction" => Ok(Self::Transaction(
                        DeserializeExt::take_from_value::<D>(&mut transmission, "transmission")
                            .map_err(de::Error::custom)?,
                    )),
                    _ => Err(de::Error::custom(error("Invalid transmission type"))),
                }
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "transmission"),
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

        for expected in crate::test_helpers::sample_transmissions(rng) {
            check_serde_json(expected);
        }
    }

    #[test]
    fn test_bincode() {
        let rng = &mut TestRng::default();

        for expected in crate::test_helpers::sample_transmissions(rng) {
            check_bincode(expected);
        }
    }
}
