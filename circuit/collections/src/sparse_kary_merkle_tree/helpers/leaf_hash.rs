// Copyright (c) 2019-2025 Provable Inc.
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
use snarkvm_circuit_algorithms::{BHP, Hash, Keccak, Poseidon};

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
        // Prepend the leaf with 2 `false` bits.
        input.push(Boolean::constant(false));
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
        // Prepend the leaf with 2 `false` bits.
        input.push(Boolean::constant(false));
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

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_algorithms::{BHP1024, Poseidon4};
    use snarkvm_circuit_types::environment::Circuit;
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    const ITERATIONS: u64 = 10;
    const DOMAIN: &str = "SparseTreeCircuit0";

    #[test]
    fn test_hash_leaf_bhp1024_constant() -> Result<()> {
        let native = snarkvm_console_algorithms::BHP1024::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
        let circuit = BHP1024::<Circuit>::constant(native.clone());

        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let leaf = console::Field::<<Circuit as Environment>::Network>::rand(&mut rng).to_bits_le();
            let expected = console::sparse_kary_merkle_tree::LeafHash::hash_leaf(&native, &leaf)?;

            Circuit::scope("LeafHash BHP", || {
                let circuit_leaf: Vec<_> = Inject::new(Mode::Constant, leaf);
                let candidate = circuit.hash_leaf(&circuit_leaf);
                assert_eq!(expected, candidate.eject_value());
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_hash_leaf_bhp1024_public() -> Result<()> {
        let native = snarkvm_console_algorithms::BHP1024::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
        let circuit = BHP1024::<Circuit>::constant(native.clone());

        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let leaf = console::Field::<<Circuit as Environment>::Network>::rand(&mut rng).to_bits_le();
            let expected = console::sparse_kary_merkle_tree::LeafHash::hash_leaf(&native, &leaf)?;

            Circuit::scope("LeafHash BHP", || {
                let circuit_leaf: Vec<_> = Inject::new(Mode::Public, leaf);
                let candidate = circuit.hash_leaf(&circuit_leaf);
                assert_eq!(expected, candidate.eject_value());
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_hash_leaf_bhp1024_private() -> Result<()> {
        let native = snarkvm_console_algorithms::BHP1024::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
        let circuit = BHP1024::<Circuit>::constant(native.clone());

        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let leaf = console::Field::<<Circuit as Environment>::Network>::rand(&mut rng).to_bits_le();
            let expected = console::sparse_kary_merkle_tree::LeafHash::hash_leaf(&native, &leaf)?;

            Circuit::scope("LeafHash BHP", || {
                let circuit_leaf: Vec<_> = Inject::new(Mode::Private, leaf);
                let candidate = circuit.hash_leaf(&circuit_leaf);
                assert_eq!(expected, candidate.eject_value());
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_hash_leaf_poseidon4_constant() -> Result<()> {
        let native = snarkvm_console_algorithms::Poseidon4::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
        let circuit = Poseidon4::<Circuit>::constant(native.clone());

        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let leaf = vec![Uniform::rand(&mut rng)];
            let expected = console::sparse_kary_merkle_tree::LeafHash::hash_leaf(&native, &leaf)?;

            Circuit::scope("LeafHash Poseidon", || {
                let circuit_leaf: Vec<_> = Inject::new(Mode::Constant, leaf);
                let candidate = circuit.hash_leaf(&circuit_leaf);
                assert_eq!(expected, candidate.eject_value());
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_hash_leaf_poseidon4_public() -> Result<()> {
        let native = snarkvm_console_algorithms::Poseidon4::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
        let circuit = Poseidon4::<Circuit>::constant(native.clone());

        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let leaf = vec![Uniform::rand(&mut rng)];
            let expected = console::sparse_kary_merkle_tree::LeafHash::hash_leaf(&native, &leaf)?;

            Circuit::scope("LeafHash Poseidon", || {
                let circuit_leaf: Vec<_> = Inject::new(Mode::Public, leaf);
                let candidate = circuit.hash_leaf(&circuit_leaf);
                assert_eq!(expected, candidate.eject_value());
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_hash_leaf_poseidon4_private() -> Result<()> {
        let native = snarkvm_console_algorithms::Poseidon4::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
        let circuit = Poseidon4::<Circuit>::constant(native.clone());

        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let leaf = vec![Uniform::rand(&mut rng)];
            let expected = console::sparse_kary_merkle_tree::LeafHash::hash_leaf(&native, &leaf)?;

            Circuit::scope("LeafHash Poseidon", || {
                let circuit_leaf: Vec<_> = Inject::new(Mode::Private, leaf);
                let candidate = circuit.hash_leaf(&circuit_leaf);
                assert_eq!(expected, candidate.eject_value());
            });
            Circuit::reset();
        }
        Ok(())
    }
}
