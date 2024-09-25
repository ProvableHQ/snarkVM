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

impl<N: Network> Serialize for Ratify<N> {
    /// Serializes the ratify object into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => match self {
                Self::Genesis(committee, public_balances, bonded_balances) => {
                    let mut input = serializer.serialize_struct("Ratify", 4)?;
                    input.serialize_field("type", "genesis")?;
                    input.serialize_field("committee", &committee)?;
                    input.serialize_field("public_balances", &public_balances)?;
                    input.serialize_field("bonded_balances", &bonded_balances)?;
                    input.end()
                }
                Self::BlockReward(amount) => {
                    let mut input = serializer.serialize_struct("Ratify", 2)?;
                    input.serialize_field("type", "block_reward")?;
                    input.serialize_field("amount", &amount)?;
                    input.end()
                }
                Self::PuzzleReward(amount) => {
                    let mut input = serializer.serialize_struct("Ratify", 2)?;
                    input.serialize_field("type", "puzzle_reward")?;
                    input.serialize_field("amount", &amount)?;
                    input.end()
                }
            },
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Ratify<N> {
    /// Deserializes the ratify object from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                // Parse the input from a string into a value.
                let mut object = serde_json::Value::deserialize(deserializer)?;

                // Recover the ratify object.
                let ratify = match object.get("type").and_then(|t| t.as_str()) {
                    Some("genesis") => {
                        // Retrieve the committee.
                        let committee: Committee<N> = DeserializeExt::take_from_value::<D>(&mut object, "committee")?;
                        // Retrieve the public balances.
                        let public_balances: PublicBalances<N> =
                            DeserializeExt::take_from_value::<D>(&mut object, "public_balances")?;
                        // Retrieve the bonded balances.
                        let bonded_balances: BondedBalances<N> =
                            DeserializeExt::take_from_value::<D>(&mut object, "bonded_balances")?;
                        // Construct the ratify object.
                        Ratify::Genesis(Box::new(committee), Box::new(public_balances), Box::new(bonded_balances))
                    }
                    Some("block_reward") => {
                        // Retrieve the amount.
                        let amount: u64 = DeserializeExt::take_from_value::<D>(&mut object, "amount")?;
                        // Construct the ratify object.
                        Ratify::BlockReward(amount)
                    }
                    Some("puzzle_reward") => {
                        // Retrieve the amount.
                        let amount: u64 = DeserializeExt::take_from_value::<D>(&mut object, "amount")?;
                        // Construct the ratify object.
                        Ratify::PuzzleReward(amount)
                    }
                    _ => return Err(de::Error::custom("Invalid ratify object type")),
                };
                // Return the ratify object.
                Ok(ratify)
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "ratify object"),
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
        let rng = &mut TestRng::default();

        for expected in crate::ratify::test_helpers::sample_ratifications(rng) {
            check_serde_json(expected);
        }
    }

    #[test]
    fn test_bincode() {
        let rng = &mut TestRng::default();

        for expected in crate::ratify::test_helpers::sample_ratifications(rng) {
            check_bincode(expected);
        }
    }
}
