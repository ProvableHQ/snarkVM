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

impl<E: Environment> FromBytes for Boolean<E> {
    /// Reads the boolean from a buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        Ok(Self::new(FromBytes::read_le(&mut reader)?))
    }
}

impl<E: Environment> ToBytes for Boolean<E> {
    /// Writes the boolean to a buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.boolean.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 10_000;

    #[test]
    fn test_bytes() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a new boolean.
            let expected = Boolean::<CurrentEnvironment>::new(Uniform::rand(&mut rng));

            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(expected, Boolean::read_le(&expected_bytes[..])?);
            assert!(Boolean::<CurrentEnvironment>::read_le(&expected_bytes[1..]).is_err());
        }
        Ok(())
    }
}
