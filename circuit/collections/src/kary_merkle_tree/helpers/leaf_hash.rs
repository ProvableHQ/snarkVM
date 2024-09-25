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
use snarkvm_circuit_algorithms::{Hash, Keccak, Poseidon, BHP};

/// A trait for a Merkle leaf hash function.
pub trait LeafHash {
    type Hash: Default + Inject + Eject + Ternary;
    type Leaf;

    /// Returns the hash of the given leaf node.
    fn hash_leaf(&self, leaf: &Self::Leaf) -> Self::Hash;
}

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> LeafHash for BHP<E, NUM_WINDOWS, WINDOW_SIZE> {
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

impl<E: Environment, const RATE: usize> LeafHash for Poseidon<E, RATE> {
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

impl<E: Environment, const TYPE: u8, const VARIANT: usize> LeafHash for Keccak<E, TYPE, VARIANT> {
    type Hash = BooleanHash<E, VARIANT>;
    type Leaf = Vec<Boolean<E>>;

    /// Returns the hash of the given leaf node.
    fn hash_leaf(&self, leaf: &Self::Leaf) -> Self::Hash {
        let mut input = Vec::with_capacity(1 + leaf.len());
        // Prepend the leaf with a `false` bit.
        input.push(Boolean::constant(false));
        input.extend_from_slice(leaf);
        // Hash the input.
        let output = Hash::hash(self, &input);
        // Read the first VARIANT bits.
        let mut result = BooleanHash::default();
        result.0.clone_from_slice(&output[..VARIANT]);
        result
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_algorithms::{Keccak256, Poseidon4, Sha3_256, BHP1024};
    use snarkvm_circuit_types::environment::Circuit;
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    const ITERATIONS: u64 = 10;
    const DOMAIN: &str = "MerkleTreeCircuit0";

    macro_rules! check_hash_leaf {
        ($native:ident, $circuit:ident, $mode:ident, $num_inputs:expr, ($num_constants:expr, $num_public:expr, $num_private:expr, $num_constraints:expr)) => {{
            let mut rng = TestRng::default();

            for i in 0..ITERATIONS {
                // Sample a random input.
                let input = (0..$num_inputs).map(|_| Uniform::rand(&mut rng)).collect::<Vec<_>>();

                // Compute the expected hash.
                let expected = console::kary_merkle_tree::LeafHash::hash_leaf(&$native, &input)?;

                // Prepare the circuit input.
                let circuit_input: Vec<_> = Inject::new(Mode::$mode, input);

                Circuit::scope(format!("LeafHash {i}"), || {
                    // Perform the hash operation.
                    let candidate = $circuit.hash_leaf(&circuit_input);
                    assert_scope!($num_constants, $num_public, $num_private, $num_constraints);
                    assert_eq!(expected, candidate.eject_value());
                });
                Circuit::reset();
            }
            Ok::<_, anyhow::Error>(())
        }};
        ($hash:ident, $mode:ident, $num_inputs:expr, ($num_constants:expr, $num_public:expr, $num_private:expr, $num_constraints:expr)) => {{
            // Initialize the hash.
            let native = snarkvm_console_algorithms::$hash::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
            let circuit = $hash::<Circuit>::constant(native.clone());

            check_hash_leaf!(
                native,
                circuit,
                $mode,
                $num_inputs,
                ($num_constants, $num_public, $num_private, $num_constraints)
            )
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

    #[test]
    fn test_hash_leaf_keccak256_constant() -> Result<()> {
        let native = snarkvm_console_algorithms::Keccak256 {};
        let circuit = Keccak256::<Circuit>::new();

        check_hash_leaf!(native, circuit, Constant, 1024, (256, 0, 0, 0))
    }

    #[test]
    fn test_hash_leaf_keccak256_public() -> Result<()> {
        let native = snarkvm_console_algorithms::Keccak256 {};
        let circuit = Keccak256::<Circuit>::new();

        check_hash_leaf!(native, circuit, Public, 1024, (256, 0, 152448, 152448))
    }

    #[test]
    fn test_hash_leaf_keccak256_private() -> Result<()> {
        let native = snarkvm_console_algorithms::Keccak256 {};
        let circuit = Keccak256::<Circuit>::new();

        check_hash_leaf!(native, circuit, Private, 1024, (256, 0, 152448, 152448))
    }

    #[test]
    fn test_hash_leaf_sha3_256_constant() -> Result<()> {
        let native = snarkvm_console_algorithms::Sha3_256 {};
        let circuit = Sha3_256::<Circuit>::new();

        check_hash_leaf!(native, circuit, Constant, 1024, (256, 0, 0, 0))
    }

    #[test]
    fn test_hash_leaf_sha3_256_public() -> Result<()> {
        let native = snarkvm_console_algorithms::Sha3_256 {};
        let circuit = Sha3_256::<Circuit>::new();

        check_hash_leaf!(native, circuit, Public, 1024, (256, 0, 152448, 152448))
    }

    #[test]
    fn test_hash_leaf_sha3_256_private() -> Result<()> {
        let native = snarkvm_console_algorithms::Sha3_256 {};
        let circuit = Sha3_256::<Circuit>::new();

        check_hash_leaf!(native, circuit, Private, 1024, (256, 0, 152448, 152448))
    }
}
