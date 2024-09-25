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

/// A trait for a Merkle leaf hash function.
pub trait LeafHash<E: Environment> {
    type Leaf;
    type Hash: FieldTrait;

    /// Returns the hash of the given leaf node.
    fn hash_leaf(&self, leaf: &Self::Leaf) -> Self::Hash;
}

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> LeafHash<E> for BHP<E, NUM_WINDOWS, WINDOW_SIZE> {
    type Hash = Field<E>;
    type Leaf = Vec<Boolean<E>>;

    /// Returns the hash of the given leaf node.
    fn hash_leaf(&self, leaf: &Self::Leaf) -> Self::Hash {
        let mut input = Vec::with_capacity(1 + leaf.len());
        // Prepend the leaf with a `false` bit.
        input.push(Boolean::constant(false));
        input.extend_from_slice(leaf);
        // Hash the input.
        Hash::hash(self, &input)
    }
}

impl<E: Environment, const RATE: usize> LeafHash<E> for Poseidon<E, RATE> {
    type Hash = Field<E>;
    type Leaf = Vec<Field<E>>;

    /// Returns the hash of the given leaf node.
    fn hash_leaf(&self, leaf: &Self::Leaf) -> Self::Hash {
        let mut input = Vec::with_capacity(1 + leaf.len());
        // Prepend the leaf with a `0field` element.
        input.push(Self::Hash::zero());
        input.extend_from_slice(leaf);
        // Hash the input.
        Hash::hash(self, &input)
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_algorithms::{Poseidon4, BHP1024};
    use snarkvm_circuit_types::environment::Circuit;
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    const ITERATIONS: u64 = 10;
    const DOMAIN: &str = "MerkleTreeCircuit0";

    macro_rules! check_hash_leaf {
        ($hash:ident, $mode:ident, $num_inputs:expr, ($num_constants:expr, $num_public:expr, $num_private:expr, $num_constraints:expr)) => {{
            // Initialize the hash.
            let native = snarkvm_console_algorithms::$hash::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
            let circuit = $hash::<Circuit>::constant(native.clone());

            let mut rng = TestRng::default();

            for i in 0..ITERATIONS {
                // Sample a random input.
                let input = (0..$num_inputs).map(|_| Uniform::rand(&mut rng)).collect::<Vec<_>>();

                // Compute the expected hash.
                let expected = console::merkle_tree::LeafHash::hash_leaf(&native, &input)?;

                // Prepare the circuit input.
                let circuit_input: Vec<_> = Inject::new(Mode::$mode, input);

                Circuit::scope(format!("LeafHash {i}"), || {
                    // Perform the hash operation.
                    let candidate = circuit.hash_leaf(&circuit_input);
                    assert_scope!($num_constants, $num_public, $num_private, $num_constraints);
                    assert_eq!(expected, candidate.eject_value());
                });
                Circuit::reset();
            }
            Ok::<_, anyhow::Error>(())
        }};
    }

    #[test]
    fn test_hash_leaf_bhp1024_constant() -> Result<()> {
        check_hash_leaf!(BHP1024, Constant, 1024, (1791, 0, 0, 0))
    }

    #[test]
    fn test_hash_leaf_bhp1024_public() -> Result<()> {
        check_hash_leaf!(BHP1024, Public, 1024, (413, 0, 1744, 1744))
    }

    #[test]
    fn test_hash_leaf_bhp1024_private() -> Result<()> {
        check_hash_leaf!(BHP1024, Private, 1024, (413, 0, 1744, 1744))
    }

    #[test]
    fn test_hash_leaf_poseidon4_constant() -> Result<()> {
        check_hash_leaf!(Poseidon4, Constant, 4, (1, 0, 0, 0))
    }

    #[test]
    fn test_hash_leaf_poseidon4_public() -> Result<()> {
        check_hash_leaf!(Poseidon4, Public, 4, (1, 0, 700, 700))
    }

    #[test]
    fn test_hash_leaf_poseidon4_private() -> Result<()> {
        check_hash_leaf!(Poseidon4, Private, 4, (1, 0, 700, 700))
    }
}
