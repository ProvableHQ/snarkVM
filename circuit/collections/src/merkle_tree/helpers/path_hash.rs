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
use snarkvm_circuit_algorithms::{Hash, Poseidon, BHP};

/// A trait for a Merkle path hash function.
pub trait PathHash<E: Environment> {
    type Hash: FieldTrait;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, left: &Self::Hash, right: &Self::Hash) -> Self::Hash;

    /// Returns the empty hash.
    fn hash_empty(&self) -> Self::Hash {
        self.hash_children(&Self::Hash::zero(), &Self::Hash::zero())
    }
}

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> PathHash<E> for BHP<E, NUM_WINDOWS, WINDOW_SIZE> {
    type Hash = Field<E>;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, left: &Self::Hash, right: &Self::Hash) -> Self::Hash {
        let mut input = Vec::new();
        // Prepend the nodes with a `true` bit.
        input.push(Boolean::constant(true));
        left.write_bits_le(&mut input);
        right.write_bits_le(&mut input);
        // Hash the input.
        Hash::hash(self, &input)
    }
}

impl<E: Environment, const RATE: usize> PathHash<E> for Poseidon<E, RATE> {
    type Hash = Field<E>;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, left: &Self::Hash, right: &Self::Hash) -> Self::Hash {
        // Prepend the nodes with a `1field` byte.
        let input = &[Self::Hash::one(), left.clone(), right.clone()];
        // Hash the input.
        Hash::hash(self, input)
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_algorithms::{Poseidon2, BHP512};
    use snarkvm_circuit_types::environment::Circuit;
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    const ITERATIONS: u64 = 10;
    const DOMAIN: &str = "MerkleTreeCircuit0";

    macro_rules! check_hash_children {
        ($hash:ident, $mode:ident, ($num_constants:expr, $num_public:expr, $num_private:expr, $num_constraints:expr)) => {{
            // Initialize the hash.
            let native = snarkvm_console_algorithms::$hash::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
            let circuit = $hash::<Circuit>::constant(native.clone());

            let mut rng = TestRng::default();

            for i in 0..ITERATIONS {
                // Sample a random input.
                let left = Uniform::rand(&mut rng);
                let right = Uniform::rand(&mut rng);

                // Compute the expected hash.
                let expected = console::merkle_tree::PathHash::hash_children(&native, &left, &right)?;

                // Prepare the circuit input.
                let left = Field::new(Mode::$mode, left);
                let right = Field::new(Mode::$mode, right);

                Circuit::scope(format!("PathHash {i}"), || {
                    // Perform the hash operation.
                    let candidate = circuit.hash_children(&left, &right);
                    assert_scope!($num_constants, $num_public, $num_private, $num_constraints);
                    assert_eq!(expected, candidate.eject_value());
                });
                Circuit::reset();
            }
            Ok::<_, anyhow::Error>(())
        }};
    }

    #[test]
    fn test_hash_children_bhp512_constant() -> Result<()> {
        check_hash_children!(BHP512, Constant, (1599, 0, 0, 0))
    }

    #[test]
    fn test_hash_children_bhp512_public() -> Result<()> {
        check_hash_children!(BHP512, Public, (409, 0, 1879, 1883))
    }

    #[test]
    fn test_hash_children_bhp512_private() -> Result<()> {
        check_hash_children!(BHP512, Private, (409, 0, 1879, 1883))
    }

    #[test]
    fn test_hash_children_poseidon2_constant() -> Result<()> {
        check_hash_children!(Poseidon2, Constant, (1, 0, 0, 0))
    }

    #[test]
    fn test_hash_children_poseidon2_public() -> Result<()> {
        check_hash_children!(Poseidon2, Public, (1, 0, 540, 540))
    }

    #[test]
    fn test_hash_children_poseidon2_private() -> Result<()> {
        check_hash_children!(Poseidon2, Private, (1, 0, 540, 540))
    }
}
