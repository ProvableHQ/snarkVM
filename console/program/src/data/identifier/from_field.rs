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

impl<N: Network> FromField for Identifier<N> {
    type Field = Field<N>;

    /// Initializes a new identifier from a field element.
    fn from_field(field: &Self::Field) -> Result<Self> {
        // Convert the field to a list of little-endian bits.
        Self::from_bits_le(&field.to_bits_le())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::identifier::tests::sample_identifier;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: usize = 100;

    #[test]
    fn test_from_field() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random fixed-length alphanumeric identifier, that always starts with an alphabetic character.
            let identifier = sample_identifier::<CurrentNetwork>(&mut rng)?;
            assert_eq!(identifier, Identifier::from_field(&identifier.to_field()?)?);
        }
        Ok(())
    }
}
