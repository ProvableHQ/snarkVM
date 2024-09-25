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

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> HashUncompressed
    for BHP<E, NUM_WINDOWS, WINDOW_SIZE>
{
    type Input = bool;
    type Output = Group<E>;

    /// Returns the BHP hash of the given input as an affine group element.
    ///
    /// This uncompressed variant of the BHP hash function is provided to support
    /// the BHP commitment scheme, as it is typically not used by applications.
    fn hash_uncompressed(&self, input: &[Self::Input]) -> Result<Self::Output> {
        // The number of hasher bits to fit.
        let num_hasher_bits = NUM_WINDOWS as usize * WINDOW_SIZE as usize * BHP_CHUNK_SIZE;
        // The number of data bits in the output.
        let num_data_bits = Field::<E>::size_in_data_bits();
        // The maximum number of input bits per iteration.
        let max_input_bits_per_iteration = num_hasher_bits - num_data_bits;

        debug_assert!(num_data_bits < num_hasher_bits);
        debug_assert_eq!(num_data_bits - 64, self.domain.len());

        // Initialize a variable to store the hash from the current iteration.
        let mut digest = Group::<E>::zero();

        // Prepare a reusable vector for the preimage.
        let mut preimage = Vec::with_capacity(num_hasher_bits);

        // Compute the hash of the input.
        for (i, input_bits) in input.chunks(max_input_bits_per_iteration).enumerate() {
            // Determine if this is the first iteration.
            match i == 0 {
                // Construct the first iteration as: [ 0...0 || DOMAIN || LENGTH(INPUT) || INPUT[0..BLOCK_SIZE] ].
                true => {
                    // Initialize a vector for the hash preimage.
                    preimage.extend(&self.domain);
                    (input.len() as u64).write_bits_le(&mut preimage);
                    preimage.extend(input_bits);
                }
                // Construct the subsequent iterations as: [ PREVIOUS_HASH[0..DATA_BITS] || INPUT[I * BLOCK_SIZE..(I + 1) * BLOCK_SIZE] ].
                false => {
                    // Initialize a vector for the hash preimage.
                    digest.to_x_coordinate().write_bits_le(&mut preimage);
                    preimage.truncate(num_data_bits);
                    preimage.extend(input_bits);
                }
            }
            // Hash the preimage for this iteration.
            digest = self.hasher.hash_uncompressed(&preimage)?;
            preimage.clear();
        }

        Ok(digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_types::environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 1000;

    #[test]
    fn test_bhp256_input_size() -> Result<()> {
        let bhp = BHP256::<CurrentEnvironment>::setup("BHPTest")?;

        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            let input = (0..bhp.window_size() as u64 + i).map(|_| bool::rand(&mut rng)).collect::<Vec<_>>();
            bhp.hash_uncompressed(&input)?;
        }
        Ok(())
    }

    #[test]
    fn test_bhp512_input_size() -> Result<()> {
        let bhp = BHP512::<CurrentEnvironment>::setup("BHPTest")?;

        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            let input = (0..bhp.window_size() as u64 + i).map(|_| bool::rand(&mut rng)).collect::<Vec<_>>();
            bhp.hash_uncompressed(&input)?;
        }
        Ok(())
    }

    #[test]
    fn test_bhp768_input_size() -> Result<()> {
        let bhp = BHP768::<CurrentEnvironment>::setup("BHPTest")?;

        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            let input = (0..bhp.window_size() as u64 + i).map(|_| bool::rand(&mut rng)).collect::<Vec<_>>();
            bhp.hash_uncompressed(&input)?;
        }
        Ok(())
    }

    #[test]
    fn test_bhp1024_input_size() -> Result<()> {
        let bhp = BHP1024::<CurrentEnvironment>::setup("BHPTest")?;

        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            let input = (0..bhp.window_size() as u64 + i).map(|_| bool::rand(&mut rng)).collect::<Vec<_>>();
            bhp.hash_uncompressed(&input)?;
        }
        Ok(())
    }
}
