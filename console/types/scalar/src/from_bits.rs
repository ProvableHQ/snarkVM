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

impl<E: Environment> FromBits for Scalar<E> {
    /// Initializes a new scalar from a list of **little-endian** bits.
    ///   - If `bits_le` is longer than `E::Scalar::size_in_bits()`, the excess bits are enforced to be `0`s.
    ///   - If `bits_le` is shorter than `E::Scalar::size_in_bits()`, it is padded with `0`s up to scalar size.
    fn from_bits_le(bits_le: &[bool]) -> Result<Self> {
        // Retrieve the data and scalar size.
        let size_in_data_bits = Scalar::<E>::size_in_data_bits();
        let size_in_bits = Scalar::<E>::size_in_bits();

        // Ensure the list of booleans is within the allowed size in bits.
        let num_bits = bits_le.len();
        if num_bits > size_in_bits {
            // Check if all excess bits are zero.
            let should_be_zero = bits_le[size_in_bits..].iter().fold(false, |acc, bit| acc | bit);
            // Ensure `should_be_zero` is `false`.
            ensure!(!should_be_zero, "The excess bits are not zero.");
        }

        // If `num_bits` is greater than `size_in_data_bits`, check it is less than `Scalar::MODULUS`.
        if num_bits > size_in_data_bits {
            // Retrieve the modulus as we'll check `bits_le` is less than this value.
            let modulus = E::Scalar::modulus();

            // Recover the scalar as a `BigInteger` for comparison.
            // As `bits_le[size_in_bits..]` is guaranteed to be zero from the above logic,
            // and `bits_le` is greater than `size_in_data_bits`, it is safe to truncate `bits_le` to `size_in_bits`.
            let scalar = E::BigInteger::from_bits_le(&bits_le[..size_in_bits])?;

            // Ensure the scalar is less than `Scalar::MODULUS`.
            ensure!(scalar < modulus, "The scalar is greater than or equal to the modulus.");

            // Return the scalar.
            Ok(Scalar { scalar: E::Scalar::from_bigint(scalar).ok_or_else(|| anyhow!("Invalid scalar from bits"))? })
        } else {
            // Construct the sanitized list of bits padded with `false`
            let mut sanitized_bits = vec![false; size_in_bits];
            // Note: This is safe, because we just checked that the length of bits isn't bigger
            // than `size_in_data_bits` which is equal to `size_in_bits - 1`.
            sanitized_bits[..num_bits].copy_from_slice(bits_le);

            // Recover the native scalar.
            let scalar = E::Scalar::from_bigint(E::BigInteger::from_bits_le(&sanitized_bits)?)
                .ok_or_else(|| anyhow!("Invalid scalar from bits"))?;

            // Return the scalar.
            Ok(Scalar { scalar })
        }
    }

    /// Initializes a new scalar from a list of big-endian bits *without* leading zeros.
    fn from_bits_be(bits_be: &[bool]) -> Result<Self> {
        // Reverse the given bits from big-endian into little-endian.
        // Note: This is safe as the bit representation is consistent (there are no leading zeros).
        let mut bits_le = bits_be.to_vec();
        bits_le.reverse();

        Self::from_bits_le(&bits_le)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: usize = 100;

    fn check_from_bits_le() -> Result<()> {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected: Scalar<CurrentEnvironment> = Uniform::rand(&mut rng);
            let given_bits = expected.to_bits_le();
            assert_eq!(Scalar::<CurrentEnvironment>::size_in_bits(), given_bits.len());

            let candidate = Scalar::<CurrentEnvironment>::from_bits_le(&given_bits)?;
            assert_eq!(expected, candidate);

            // Add excess zero bits.
            let candidate = [given_bits, vec![false; i]].concat();

            let candidate = Scalar::<CurrentEnvironment>::from_bits_le(&candidate)?;
            assert_eq!(expected, candidate);
            assert_eq!(Scalar::<CurrentEnvironment>::size_in_bits(), candidate.to_bits_le().len());
        }
        Ok(())
    }

    fn check_from_bits_be() -> Result<()> {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected: Scalar<CurrentEnvironment> = Uniform::rand(&mut rng);
            let given_bits = expected.to_bits_be();
            assert_eq!(Scalar::<CurrentEnvironment>::size_in_bits(), given_bits.len());

            let candidate = Scalar::<CurrentEnvironment>::from_bits_be(&given_bits)?;
            assert_eq!(expected, candidate);

            // Add excess zero bits.
            let candidate = [vec![false; i], given_bits].concat();

            let candidate = Scalar::<CurrentEnvironment>::from_bits_be(&candidate)?;
            assert_eq!(expected, candidate);
            assert_eq!(Scalar::<CurrentEnvironment>::size_in_bits(), candidate.to_bits_be().len());
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
