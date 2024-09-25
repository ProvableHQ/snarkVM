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
pub use helpers::*;

mod verify;

#[cfg(test)]
use snarkvm_circuit_types::environment::assert_scope;

use snarkvm_circuit_collections::merkle_tree::MerklePath;
use snarkvm_circuit_network::Aleo;
use snarkvm_circuit_types::{environment::prelude::*, Boolean, Field, U8};

/// The depth of the Merkle tree for the blocks.
const BLOCKS_DEPTH: u8 = console::BLOCKS_DEPTH;
/// The depth of the Merkle tree for the block header.
const HEADER_DEPTH: u8 = console::HEADER_DEPTH;
/// The depth of the Merkle tree for transactions in a block.
const TRANSACTIONS_DEPTH: u8 = console::TRANSACTIONS_DEPTH;
/// The depth of the Merkle tree for the transaction.
const TRANSACTION_DEPTH: u8 = console::TRANSACTION_DEPTH;
/// The depth of the Merkle tree for the transition.
const TRANSITION_DEPTH: u8 = console::TRANSITION_DEPTH;

type BlockPath<A> = MerklePath<A, BLOCKS_DEPTH>;
type HeaderPath<A> = MerklePath<A, HEADER_DEPTH>;
type TransactionsPath<A> = MerklePath<A, TRANSACTIONS_DEPTH>;
type TransactionPath<A> = MerklePath<A, TRANSACTION_DEPTH>;
type TransitionPath<A> = MerklePath<A, TRANSITION_DEPTH>;

/// The state path proves existence of the transition leaf to either a global or local state root.
pub struct StatePath<A: Aleo> {
    /// The global state root (Public).
    global_state_root: Field<A>,
    /// The Merkle path for the block hash.
    block_path: BlockPath<A>,
    /// The block hash.
    block_hash: Field<A>,
    /// The previous block hash.
    previous_block_hash: Field<A>,
    /// The block header root.
    header_root: Field<A>,
    /// The Merkle path for the block header leaf.
    header_path: HeaderPath<A>,
    /// The block header leaf.
    header_leaf: HeaderLeaf<A>,
    /// The Merkle path for the transaction ID.
    transactions_path: TransactionsPath<A>,
    /// The transaction ID.
    transaction_id: Field<A>,
    /// The Merkle path for the transaction leaf.
    transaction_path: TransactionPath<A>,
    /// The transaction leaf.
    transaction_leaf: TransactionLeaf<A>,
    /// The transition root.
    transition_root: Field<A>,
    /// The transition commitment.
    tcm: Field<A>,
    /// The Merkle path for the transition leaf.
    transition_path: TransitionPath<A>,
    /// The transition leaf.
    transition_leaf: TransitionLeaf<A>,
}

impl<A: Aleo> StatePath<A> {
    /// Returns the transition leaf.
    pub const fn transition_leaf(&self) -> &TransitionLeaf<A> {
        &self.transition_leaf
    }
}

impl<A: Aleo> Inject for StatePath<A> {
    type Primitive = console::StatePath<A::Network>;

    /// Initializes a new ciphertext circuit from a primitive.
    fn new(mode: Mode, state_path: Self::Primitive) -> Self {
        Self {
            global_state_root: Field::new(Mode::Public, *state_path.global_state_root()),
            block_path: BlockPath::new(mode, state_path.block_path().clone()),
            block_hash: Field::new(mode, *state_path.block_hash()),
            previous_block_hash: Field::new(mode, *state_path.previous_block_hash()),
            header_root: Field::new(mode, *state_path.header_root()),
            header_path: HeaderPath::new(mode, state_path.header_path().clone()),
            header_leaf: HeaderLeaf::new(mode, *state_path.header_leaf()),
            transactions_path: TransactionsPath::new(mode, state_path.transactions_path().clone()),
            transaction_id: Field::new(mode, **state_path.transaction_id()),
            transaction_path: TransactionPath::new(mode, state_path.transaction_path().clone()),
            transaction_leaf: TransactionLeaf::new(mode, *state_path.transaction_leaf()),
            transition_root: Field::new(mode, *state_path.transition_root()),
            tcm: Field::new(mode, *state_path.tcm()),
            transition_path: TransitionPath::new(mode, state_path.transition_path().clone()),
            transition_leaf: TransitionLeaf::new(mode, *state_path.transition_leaf()),
        }
    }
}

impl<A: Aleo> Eject for StatePath<A> {
    type Primitive = console::StatePath<A::Network>;

    /// Ejects the mode of the state path.
    fn eject_mode(&self) -> Mode {
        Mode::combine(self.global_state_root.eject_mode(), [
            self.block_path.eject_mode(),
            self.block_hash.eject_mode(),
            self.previous_block_hash.eject_mode(),
            self.header_root.eject_mode(),
            self.header_path.eject_mode(),
            self.header_leaf.eject_mode(),
            self.transactions_path.eject_mode(),
            self.transaction_id.eject_mode(),
            self.transaction_path.eject_mode(),
            self.transaction_leaf.eject_mode(),
            self.transition_root.eject_mode(),
            self.tcm.eject_mode(),
            self.transition_path.eject_mode(),
            self.transition_leaf.eject_mode(),
        ])
    }

    /// Ejects the state path.
    fn eject_value(&self) -> Self::Primitive {
        Self::Primitive::from(
            self.global_state_root.eject_value().into(),
            self.block_path.eject_value(),
            self.block_hash.eject_value().into(),
            self.previous_block_hash.eject_value().into(),
            self.header_root.eject_value(),
            self.header_path.eject_value(),
            self.header_leaf.eject_value(),
            self.transactions_path.eject_value(),
            self.transaction_id.eject_value().into(),
            self.transaction_path.eject_value(),
            self.transaction_leaf.eject_value(),
            self.transition_root.eject_value(),
            self.tcm.eject_value(),
            self.transition_path.eject_value(),
            self.transition_leaf.eject_value(),
        )
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use crate::Circuit;

    use snarkvm_utilities::TestRng;

    use anyhow::Result;

    type CurrentNetwork = <Circuit as Environment>::Network;

    const ITERATIONS: u64 = 250;

    fn check_new(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        let rng = &mut TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample the console state path.
            let console_state_path =
                console::state_path::test_helpers::sample_local_state_path::<CurrentNetwork>(None, rng).unwrap();

            Circuit::scope(format!("New {mode}"), || {
                let candidate = StatePath::<Circuit>::new(mode, console_state_path.clone());
                assert_eq!(console_state_path, candidate.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_state_path_new_constant() -> Result<()> {
        check_new(Mode::Constant, 450, 1, 0, 0)
    }

    #[test]
    fn test_state_path_new_public() -> Result<()> {
        check_new(Mode::Public, 0, 451, 0, 376)
    }

    #[test]
    fn test_state_path_new_private() -> Result<()> {
        check_new(Mode::Private, 0, 1, 450, 376)
    }
}
