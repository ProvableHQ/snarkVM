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

impl<E: Environment, I: IntegerType> FromBytes for Integer<E, I> {
    /// Reads the integer from a buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        Ok(Self::new(FromBytes::read_le(&mut reader)?))
    }
}

impl<E: Environment, I: IntegerType> ToBytes for Integer<E, I> {
    /// Writes the integer to a buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.integer.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 10_000;

    fn check_bytes<I: IntegerType>(rng: &mut TestRng) -> Result<()> {
        for _ in 0..ITERATIONS {
            // Sample a random integer.
            let expected: Integer<CurrentEnvironment, I> = Uniform::rand(rng);

            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(expected, Integer::read_le(&expected_bytes[..])?);
            assert!(Integer::<CurrentEnvironment, I>::read_le(&expected_bytes[1..]).is_err());

            // Dereference the integer and compare bytes.
            let deref_bytes = (*expected).to_bytes_le()?;
            for (expected, candidate) in expected_bytes.iter().zip_eq(&deref_bytes) {
                assert_eq!(expected, candidate);
            }
        }
        Ok(())
    }

    #[test]
    fn test_bytes() -> Result<()> {
        let mut rng = TestRng::default();

        check_bytes::<u8>(&mut rng)?;
        check_bytes::<u16>(&mut rng)?;
        check_bytes::<u32>(&mut rng)?;
        check_bytes::<u64>(&mut rng)?;
        check_bytes::<u128>(&mut rng)?;

        check_bytes::<i8>(&mut rng)?;
        check_bytes::<i16>(&mut rng)?;
        check_bytes::<i32>(&mut rng)?;
        check_bytes::<i64>(&mut rng)?;
        check_bytes::<i128>(&mut rng)?;

        Ok(())
    }
}
