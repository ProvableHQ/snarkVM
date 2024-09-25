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

impl<E: Environment, I: IntegerType> Serialize for Integer<E, I> {
    /// Serializes the integer into a string or as bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => serializer.collect_str(self),
            false => ToBytesSerializer::serialize(self, serializer),
        }
    }
}

impl<'de, E: Environment, I: IntegerType> Deserialize<'de> for Integer<E, I> {
    /// Deserializes the integer from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => FromStr::from_str(&String::deserialize(deserializer)?).map_err(de::Error::custom),
            false => FromBytesDeserializer::<Self>::deserialize(deserializer, "integer", Self::size_in_bytes()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 1000;

    fn check_serde_json<I: IntegerType>(rng: &mut TestRng) -> Result<()> {
        for _ in 0..ITERATIONS {
            // Sample a new integer.
            let expected = Integer::<CurrentEnvironment, I>::new(Uniform::rand(rng));

            // Serialize
            let expected_string = &expected.to_string();
            let candidate_string = serde_json::to_string(&expected)?;
            assert_eq!(expected_string, serde_json::Value::from_str(&candidate_string)?.as_str().unwrap());

            // Deserialize
            assert_eq!(expected, Integer::from_str(expected_string)?);
            assert_eq!(expected, serde_json::from_str(&candidate_string)?);
        }
        Ok(())
    }

    fn check_bincode<I: IntegerType>(rng: &mut TestRng) -> Result<()> {
        for _ in 0..ITERATIONS {
            // Sample a new integer.
            let expected = Integer::<CurrentEnvironment, I>::new(Uniform::rand(rng));

            // Serialize
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(&expected_bytes[..], &bincode::serialize(&expected)?[..]);

            // Deserialize
            assert_eq!(expected, Integer::read_le(&expected_bytes[..])?);
            assert_eq!(expected, bincode::deserialize(&expected_bytes[..])?);
        }
        Ok(())
    }

    #[test]
    fn test_serde_json() -> Result<()> {
        let mut rng = TestRng::default();

        check_serde_json::<u8>(&mut rng)?;
        check_serde_json::<u16>(&mut rng)?;
        check_serde_json::<u32>(&mut rng)?;
        check_serde_json::<u64>(&mut rng)?;
        check_serde_json::<u128>(&mut rng)?;

        check_serde_json::<i8>(&mut rng)?;
        check_serde_json::<i16>(&mut rng)?;
        check_serde_json::<i32>(&mut rng)?;
        check_serde_json::<i64>(&mut rng)?;
        check_serde_json::<i128>(&mut rng)?;
        Ok(())
    }

    #[test]
    fn test_bincode() -> Result<()> {
        let mut rng = TestRng::default();

        check_bincode::<u8>(&mut rng)?;
        check_bincode::<u16>(&mut rng)?;
        check_bincode::<u32>(&mut rng)?;
        check_bincode::<u64>(&mut rng)?;
        check_bincode::<u128>(&mut rng)?;

        check_bincode::<i8>(&mut rng)?;
        check_bincode::<i16>(&mut rng)?;
        check_bincode::<i32>(&mut rng)?;
        check_bincode::<i64>(&mut rng)?;
        check_bincode::<i128>(&mut rng)?;
        Ok(())
    }
}
