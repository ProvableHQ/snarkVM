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

impl<N: Network> Identifier<N> {
    /// Returns the number of bits of this identifier.
    pub fn size_in_bits(&self) -> u8 {
        // The size in bits always fits in a u8, as the underlying representation is a field element.
        debug_assert!(Field::<N>::size_in_data_bits() <= u8::MAX as usize);
        // Convert the size to bits (as a byte-aligned multiple).
        8 * self.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::identifier::tests::sample_identifier_as_string;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: usize = 100;

    #[test]
    fn test_size_in_bits() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random fixed-length alphanumeric string, that always starts with an alphabetic character.
            let expected_string = sample_identifier_as_string::<CurrentNetwork>(&mut rng)?;

            let candidate = Identifier::<CurrentNetwork>::from_str(&expected_string)?;
            assert_eq!(expected_string.len() * 8, candidate.size_in_bits() as usize);
        }
        Ok(())
    }
}
