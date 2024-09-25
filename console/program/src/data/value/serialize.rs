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

impl<N: Network> Serialize for Value<N> {
    /// Serializes the value into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => serializer.collect_str(self),
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Value<N> {
    /// Deserializes the value from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => FromStr::from_str(&String::deserialize(deserializer)?).map_err(de::Error::custom),
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "value"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_serde_json() -> Result<()> {
        {
            // Prepare the plaintext string.
            let string = r"{
  owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah,
  token_amount: 100u64
}";
            // Construct a new plaintext value.
            let expected = Value::<CurrentNetwork>::from_str(string).unwrap();
            assert!(matches!(expected, Value::Plaintext(..)));

            // Serialize
            let expected_string = &expected.to_string();
            let candidate_string = serde_json::to_string(&expected)?;
            assert_eq!(expected_string, serde_json::Value::from_str(&candidate_string)?.as_str().unwrap());

            // Deserialize
            assert_eq!(expected, Value::from_str(expected_string)?);
            assert_eq!(expected, serde_json::from_str(&candidate_string)?);
        }
        {
            // Prepare the record string.
            let string = r"{
  owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah.private,
  token_amount: 100u64.private,
  _nonce: 6122363155094913586073041054293642159180066699840940609722305038224296461351group.public
}";
            // Construct a new record value.
            let expected = Value::<CurrentNetwork>::from_str(string).unwrap();
            assert!(matches!(expected, Value::Record(..)));

            // Serialize
            let expected_string = &expected.to_string();
            let candidate_string = serde_json::to_string(&expected)?;
            assert_eq!(expected_string, serde_json::Value::from_str(&candidate_string)?.as_str().unwrap());

            // Deserialize
            assert_eq!(expected, Value::from_str(expected_string)?);
            assert_eq!(expected, serde_json::from_str(&candidate_string)?);
        }
        Ok(())
    }

    #[test]
    fn test_bincode() -> Result<()> {
        {
            // Prepare the plaintext string.
            let string = r"{
  owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah,
  token_amount: 100u64
}";
            // Construct a new plaintext value.
            let expected = Value::<CurrentNetwork>::from_str(string).unwrap();
            assert!(matches!(expected, Value::Plaintext(..)));

            // Serialize
            let expected_bytes = expected.to_bytes_le()?;
            let expected_bytes_with_size_encoding = bincode::serialize(&expected)?;
            assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

            // Deserialize
            assert_eq!(expected, Value::read_le(&expected_bytes[..])?);
            assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..])?);
        }
        {
            // Prepare the record string.
            let string = r"{
  owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah.private,
  token_amount: 100u64.private,
  _nonce: 6122363155094913586073041054293642159180066699840940609722305038224296461351group.public
}";
            // Construct a new record value.
            let expected = Value::<CurrentNetwork>::from_str(string).unwrap();
            assert!(matches!(expected, Value::Record(..)));

            // Serialize
            let expected_bytes = expected.to_bytes_le()?;
            let expected_bytes_with_size_encoding = bincode::serialize(&expected)?;
            assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

            // Deserialize
            assert_eq!(expected, Value::read_le(&expected_bytes[..])?);
            assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..])?);
        }
        Ok(())
    }
}
