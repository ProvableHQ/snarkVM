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

/// A trait for a Merkle path hash function.
pub trait PathHash<E: Environment> {
    type Hash: Clone
        + Default
        + Inject<Primitive = <Self::Primitive as console::kary_merkle_tree::PathHash>::Hash>
        + Eject<Primitive = <Self::Primitive as console::kary_merkle_tree::PathHash>::Hash>
        + Equal<Output = Boolean<E>>
        + Ternary<Boolean = Boolean<E>, Output = Self::Hash>;
    type Primitive: console::kary_merkle_tree::PathHash;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, children: &[Self::Hash]) -> Self::Hash;

    /// Returns the empty hash.
    fn hash_empty<const ARITY: u8>(&self) -> Self::Hash {
        let children = vec![Self::Hash::default(); ARITY as usize];
        self.hash_children(&children)
    }
}

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> PathHash<E> for BHP<E, NUM_WINDOWS, WINDOW_SIZE> {
    type Hash = Field<E>;
    type Primitive = console::algorithms::BHP<E::Network, NUM_WINDOWS, WINDOW_SIZE>;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, children: &[Self::Hash]) -> Self::Hash {
        let mut input = Vec::new();
        // Prepend the nodes with a `true` bit.
        input.push(Boolean::constant(true));
        for child in children {
            child.write_bits_le(&mut input);
        }
        // Hash the input.
        Hash::hash(self, &input)
    }
}

impl<E: Environment, const RATE: usize> PathHash<E> for Poseidon<E, RATE> {
    type Hash = Field<E>;
    type Primitive = console::algorithms::Poseidon<E::Network, RATE>;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, children: &[Self::Hash]) -> Self::Hash {
        let mut input = Vec::with_capacity(1 + children.len());
        // Prepend the nodes with a `1field` byte.
        input.push(Self::Hash::one());
        for child in children {
            input.push(child.clone());
        }
        // Hash the input.
        Hash::hash(self, &input)
    }
}

impl<E: Environment, const TYPE: u8, const VARIANT: usize> PathHash<E> for Keccak<E, TYPE, VARIANT> {
    type Hash = BooleanHash<E, VARIANT>;
    type Primitive = console::algorithms::Keccak<TYPE, VARIANT>;

    /// Returns the hash of the given child nodes.
    fn hash_children(&self, children: &[Self::Hash]) -> Self::Hash {
        let mut input = Vec::new();
        // Prepend the nodes with a `true` bit.
        input.push(Boolean::constant(true));
        for child in children {
            child.write_bits_le(&mut input);
        }
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
    use snarkvm_circuit_algorithms::{Keccak256, Poseidon2, Sha3_256, BHP512};
    use snarkvm_circuit_types::environment::Circuit;
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    const ITERATIONS: u64 = 5;
    const DOMAIN: &str = "MerkleTreeCircuit0";

    macro_rules! check_hash_children {
        ($native:ident, $circuit:ident, $mode:ident, $arity:expr, ($num_constants:expr, $num_public:expr, $num_private:expr, $num_constraints:expr)) => {{
            let mut rng = TestRng::default();

            for i in 0..ITERATIONS {
                // Sample a random input.
                let children = (0..$arity).map(|_| Uniform::rand(&mut rng)).collect::<Vec<_>>();

                // Compute the expected hash.
                let expected = console::kary_merkle_tree::PathHash::hash_children(&$native, &children)?;

                // Prepare the circuit input.
                let children = children.into_iter().map(|child| Inject::new(Mode::$mode, child)).collect::<Vec<_>>();

                Circuit::scope(format!("PathHash {i}"), || {
                    // Perform the hash operation.
                    let candidate = $circuit.hash_children(&children);
                    assert_scope!($num_constants, $num_public, $num_private, $num_constraints);
                    assert_eq!(expected, candidate.eject_value());
                });
                Circuit::reset();
            }
            Ok::<_, anyhow::Error>(())
        }};
        ($hash:ident, $mode:ident, $arity:expr, ($num_constants:expr, $num_public:expr, $num_private:expr, $num_constraints:expr)) => {{
            // Initialize the hash.
            let native = snarkvm_console_algorithms::$hash::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
            let circuit = $hash::<Circuit>::constant(native.clone());

            check_hash_children!(
                native,
                circuit,
                $mode,
                $arity,
                ($num_constants, $num_public, $num_private, $num_constraints)
            )
        }};
    }

    #[test]
    fn test_hash_children_bhp512_constant() -> Result<()> {
        check_hash_children!(BHP512, Constant, 2, (1599, 0, 0, 0))?;
        check_hash_children!(BHP512, Constant, 3, (2792, 0, 0, 0))
    }

    #[test]
    fn test_hash_children_bhp512_public() -> Result<()> {
        check_hash_children!(BHP512, Public, 2, (409, 0, 1879, 1883))?;
        check_hash_children!(BHP512, Public, 3, (418, 0, 3747, 3755))
    }

    #[test]
    fn test_hash_children_bhp512_private() -> Result<()> {
        check_hash_children!(BHP512, Private, 2, (409, 0, 1879, 1883))?;
        check_hash_children!(BHP512, Private, 3, (418, 0, 3747, 3755))
    }

    #[test]
    fn test_hash_children_poseidon2_constant() -> Result<()> {
        check_hash_children!(Poseidon2, Constant, 2, (1, 0, 0, 0))?;
        check_hash_children!(Poseidon2, Constant, 3, (1, 0, 0, 0))?;

        check_hash_children!(Poseidon2, Constant, 4, (1, 0, 0, 0))?;
        check_hash_children!(Poseidon2, Constant, 5, (1, 0, 0, 0))?;
        check_hash_children!(Poseidon2, Constant, 6, (1, 0, 0, 0))
    }

    #[test]
    fn test_hash_children_poseidon2_public() -> Result<()> {
        check_hash_children!(Poseidon2, Public, 2, (1, 0, 540, 540))?;
        check_hash_children!(Poseidon2, Public, 3, (1, 0, 540, 540))?;

        check_hash_children!(Poseidon2, Public, 4, (1, 0, 815, 815))?;
        check_hash_children!(Poseidon2, Public, 5, (1, 0, 815, 815))?;

        check_hash_children!(Poseidon2, Public, 6, (1, 0, 1090, 1090))?;
        check_hash_children!(Poseidon2, Public, 7, (1, 0, 1090, 1090))
    }

    #[test]
    fn test_hash_children_poseidon2_private() -> Result<()> {
        check_hash_children!(Poseidon2, Private, 2, (1, 0, 540, 540))?;
        check_hash_children!(Poseidon2, Private, 3, (1, 0, 540, 540))?;

        check_hash_children!(Poseidon2, Private, 4, (1, 0, 815, 815))?;
        check_hash_children!(Poseidon2, Private, 5, (1, 0, 815, 815))?;

        check_hash_children!(Poseidon2, Private, 6, (1, 0, 1090, 1090))?;
        check_hash_children!(Poseidon2, Private, 7, (1, 0, 1090, 1090))
    }

    #[test]
    fn test_hash_children_keccak256_constant() -> Result<()> {
        let native = snarkvm_console_algorithms::Keccak256 {};
        let circuit = Keccak256::<Circuit>::new();

        check_hash_children!(native, circuit, Constant, 2, (256, 0, 0, 0))?;
        check_hash_children!(native, circuit, Constant, 3, (256, 0, 0, 0))?;
        check_hash_children!(native, circuit, Constant, 4, (256, 0, 0, 0))?;
        check_hash_children!(native, circuit, Constant, 5, (256, 0, 0, 0))?;
        check_hash_children!(native, circuit, Constant, 8, (256, 0, 0, 0))?;
        check_hash_children!(native, circuit, Constant, 16, (256, 0, 0, 0))
    }

    #[test]
    fn test_hash_children_keccak256_public() -> Result<()> {
        let native = snarkvm_console_algorithms::Keccak256 {};
        let circuit = Keccak256::<Circuit>::new();

        check_hash_children!(native, circuit, Public, 2, (256, 0, 151424, 151424))?;
        check_hash_children!(native, circuit, Public, 3, (256, 0, 151936, 151936))?;
        check_hash_children!(native, circuit, Public, 4, (256, 0, 152448, 152448))?;
        check_hash_children!(native, circuit, Public, 5, (256, 0, 306367, 306367))?;
        check_hash_children!(native, circuit, Public, 8, (256, 0, 307135, 307135))?;
        check_hash_children!(native, circuit, Public, 12, (256, 0, 461759, 461759))?;
        check_hash_children!(native, circuit, Public, 16, (256, 0, 616383, 616383))
    }

    #[test]
    fn test_hash_children_keccak256_private() -> Result<()> {
        let native = snarkvm_console_algorithms::Keccak256 {};
        let circuit = Keccak256::<Circuit>::new();

        check_hash_children!(native, circuit, Private, 2, (256, 0, 151424, 151424))?;
        check_hash_children!(native, circuit, Private, 3, (256, 0, 151936, 151936))?;
        check_hash_children!(native, circuit, Private, 4, (256, 0, 152448, 152448))?;
        check_hash_children!(native, circuit, Private, 5, (256, 0, 306367, 306367))?;
        check_hash_children!(native, circuit, Private, 8, (256, 0, 307135, 307135))?;
        check_hash_children!(native, circuit, Private, 12, (256, 0, 461759, 461759))?;
        check_hash_children!(native, circuit, Private, 16, (256, 0, 616383, 616383))
    }

    #[test]
    fn test_hash_children_sha3_256_constant() -> Result<()> {
        let native = snarkvm_console_algorithms::Sha3_256 {};
        let circuit = Sha3_256::<Circuit>::new();

        check_hash_children!(native, circuit, Constant, 2, (256, 0, 0, 0))?;
        check_hash_children!(native, circuit, Constant, 3, (256, 0, 0, 0))?;
        check_hash_children!(native, circuit, Constant, 4, (256, 0, 0, 0))?;
        check_hash_children!(native, circuit, Constant, 5, (256, 0, 0, 0))?;
        check_hash_children!(native, circuit, Constant, 8, (256, 0, 0, 0))?;
        check_hash_children!(native, circuit, Constant, 16, (256, 0, 0, 0))
    }

    #[test]
    fn test_hash_children_sha3_256_public() -> Result<()> {
        let native = snarkvm_console_algorithms::Sha3_256 {};
        let circuit = Sha3_256::<Circuit>::new();

        check_hash_children!(native, circuit, Public, 2, (256, 0, 151424, 151424))?;
        check_hash_children!(native, circuit, Public, 3, (256, 0, 151936, 151936))?;
        check_hash_children!(native, circuit, Public, 4, (256, 0, 152448, 152448))?;
        check_hash_children!(native, circuit, Public, 5, (256, 0, 306367, 306367))?;
        check_hash_children!(native, circuit, Public, 8, (256, 0, 307135, 307135))?;
        check_hash_children!(native, circuit, Public, 12, (256, 0, 461759, 461759))?;
        check_hash_children!(native, circuit, Public, 16, (256, 0, 616383, 616383))
    }

    #[test]
    fn test_hash_children_sha3_256_private() -> Result<()> {
        let native = snarkvm_console_algorithms::Sha3_256 {};
        let circuit = Sha3_256::<Circuit>::new();

        check_hash_children!(native, circuit, Private, 2, (256, 0, 151424, 151424))?;
        check_hash_children!(native, circuit, Private, 3, (256, 0, 151936, 151936))?;
        check_hash_children!(native, circuit, Private, 4, (256, 0, 152448, 152448))?;
        check_hash_children!(native, circuit, Private, 5, (256, 0, 306367, 306367))?;
        check_hash_children!(native, circuit, Private, 8, (256, 0, 307135, 307135))?;
        check_hash_children!(native, circuit, Private, 12, (256, 0, 461759, 461759))?;
        check_hash_children!(native, circuit, Private, 16, (256, 0, 616383, 616383))
    }
}
