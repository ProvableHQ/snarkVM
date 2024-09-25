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

impl<E: Environment, I: IntegerType> FromBits for Integer<E, I> {
    /// Initializes a new integer from a list of **little-endian** bits.
    fn from_bits_le(bits_le: &[bool]) -> Result<Self> {
        Ok(Self::new(I::from_bits_le(bits_le)?))
    }

    /// Initializes a new integer from a list of **big-endian** bits.
    fn from_bits_be(bits_be: &[bool]) -> Result<Self> {
        Ok(Self::new(I::from_bits_be(bits_be)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: usize = 100;

    fn check_from_bits_le<I: IntegerType>(rng: &mut TestRng) -> Result<()> {
        for i in 0..ITERATIONS {
            // Sample a random value.
            let expected: I = Uniform::rand(rng);

            let expected = Integer::<CurrentEnvironment, I>::new(expected);
            let given_bits = expected.to_bits_le();
            assert_eq!(Integer::<CurrentEnvironment, I>::size_in_bits(), given_bits.len());

            let candidate = Integer::<CurrentEnvironment, I>::from_bits_le(&given_bits)?;
            assert_eq!(expected, candidate);

            // Add excess zero bits.
            let candidate = [given_bits, vec![false; i]].concat();

            let candidate = Integer::<CurrentEnvironment, I>::from_bits_le(&candidate)?;
            assert_eq!(expected, candidate);
            assert_eq!(Integer::<CurrentEnvironment, I>::size_in_bits(), candidate.to_bits_le().len());
        }
        Ok(())
    }

    fn check_from_bits_be<I: IntegerType>(rng: &mut TestRng) -> Result<()> {
        for i in 0..ITERATIONS {
            // Sample a random value.
            let expected: I = Uniform::rand(rng);

            let expected = Integer::<CurrentEnvironment, I>::new(expected);
            let given_bits = expected.to_bits_be();
            assert_eq!(Integer::<CurrentEnvironment, I>::size_in_bits(), given_bits.len());

            let candidate = Integer::<CurrentEnvironment, I>::from_bits_be(&given_bits)?;
            assert_eq!(expected, candidate);

            // Add excess zero bits.
            let candidate = [vec![false; i], given_bits].concat();

            let candidate = Integer::<CurrentEnvironment, I>::from_bits_be(&candidate)?;
            assert_eq!(expected, candidate);
            assert_eq!(Integer::<CurrentEnvironment, I>::size_in_bits(), candidate.to_bits_be().len());
        }
        Ok(())
    }

    #[test]
    fn test_from_bits_le() -> Result<()> {
        let mut rng = TestRng::default();

        check_from_bits_le::<u8>(&mut rng)?;
        check_from_bits_le::<u16>(&mut rng)?;
        check_from_bits_le::<u32>(&mut rng)?;
        check_from_bits_le::<u64>(&mut rng)?;
        check_from_bits_le::<u128>(&mut rng)?;

        check_from_bits_le::<i8>(&mut rng)?;
        check_from_bits_le::<i16>(&mut rng)?;
        check_from_bits_le::<i32>(&mut rng)?;
        check_from_bits_le::<i64>(&mut rng)?;
        check_from_bits_le::<i128>(&mut rng)?;

        Ok(())
    }

    #[test]
    fn test_from_bits_be() -> Result<()> {
        let mut rng = TestRng::default();

        check_from_bits_be::<u8>(&mut rng)?;
        check_from_bits_be::<u16>(&mut rng)?;
        check_from_bits_be::<u32>(&mut rng)?;
        check_from_bits_be::<u64>(&mut rng)?;
        check_from_bits_be::<u128>(&mut rng)?;

        check_from_bits_be::<i8>(&mut rng)?;
        check_from_bits_be::<i16>(&mut rng)?;
        check_from_bits_be::<i32>(&mut rng)?;
        check_from_bits_be::<i64>(&mut rng)?;
        check_from_bits_be::<i128>(&mut rng)?;

        Ok(())
    }
}
