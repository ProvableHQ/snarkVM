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

mod hasher;
use hasher::BHPHasher;

mod commit;
mod commit_uncompressed;
mod hash;
mod hash_uncompressed;

#[cfg(all(test, console))]
use snarkvm_circuit_types::environment::assert_scope;

use crate::{Commit, CommitUncompressed, Hash, HashUncompressed};
use snarkvm_circuit_types::prelude::*;

/// BHP256 is a collision-resistant hash function that processes inputs in 256-bit chunks.
pub type BHP256<E> = BHP<E, 3, 57>; // Supports inputs up to 261 bits (1 u8 + 1 Fq).
/// BHP512 is a collision-resistant hash function that processes inputs in 512-bit chunks.
pub type BHP512<E> = BHP<E, 6, 43>; // Supports inputs up to 522 bits (2 u8 + 2 Fq).
/// BHP768 is a collision-resistant hash function that processes inputs in 768-bit chunks.
pub type BHP768<E> = BHP<E, 15, 23>; // Supports inputs up to 783 bits (3 u8 + 3 Fq).
/// BHP1024 is a collision-resistant hash function that processes inputs in 1024-bit chunks.
pub type BHP1024<E> = BHP<E, 8, 54>; // Supports inputs up to 1044 bits (4 u8 + 4 Fq).

/// The BHP chunk size (this implementation is for a 3-bit BHP).
const BHP_CHUNK_SIZE: usize = 3;

/// BHP is a collision-resistant hash function that takes a variable-length input.
/// The BHP hash function does *not* behave like a random oracle, see Poseidon for one.
///
/// ## Design
/// The BHP hash function splits the given input into blocks, and processes them iteratively.
///
/// The first iteration is initialized as follows:
/// ```text
/// DIGEST_0 = BHP([ 0...0 || DOMAIN || LENGTH(INPUT) || INPUT[0..BLOCK_SIZE] ]);
/// ```
/// Each subsequent iteration is initialized as follows:
/// ```text
/// DIGEST_N+1 = BHP([ DIGEST_N[0..DATA_BITS] || INPUT[(N+1)*BLOCK_SIZE..(N+2)*BLOCK_SIZE] ]);
/// ```
pub struct BHP<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> {
    /// The domain separator for the BHP hash function.
    domain: Vec<Boolean<E>>,
    /// The internal BHP hasher used to process one iteration.
    hasher: BHPHasher<E, NUM_WINDOWS, WINDOW_SIZE>,
}

#[cfg(console)]
impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> Inject for BHP<E, NUM_WINDOWS, WINDOW_SIZE> {
    type Primitive = console::BHP<E::Network, NUM_WINDOWS, WINDOW_SIZE>;

    /// Initializes a new instance of a BHP circuit with the given BHP variant.
    fn new(_mode: Mode, bhp: Self::Primitive) -> Self {
        // Ensure the given domain is within the allowed size in bits.
        let num_bits = bhp.domain().len();
        let max_bits = E::BaseField::size_in_data_bits() - 64; // 64 bits encode the length.
        if num_bits > max_bits {
            E::halt(format!("Domain cannot exceed {max_bits} bits, found {num_bits} bits"))
        } else if num_bits != max_bits {
            E::halt(format!("Domain was not padded to {max_bits} bits, found {num_bits} bits"))
        }

        // Initialize the domain.
        let domain = Vec::constant(bhp.domain().to_vec());

        // Initialize the BHP hasher.
        let hasher = BHPHasher::<E, NUM_WINDOWS, WINDOW_SIZE>::constant(bhp);

        Self { domain, hasher }
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_types::environment::{Circuit, Eject};

    use anyhow::Result;

    const ITERATIONS: usize = 10;
    const MESSAGE: &str = "BHPCircuit0";

    #[test]
    fn test_setup_constant() -> Result<()> {
        for _ in 0..ITERATIONS {
            let native = console::BHP::<<Circuit as Environment>::Network, 8, 32>::setup(MESSAGE)?;
            let circuit = BHP::<Circuit, 8, 32>::new(Mode::Constant, native.clone());

            native.bases().iter().zip(circuit.hasher.bases().iter()).for_each(|(native_bases, circuit_bases)| {
                native_bases.iter().zip(circuit_bases).for_each(|(native_base, circuit_base_lookups)| {
                    // Check the first circuit base (when converted back to twisted Edwards) matches the native one.
                    let (circuit_x, circuit_y) = {
                        let (x_bases, y_bases) = circuit_base_lookups;
                        // Convert the Montgomery point to a twisted Edwards point.
                        let edwards_x = &x_bases[0] / &y_bases[0]; // 1 constraint
                        let edwards_y = (&x_bases[0] - Field::one()) / (&x_bases[0] + Field::one());
                        (edwards_x, edwards_y)
                    };
                    assert_eq!(native_base.to_x_coordinate(), circuit_x.eject_value());
                    assert_eq!(native_base.to_y_coordinate(), circuit_y.eject_value());
                })
            });
        }
        Ok(())
    }
}
