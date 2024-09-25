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

impl<E: Environment, const NUM_BITS: u8> HashUncompressed for Pedersen<E, NUM_BITS> {
    type Input = bool;
    type Output = Group<E>;

    /// Returns the Pedersen hash of the given input as a group element.
    fn hash_uncompressed(&self, input: &[Self::Input]) -> Result<Self::Output> {
        let mut input = Cow::Borrowed(input);
        match input.len() <= NUM_BITS as usize {
            // Pad the input if it is under the required parameter size.
            true => input.to_mut().resize(NUM_BITS as usize, false),
            // Ensure the input size is within the parameter size,
            false => bail!("Invalid input size for Pedersen: expected <= {NUM_BITS}, found {}", input.len()),
        }

        // Compute sum of h_i^{m_i} for all i.
        Ok(input
            .iter()
            .zip_eq(&*self.base_window)
            .flat_map(|(bit, base)| match bit {
                true => Some(*base),
                false => None,
            })
            .sum())
    }
}
