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

mod hash;
mod hash_many;
mod hash_to_group;
mod hash_to_scalar;
mod prf;

#[cfg(all(test, console))]
use snarkvm_circuit_types::environment::assert_scope;
#[cfg(test)]
use snarkvm_utilities::{TestRng, Uniform};

use crate::{Elligator2, Hash, HashMany, HashToGroup, HashToScalar, PRF};
use snarkvm_circuit_types::{environment::prelude::*, Field, Group, Scalar};

/// Poseidon2 is a cryptographic hash function of input rate 2.
pub type Poseidon2<E> = Poseidon<E, 2>;
/// Poseidon4 is a cryptographic hash function of input rate 4.
pub type Poseidon4<E> = Poseidon<E, 4>;
/// Poseidon8 is a cryptographic hash function of input rate 8.
pub type Poseidon8<E> = Poseidon<E, 8>;

const CAPACITY: usize = 1;

/// The mode structure for duplex sponges.
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum DuplexSpongeMode {
    /// The sponge is currently absorbing data.
    Absorbing {
        /// The next position of the state to be XOR-ed when absorbing.
        next_absorb_index: usize,
    },
    /// The sponge is currently squeezing data out.
    Squeezing {
        /// The next position of the state to be outputted when squeezing.
        next_squeeze_index: usize,
    },
}

#[derive(Clone)]
pub struct Poseidon<E: Environment, const RATE: usize> {
    /// The domain separator for the Poseidon hash function.
    domain: Field<E>,
    /// The number of rounds in a full-round operation.
    full_rounds: usize,
    /// The number of rounds in a partial-round operation.
    partial_rounds: usize,
    /// The exponent used in S-boxes.
    alpha: Field<E>,
    /// The additive round keys. These are added before each MDS matrix application to make it an affine shift.
    /// They are indexed by `ark[round_number][state_element_index]`
    ark: Vec<Vec<Field<E>>>,
    /// The Maximally Distance Separating (MDS) matrix.
    mds: Vec<Vec<Field<E>>>,
}

#[cfg(console)]
impl<E: Environment, const RATE: usize> Inject for Poseidon<E, RATE> {
    type Primitive = console::Poseidon<E::Network, RATE>;

    fn new(_mode: Mode, poseidon: Self::Primitive) -> Self {
        // Initialize the domain separator.
        let domain = Field::constant(poseidon.domain());

        // Initialize the Poseidon parameters.
        let parameters = poseidon.parameters();
        let full_rounds = parameters.full_rounds;
        let partial_rounds = parameters.partial_rounds;
        let alpha = Field::constant(console::Field::from_u128(parameters.alpha as u128));
        // Cache the bits for the field element.
        alpha.to_bits_le();
        let ark = parameters
            .ark
            .iter()
            .take(full_rounds + partial_rounds)
            .map(|round| {
                round.iter().take(RATE + 1).copied().map(|field| Field::constant(console::Field::new(field))).collect()
            })
            .collect();
        let mds = parameters
            .mds
            .iter()
            .take(RATE + 1)
            .map(|round| {
                round.iter().take(RATE + 1).copied().map(|field| Field::constant(console::Field::new(field))).collect()
            })
            .collect();

        Self { domain, full_rounds, partial_rounds, alpha, ark, mds }
    }
}
