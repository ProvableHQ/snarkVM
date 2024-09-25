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

impl<N: Network> FromBits for ComputeKey<N> {
    /// Initializes a new compute key from a list of **little-endian** bits.
    fn from_bits_le(bits_le: &[bool]) -> Result<Self> {
        let group_size_in_bits = Group::<N>::size_in_bits();

        let (pk_sig_start, pk_sig_end) = (0, group_size_in_bits);
        let (pr_sig_start, pr_sig_end) = (pk_sig_end, pk_sig_end + group_size_in_bits);

        let Some(pk_sig_bits) = bits_le.get(pk_sig_start..pk_sig_end) else {
            bail!("Unable to recover the 'pk_sig' (LE) bits for the compute key");
        };
        let Some(pr_sig_bits) = bits_le.get(pr_sig_start..pr_sig_end) else {
            bail!("Unable to recover the 'pr_sig' (LE) bits for the compute key");
        };

        Self::try_from((Group::from_bits_le(pk_sig_bits)?, Group::from_bits_le(pr_sig_bits)?))
    }

    /// Initializes a new compute key from a list of **big-endian** bits.
    fn from_bits_be(bits_be: &[bool]) -> Result<Self> {
        let group_size_in_bits = Group::<N>::size_in_bits();

        let (pk_sig_start, pk_sig_end) = (0, group_size_in_bits);
        let (pr_sig_start, pr_sig_end) = (pk_sig_end, pk_sig_end + group_size_in_bits);

        let Some(pk_sig_bits) = bits_be.get(pk_sig_start..pk_sig_end) else {
            bail!("Unable to recover the 'pk_sig' (BE) bits for the compute key");
        };
        let Some(pr_sig_bits) = bits_be.get(pr_sig_start..pr_sig_end) else {
            bail!("Unable to recover the 'pr_sig' (BE) bits for the compute key");
        };

        Self::try_from((Group::from_bits_be(pk_sig_bits)?, Group::from_bits_be(pr_sig_bits)?))
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
            // Sample a random compute_key.
            let expected = ComputeKey::<CurrentNetwork>::try_from(PrivateKey::new(rng).unwrap()).unwrap();

            let given_bits = expected.to_bits_le();
            assert_eq!(ComputeKey::<CurrentNetwork>::size_in_bits(), given_bits.len());

            let candidate = ComputeKey::<CurrentNetwork>::from_bits_le(&given_bits)?;
            assert_eq!(expected, candidate);

            // Add excess zero bits.
            let candidate = [given_bits, vec![false; i]].concat();

            let candidate = ComputeKey::<CurrentNetwork>::from_bits_le(&candidate)?;
            assert_eq!(expected, candidate);
            assert_eq!(ComputeKey::<CurrentNetwork>::size_in_bits(), candidate.to_bits_le().len());
        }
        Ok(())
    }

    fn check_from_bits_be() -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random compute_key.
            let expected = ComputeKey::<CurrentNetwork>::try_from(PrivateKey::new(rng).unwrap()).unwrap();

            let given_bits = expected.to_bits_be();
            assert_eq!(ComputeKey::<CurrentNetwork>::size_in_bits(), given_bits.len());

            let candidate = ComputeKey::<CurrentNetwork>::from_bits_be(&given_bits)?;
            assert_eq!(expected, candidate);

            // Add excess zero bits.
            let candidate = [given_bits, vec![false; i]].concat();

            let candidate = ComputeKey::<CurrentNetwork>::from_bits_be(&candidate)?;
            assert_eq!(expected, candidate);
            assert_eq!(ComputeKey::<CurrentNetwork>::size_in_bits(), candidate.to_bits_be().len());
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
