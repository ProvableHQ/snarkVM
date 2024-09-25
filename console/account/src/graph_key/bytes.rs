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

impl<N: Network> FromBytes for GraphKey<N> {
    /// Reads an account graph key from a buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let sk_tag = Field::<N>::read_le(&mut reader).map_err(|e| error(format!("{e}")))?;
        Self::try_from(sk_tag).map_err(|e| error(format!("{e}")))
    }
}

impl<N: Network> ToBytes for GraphKey<N> {
    /// Writes an account graph key to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.sk_tag.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrivateKey;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 1000;

    #[test]
    fn test_bytes() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a new graph key.
            let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng)?;
            let view_key = ViewKey::try_from(private_key)?;
            let expected = GraphKey::try_from(view_key)?;

            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(expected, GraphKey::read_le(&expected_bytes[..])?);
            assert!(GraphKey::<CurrentNetwork>::read_le(&expected_bytes[1..]).is_err());
        }
        Ok(())
    }
}
