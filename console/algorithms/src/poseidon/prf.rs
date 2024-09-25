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

impl<E: Environment, const RATE: usize> PRF for Poseidon<E, RATE> {
    type Input = Field<E>;
    type Output = Field<E>;
    type Seed = Field<E>;

    #[inline]
    fn prf(&self, seed: &Self::Seed, input: &[Self::Input]) -> Result<Self::Output> {
        // Construct the preimage: seed || input.
        let mut preimage = Vec::with_capacity(1 + input.len());
        preimage.push(*seed);
        preimage.extend_from_slice(input);

        // Hash the preimage to derive the PRF output.
        self.hash(&preimage)
    }
}
