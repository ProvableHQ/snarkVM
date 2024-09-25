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

impl<N: Network> ToField for Identifier<N> {
    type Field = Field<N>;

    /// Returns the identifier as a field element.
    fn to_field(&self) -> Result<Self::Field> {
        (&self).to_field()
    }
}

impl<N: Network> ToField for &Identifier<N> {
    type Field = Field<N>;

    /// Returns the identifier as a field element.
    fn to_field(&self) -> Result<Self::Field> {
        Ok(self.0)
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
    fn test_to_field() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random fixed-length alphanumeric string, that always starts with an alphabetic character.
            let expected_string = sample_identifier_as_string::<CurrentNetwork>(&mut rng)?;
            // Recover the field element from the bits.
            let expected_field = Field::<CurrentNetwork>::from_bits_le(&expected_string.to_bits_le())?;

            let candidate = Identifier::<CurrentNetwork>::from_str(&expected_string)?;
            assert_eq!(expected_field, candidate.to_field()?);
        }
        Ok(())
    }
}
