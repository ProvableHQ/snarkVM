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

impl<N: Network> Serialize for Plaintext<N> {
    /// Serializes the plaintext into a string or as bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => serializer.collect_str(self),
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Plaintext<N> {
    /// Deserializes the plaintext from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => FromStr::from_str(&String::deserialize(deserializer)?).map_err(de::Error::custom),
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "plaintext"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 2;

    #[test]
    fn test_serde_json() -> Result<()> {
        fn run_test(expected: Plaintext<CurrentNetwork>) {
            for _ in 0..ITERATIONS {
                // Serialize
                let expected_string = &expected.to_string();
                let candidate_string = serde_json::to_string(&expected).unwrap();
                assert_eq!(expected_string, serde_json::Value::from_str(&candidate_string).unwrap().as_str().unwrap());

                // Deserialize
                assert_eq!(expected, Plaintext::from_str(expected_string).unwrap());
                assert_eq!(expected, serde_json::from_str(&candidate_string).unwrap());
            }
        }

        // Test struct.
        run_test(Plaintext::<CurrentNetwork>::from_str(
            "{ owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah, token_amount: 100u64 }",
        )?);

        // Test array.
        run_test(Plaintext::<CurrentNetwork>::from_str("[ 0field, 1field, 2field, 3field, 4field ]")?);

        Ok(())
    }

    #[test]
    fn test_bincode() -> Result<()> {
        fn run_test(expected: Plaintext<CurrentNetwork>) {
            for _ in 0..ITERATIONS {
                // Serialize
                let expected_bytes = expected.to_bytes_le().unwrap();
                let expected_bytes_with_size_encoding = bincode::serialize(&expected).unwrap();
                assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

                // Deserialize
                assert_eq!(expected, Plaintext::read_le(&expected_bytes[..]).unwrap());
                assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..]).unwrap());
            }
        }

        // Test struct.
        run_test(Plaintext::<CurrentNetwork>::from_str(
            "{ owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah, token_amount: 100u64 }",
        )?);

        // Test array.
        run_test(Plaintext::<CurrentNetwork>::from_str("[ 0field, 1field, 2field, 3field, 4field ]")?);

        Ok(())
    }
}
