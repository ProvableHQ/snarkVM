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

impl<N: Network> FromBits for Signature<N> {
    /// Initializes a new signature from a list of **little-endian** bits.
    fn from_bits_le(bits_le: &[bool]) -> Result<Self> {
        let scalar_size_in_bits = Scalar::<N>::size_in_bits();
        let compute_key_size_in_bits = ComputeKey::<N>::size_in_bits();

        let (challenge_start, challenge_end) = (0, scalar_size_in_bits);
        let (response_start, response_end) = (challenge_end, challenge_end + scalar_size_in_bits);
        let (compute_key_start, compute_key_end) = (response_end, response_end + compute_key_size_in_bits);

        let Some(challenge_bits) = bits_le.get(challenge_start..challenge_end) else {
            bail!("Unable to recover the signature challenge from (LE) bits");
        };
        let Some(response_bits) = bits_le.get(response_start..response_end) else {
            bail!("Unable to recover the signature response from (LE) bits");
        };
        let Some(compute_key_bits) = bits_le.get(compute_key_start..compute_key_end) else {
            bail!("Unable to recover the signature compute key from (LE) bits");
        };

        Ok(Self {
            challenge: Scalar::from_bits_le(challenge_bits)?,
            response: Scalar::from_bits_le(response_bits)?,
            compute_key: ComputeKey::from_bits_le(compute_key_bits)?,
        })
    }

    /// Initializes a new signature from a list of **big-endian** bits.
    fn from_bits_be(bits_be: &[bool]) -> Result<Self> {
        let scalar_size_in_bits = Scalar::<N>::size_in_bits();
        let compute_key_size_in_bits = ComputeKey::<N>::size_in_bits();

        let (challenge_start, challenge_end) = (0, scalar_size_in_bits);
        let (response_start, response_end) = (challenge_end, challenge_end + scalar_size_in_bits);
        let (compute_key_start, compute_key_end) = (response_end, response_end + compute_key_size_in_bits);

        let Some(challenge_bits) = bits_be.get(challenge_start..challenge_end) else {
            bail!("Unable to recover the signature challenge from (BE) bits");
        };
        let Some(response_bits) = bits_be.get(response_start..response_end) else {
            bail!("Unable to recover the signature response from (BE) bits");
        };
        let Some(compute_key_bits) = bits_be.get(compute_key_start..compute_key_end) else {
            bail!("Unable to recover the signature compute key from (BE) bits");
        };

        Ok(Self {
            challenge: Scalar::from_bits_be(challenge_bits)?,
            response: Scalar::from_bits_be(response_bits)?,
            compute_key: ComputeKey::from_bits_be(compute_key_bits)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: usize = 100;

    fn check_from_bits_le() -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random signature.
            let expected = test_helpers::sample_signature(i as u64, rng);

            let given_bits = expected.to_bits_le();
            assert_eq!(Signature::<CurrentNetwork>::size_in_bits(), given_bits.len());

            let candidate = Signature::<CurrentNetwork>::from_bits_le(&given_bits)?;
            assert_eq!(expected, candidate);

            // Add excess zero bits.
            let candidate = [given_bits, vec![false; i]].concat();

            let candidate = Signature::<CurrentNetwork>::from_bits_le(&candidate)?;
            assert_eq!(expected, candidate);
            assert_eq!(Signature::<CurrentNetwork>::size_in_bits(), candidate.to_bits_le().len());
        }
        Ok(())
    }

    fn check_from_bits_be() -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random signature.
            let expected = test_helpers::sample_signature(i as u64, rng);

            let given_bits = expected.to_bits_be();
            assert_eq!(Signature::<CurrentNetwork>::size_in_bits(), given_bits.len());

            let candidate = Signature::<CurrentNetwork>::from_bits_be(&given_bits)?;
            assert_eq!(expected, candidate);

            // Add excess zero bits.
            let candidate = [given_bits, vec![false; i]].concat();

            let candidate = Signature::<CurrentNetwork>::from_bits_be(&candidate)?;
            assert_eq!(expected, candidate);
            assert_eq!(Signature::<CurrentNetwork>::size_in_bits(), candidate.to_bits_be().len());
        }
        Ok(())
    }

    #[test]
    fn test_from_bits_le() -> Result<()> {
        check_from_bits_le()
    }

    #[test]
    fn test_from_bits_be() -> Result<()> {
        check_from_bits_be()
    }
}
