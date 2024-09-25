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

impl<N: Network> FromBytes for PrivateKey<N> {
    /// Reads an account private key from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        Self::try_from(Field::new(FromBytes::read_le(&mut reader)?)).map_err(|e| error(format!("{e}")))
    }
}

impl<N: Network> ToBytes for PrivateKey<N> {
    /// Writes an account private key to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.seed.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 1000;

    #[test]
    fn test_bytes() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a new private key.
            let expected = PrivateKey::<CurrentNetwork>::new(&mut rng)?;

            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(expected, PrivateKey::read_le(&expected_bytes[..])?);
            assert!(PrivateKey::<CurrentNetwork>::read_le(&expected_bytes[1..]).is_err());
        }
        Ok(())
    }
}
