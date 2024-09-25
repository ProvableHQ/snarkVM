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

impl<N: Network> FromBytes for Signature<N> {
    /// Reads an account signature from a buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let challenge = Scalar::new(FromBytes::read_le(&mut reader)?);
        let response = Scalar::new(FromBytes::read_le(&mut reader)?);
        let compute_key = ComputeKey::read_le(&mut reader)?;
        Ok(Self { challenge, response, compute_key })
    }
}

impl<N: Network> ToBytes for Signature<N> {
    /// Writes an account signature to a buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.challenge.write_le(&mut writer)?;
        self.response.write_le(&mut writer)?;
        self.compute_key.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 100;

    #[test]
    fn test_bytes() -> Result<()> {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a new signature.
            let signature = test_helpers::sample_signature(i, &mut rng);

            // Check the byte representation.
            let signature_bytes = signature.to_bytes_le()?;
            assert_eq!(signature, Signature::read_le(&signature_bytes[..])?);
            assert!(Signature::<CurrentNetwork>::read_le(&signature_bytes[1..]).is_err());
        }
        Ok(())
    }
}
