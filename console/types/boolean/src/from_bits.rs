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

impl<E: Environment> FromBits for Boolean<E> {
    /// Initializes a new boolean by extracting the first bit from a list of length 1.
    fn from_bits_le(bits_le: &[bool]) -> Result<Self> {
        match bits_le.len().is_one() {
            true => Ok(Boolean::new(bits_le[0])),
            false => bail!("Boolean::from_bits_le expects a list of one boolean, found {}", bits_le.len()),
        }
    }

    /// Initializes a new boolean by extracting the first bit from a list of length 1.
    fn from_bits_be(bits_be: &[bool]) -> Result<Self> {
        match bits_be.len().is_one() {
            true => Ok(Boolean::new(bits_be[0])),
            false => bail!("Boolean::from_bits_be expects a list of one boolean, found {}", bits_be.len()),
        }
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

        for i in 1..ITERATIONS {
            // Sample a random element.
            let expected: Boolean<CurrentEnvironment> = Uniform::rand(&mut rng);
            let given_bits = expected.to_bits_le();
            assert_eq!(Boolean::<CurrentEnvironment>::size_in_bits(), given_bits.len());

            let candidate = Boolean::<CurrentEnvironment>::from_bits_le(&given_bits)?;
            assert_eq!(expected, candidate);

            // Add excess zero bits.
            let candidate = [given_bits, vec![false; i]].concat();
            assert!(Boolean::<CurrentEnvironment>::from_bits_le(&candidate).is_err());
        }
        Ok(())
    }

    fn check_from_bits_be() -> Result<()> {
        let mut rng = TestRng::default();

        for i in 1..ITERATIONS {
            // Sample a random element.
            let expected: Boolean<CurrentEnvironment> = Uniform::rand(&mut rng);
            let given_bits = expected.to_bits_be();
            assert_eq!(Boolean::<CurrentEnvironment>::size_in_bits(), given_bits.len());

            let candidate = Boolean::<CurrentEnvironment>::from_bits_be(&given_bits)?;
            assert_eq!(expected, candidate);

            // Add excess zero bits.
            let candidate = [vec![false; i], given_bits].concat();
            assert!(Boolean::<CurrentEnvironment>::from_bits_be(&candidate).is_err());
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
