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

use std::borrow::Cow;

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> HashUncompressed
    for BHPHasher<E, NUM_WINDOWS, WINDOW_SIZE>
{
    type Input = bool;
    type Output = Group<E>;

    /// Returns the BHP hash of the given input as an affine group element.
    ///
    /// This uncompressed variant of the BHP hash function is provided to support
    /// the BHP commitment scheme, as it is typically not used by applications.
    fn hash_uncompressed(&self, input: &[Self::Input]) -> Result<Self::Output> {
        // Ensure the input size is at least the window size.
        ensure!(input.len() > Self::MIN_BITS, "Inputs to this BHP must be greater than {} bits", Self::MIN_BITS);
        // Ensure the input size is within the parameter size,
        ensure!(
            input.len() <= Self::MAX_BITS,
            "Inputs to this BHP cannot exceed {} bits, found {}",
            Self::MAX_BITS,
            input.len()
        );

        // Pad the input to a multiple of `BHP_CHUNK_SIZE` for hashing.
        let input = if input.len() % BHP_CHUNK_SIZE != 0 {
            let padding = BHP_CHUNK_SIZE - (input.len() % BHP_CHUNK_SIZE);
            let mut padded_input = vec![false; input.len() + padding];
            padded_input[..input.len()].copy_from_slice(input);
            ensure!((padded_input.len() % BHP_CHUNK_SIZE) == 0, "Input must be a multiple of {BHP_CHUNK_SIZE}");
            Cow::Owned(padded_input)
        } else {
            Cow::Borrowed(input)
        };

        // Compute sum of h_i^{sum of (1-2*c_{i,j,2})*(1+c_{i,j,0}+2*c_{i,j,1})*2^{4*(j-1)} for all j in segment}
        // for all i. Described in section 5.4.1.7 in the Zcash protocol specification.
        //
        // Note: `.zip()` is used here (as opposed to `.zip_eq()`) as the input can be less than
        // `NUM_WINDOWS * WINDOW_SIZE * BHP_CHUNK_SIZE` in length, which is the parameter size here.
        Ok(input
            .chunks(WINDOW_SIZE as usize * BHP_CHUNK_SIZE)
            .zip(&*self.bases_lookup)
            .flat_map(|(bits, bases)| {
                bits.chunks(BHP_CHUNK_SIZE).zip(bases).map(|(chunk_bits, base)| {
                    base[(chunk_bits[0] as usize) | (chunk_bits[1] as usize) << 1 | (chunk_bits[2] as usize) << 2]
                })
            })
            .sum())
    }
}
