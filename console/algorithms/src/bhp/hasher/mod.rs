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

mod hash_uncompressed;

use crate::Blake2Xs;
use snarkvm_console_types::prelude::*;
use snarkvm_utilities::BigInteger;

use std::sync::Arc;

/// The BHP chunk size (this implementation is for a 3-bit BHP).
pub(super) const BHP_CHUNK_SIZE: usize = 3;
pub(super) const BHP_LOOKUP_SIZE: usize = 1 << BHP_CHUNK_SIZE;

/// BHP is a collision-resistant hash function that takes a variable-length input.
/// The BHP hasher is used to process one internal iteration of the BHP hash function.
#[derive(Clone, Debug, PartialEq)]
pub struct BHPHasher<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> {
    /// The bases for the BHP hash.
    bases: Arc<Vec<Vec<Group<E>>>>,
    /// The bases lookup table for the BHP hash.
    bases_lookup: Arc<Vec<Vec<[Group<E>; BHP_LOOKUP_SIZE]>>>,
    /// The random base for the BHP commitment.
    random_base: Arc<Vec<Group<E>>>,
}

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> BHPHasher<E, NUM_WINDOWS, WINDOW_SIZE> {
    /// The maximum number of input bits.
    const MAX_BITS: usize = NUM_WINDOWS as usize * WINDOW_SIZE as usize * BHP_CHUNK_SIZE;
    /// The minimum number of input bits (at least one window).
    const MIN_BITS: usize = WINDOW_SIZE as usize * BHP_CHUNK_SIZE;

    /// Initializes a new instance of BHP with the given domain.
    pub fn setup(domain: &str) -> Result<Self> {
        // Calculate the maximum window size.
        let mut maximum_window_size = 0;
        let mut range = E::BigInteger::from(2_u64);
        while range < E::Scalar::modulus_minus_one_div_two() {
            // range < (p-1)/2
            range.muln(4); // range * 2^4
            maximum_window_size += 1;
        }
        ensure!(WINDOW_SIZE <= maximum_window_size, "The maximum BHP window size is {maximum_window_size}");

        // Compute the bases.
        let bases = (0..NUM_WINDOWS)
            .map(|index| {
                // Construct an indexed message to attempt to sample a base.
                let (generator, _, _) = Blake2Xs::hash_to_curve::<E::Affine>(&format!(
                    "Aleo.BHP.{NUM_WINDOWS}.{WINDOW_SIZE}.{domain}.{index}"
                ));
                let mut base = Group::<E>::new(generator);
                // Compute the generators for the sampled base.
                let mut powers = Vec::with_capacity(WINDOW_SIZE as usize);
                for _ in 0..WINDOW_SIZE {
                    powers.push(base);
                    for _ in 0..4 {
                        base = base.double();
                    }
                }
                powers
            })
            .collect::<Vec<Vec<Group<E>>>>();
        ensure!(bases.len() == NUM_WINDOWS as usize, "Incorrect number of BHP windows ({})", bases.len());
        for window in &bases {
            ensure!(window.len() == WINDOW_SIZE as usize, "Incorrect BHP window size ({})", window.len());
        }

        // Compute the bases lookup.
        let bases_lookup = bases
            .iter()
            .map(|x| {
                x.iter()
                    .map(|g| {
                        let mut lookup = [Group::<E>::zero(); BHP_LOOKUP_SIZE];
                        for (i, element) in lookup.iter_mut().enumerate().take(BHP_LOOKUP_SIZE) {
                            *element = *g;
                            if (i & 0x01) != 0 {
                                *element += g;
                            }
                            if (i & 0x02) != 0 {
                                *element += g.double();
                            }
                            if (i & 0x04) != 0 {
                                *element = element.neg();
                            }
                        }
                        lookup
                    })
                    .collect()
            })
            .collect::<Vec<Vec<[Group<E>; BHP_LOOKUP_SIZE]>>>();
        ensure!(bases_lookup.len() == NUM_WINDOWS as usize, "Incorrect number of BHP lookups ({})", bases_lookup.len());
        for window in &bases_lookup {
            ensure!(window.len() == WINDOW_SIZE as usize, "Incorrect BHP lookup window size ({})", window.len());
        }

        // Next, compute the random base.
        let (generator, _, _) =
            Blake2Xs::hash_to_curve::<E::Affine>(&format!("Aleo.BHP.{NUM_WINDOWS}.{WINDOW_SIZE}.{domain}.Randomizer"));
        let mut base_power = Group::<E>::new(generator);
        let mut random_base = Vec::with_capacity(Scalar::<E>::size_in_bits());
        for _ in 0..Scalar::<E>::size_in_bits() {
            random_base.push(base_power);
            base_power = base_power.double();
        }
        ensure!(
            random_base.len() == Scalar::<E>::size_in_bits(),
            "Incorrect number of BHP random base powers ({})",
            random_base.len()
        );

        Ok(Self { bases: Arc::new(bases), bases_lookup: Arc::new(bases_lookup), random_base: Arc::new(random_base) })
    }

    /// Returns the bases.
    pub fn bases(&self) -> &Arc<Vec<Vec<Group<E>>>> {
        &self.bases
    }

    /// Returns the random base window.
    pub fn random_base(&self) -> &Arc<Vec<Group<E>>> {
        &self.random_base
    }
}
