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

impl<E: Environment> FromBits for Address<E> {
    /// Initializes a new address by recovering the **x-coordinate** of an affine group from a list of **little-endian** bits.
    fn from_bits_le(bits_le: &[bool]) -> Result<Self> {
        Ok(Address::new(Group::from_bits_le(bits_le)?))
    }

    /// Initializes a new address by recovering the **x-coordinate** of an affine group from a list of **big-endian** bits.
    fn from_bits_be(bits_be: &[bool]) -> Result<Self> {
        Ok(Address::new(Group::from_bits_be(bits_be)?))
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
            let expected = Address::<CurrentEnvironment>::rand(&mut rng);

            let given_bits = expected.to_bits_le();
            assert_eq!(Address::<CurrentEnvironment>::size_in_bits(), given_bits.len());

            let candidate = Address::<CurrentEnvironment>::from_bits_le(&given_bits)?;
            assert_eq!(expected, candidate);

            // Add excess zero bits.
            let candidate = [given_bits, vec![false; i]].concat();

            let candidate = Address::<CurrentEnvironment>::from_bits_le(&candidate)?;
            assert_eq!(expected, candidate);
            assert_eq!(Group::<CurrentEnvironment>::size_in_bits(), candidate.to_bits_le().len());
        }
        Ok(())
    }

    fn check_from_bits_be() -> Result<()> {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected = Address::<CurrentEnvironment>::rand(&mut rng);

            let given_bits = expected.to_bits_be();
            assert_eq!(Address::<CurrentEnvironment>::size_in_bits(), given_bits.len());

            let candidate = Address::<CurrentEnvironment>::from_bits_be(&given_bits)?;
            assert_eq!(expected, candidate);

            // Add excess zero bits.
            let candidate = [vec![false; i], given_bits].concat();

            let candidate = Address::<CurrentEnvironment>::from_bits_be(&candidate)?;
            assert_eq!(expected, candidate);
            assert_eq!(Group::<CurrentEnvironment>::size_in_bits(), candidate.to_bits_be().len());
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
