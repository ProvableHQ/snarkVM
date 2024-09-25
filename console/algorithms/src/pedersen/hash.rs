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

impl<E: Environment, const NUM_BITS: u8> Hash for Pedersen<E, NUM_BITS> {
    type Input = bool;
    type Output = Field<E>;

    /// Returns the Pedersen hash of the given input as a field element.
    fn hash(&self, input: &[Self::Input]) -> Result<Self::Output> {
        // Compute the Pedersen hash as an affine group element, and return the x-coordinate.
        Ok(self.hash_uncompressed(input)?.to_x_coordinate())
    }
}
