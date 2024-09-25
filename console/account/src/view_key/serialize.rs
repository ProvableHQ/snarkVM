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

impl<N: Network> Serialize for ViewKey<N> {
    /// Serializes an account view key into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => serializer.collect_str(self),
            false => ToBytesSerializer::serialize(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for ViewKey<N> {
    /// Deserializes an account view key from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => FromStr::from_str(&String::deserialize(deserializer)?).map_err(de::Error::custom),
            false => FromBytesDeserializer::<Self>::deserialize(
                deserializer,
                "view key",
                (N::Scalar::size_in_bits() + 7) / 8,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 1000;

    #[test]
    fn test_serde_json() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a new view key.
            let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng)?;
            let expected = ViewKey::try_from(private_key)?;

            // Serialize
            let expected_string = &expected.to_string();
            let candidate_string = serde_json::to_string(&expected)?;
            assert_eq!(expected_string, serde_json::Value::from_str(&candidate_string)?.as_str().unwrap());

            // Deserialize
            assert_eq!(expected, ViewKey::from_str(expected_string)?);
            assert_eq!(expected, serde_json::from_str(&candidate_string)?);
        }
        Ok(())
    }

    #[test]
    fn test_bincode() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a new view key.
            let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng)?;
            let expected = ViewKey::try_from(private_key)?;

            // Serialize
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(&expected_bytes[..], &bincode::serialize(&expected)?[..]);

            // Deserialize
            assert_eq!(expected, ViewKey::read_le(&expected_bytes[..])?);
            assert_eq!(expected, bincode::deserialize(&expected_bytes[..])?);
        }
        Ok(())
    }
}
