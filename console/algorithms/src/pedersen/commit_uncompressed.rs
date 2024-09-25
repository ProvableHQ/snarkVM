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

impl<E: Environment, const NUM_BITS: u8> CommitUncompressed for Pedersen<E, NUM_BITS> {
    type Input = bool;
    type Output = Group<E>;
    type Randomizer = Scalar<E>;

    /// Returns the Pedersen commitment of the given input and randomizer as a group element.
    fn commit_uncompressed(&self, input: &[Self::Input], randomizer: &Self::Randomizer) -> Result<Self::Output> {
        let mut output = self.hash_uncompressed(input)?;

        // Compute h^r.
        randomizer.to_bits_le().iter().zip_eq(&*self.random_base_window).filter(|(bit, _)| **bit).for_each(
            |(_, base)| {
                output += base;
            },
        );

        Ok(output)
    }
}
