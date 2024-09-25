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

mod helpers;
use helpers::{LeafHash, PathHash};

mod verify;

#[cfg(all(test, console))]
use snarkvm_circuit_types::environment::assert_scope;

use snarkvm_circuit_types::{environment::prelude::*, Boolean, Field, U64};

pub struct MerklePath<E: Environment, const DEPTH: u8> {
    /// The leaf index for the path.
    leaf_index: U64<E>,
    /// The `siblings` contains a list of sibling hashes from the leaf to the root.
    siblings: Vec<Field<E>>,
}

#[cfg(console)]
impl<E: Environment, const DEPTH: u8> Inject for MerklePath<E, DEPTH> {
    type Primitive = console::merkle_tree::MerklePath<E::Network, DEPTH>;

    /// Initializes a Merkle path from the given mode and native Merkle path.
    fn new(mode: Mode, merkle_path: Self::Primitive) -> Self {
        // Initialize the leaf index.
        let leaf_index = U64::new(mode, merkle_path.leaf_index());
        // Initialize the Merkle path siblings.
        let siblings: Vec<_> = merkle_path.siblings().iter().map(|node| Field::new(mode, *node)).collect();
        // Ensure the Merkle path is the correct depth.
        match siblings.len() == DEPTH as usize {
            // Return the Merkle path.
            true => Self { leaf_index, siblings },
            false => E::halt("Merkle path is not the correct depth"),
        }
    }
}

#[cfg(console)]
impl<E: Environment, const DEPTH: u8> Eject for MerklePath<E, DEPTH> {
    type Primitive = console::merkle_tree::MerklePath<E::Network, DEPTH>;

    /// Ejects the mode of the Merkle path.
    fn eject_mode(&self) -> Mode {
        (&self.leaf_index, &self.siblings).eject_mode()
    }

    /// Ejects the Merkle path.
    fn eject_value(&self) -> Self::Primitive {
        match Self::Primitive::try_from((&self.leaf_index, &self.siblings).eject_value()) {
            Ok(merkle_path) => merkle_path,
            Err(error) => E::halt(format!("Failed to eject the Merkle path: {error}")),
        }
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_network::AleoV0 as Circuit;
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    const ITERATIONS: u128 = 100;

    fn check_new<const DEPTH: u8>(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        let mut rng = TestRng::default();

        let mut create_leaves = |num_leaves| {
            (0..num_leaves)
                .map(|_| console::Field::<<Circuit as Environment>::Network>::rand(&mut rng).to_bits_le())
                .collect::<Vec<_>>()
        };

        for i in 0..ITERATIONS {
            // Determine the number of leaves.
            let num_leaves = core::cmp::min(2u128.pow(DEPTH as u32), i);
            // Compute the leaves.
            let leaves = create_leaves(num_leaves);
            // Compute the Merkle tree.
            let merkle_tree = <<Circuit as Environment>::Network as snarkvm_console_network::Network>::merkle_tree_bhp::<
                DEPTH,
            >(&leaves)?;

            for (index, leaf) in leaves.iter().enumerate() {
                // Compute the Merkle path.
                let merkle_path = merkle_tree.prove(index, leaf)?;

                // // Initialize the Merkle leaf.
                // let leaf: Vec<Boolean<_>> = Inject::new(mode, leaf.clone());

                Circuit::scope(format!("New {mode}"), || {
                    let candidate = MerklePath::<Circuit, DEPTH>::new(mode, merkle_path.clone());
                    assert_eq!(merkle_path, candidate.eject_value());
                    assert_scope!(num_constants, num_public, num_private, num_constraints);
                });
                Circuit::reset();
            }
        }
        Ok(())
    }

    #[test]
    fn test_new_constant() -> Result<()> {
        check_new::<32>(Mode::Constant, 96, 0, 0, 0)
    }

    #[test]
    fn test_new_public() -> Result<()> {
        check_new::<32>(Mode::Public, 0, 96, 0, 64)
    }

    #[test]
    fn test_new_private() -> Result<()> {
        check_new::<32>(Mode::Private, 0, 0, 96, 64)
    }
}
