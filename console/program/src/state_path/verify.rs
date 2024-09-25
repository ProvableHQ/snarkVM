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

impl<N: Network> StatePath<N> {
    /// Checks if the state path is valid.
    ///
    /// # Parameters
    ///  - `local_state_root` is the local transaction root for the current execution.
    ///  - `is_global` is a boolean indicating whether this is a global or local state root.
    ///
    /// # Diagram
    /// The `[[ ]]` notation is used to denote public inputs.
    /// ```ignore
    ///
    ///  [[ global_state_root ]]
    ///           |
    ///      block_path
    ///          |
    ///     block_hash := Hash( previous_block_hash || header_root )
    ///                                                     |
    ///                                                header_path
    ///                                                    |
    ///                                               header_leaf
    ///                                                   |
    ///                                            transactions_path          [[ local_state_root ]]
    ///                                                  |                               |
    ///                                               (true) ------ is_global ------ (false)
    ///                                                                 |
    ///                                                          transaction_id
    ///                                                                |
    ///                                                        transaction_path
    ///                                                               |
    ///                                                       transaction_leaf
    ///                                                              |
    ///                                                      transition_id := Hash( transition_root || tcm )
    ///                                                                                  |
    ///                                                                           transition_path
    ///                                                                                 |
    ///                                                                          transition_leaf
    /// ```
    pub fn verify(&self, is_global: bool, local_state_root: Field<N>) -> Result<()> {
        // Ensure the transition leaf variant is 3 (Input::Record).
        ensure!(self.transition_leaf.variant() == 3, "Transition leaf variant must be 3 (Input::Record)");
        // Ensure the transition path is valid.
        ensure!(
            N::verify_merkle_path_bhp(&self.transition_path, &self.transition_root, &self.transition_leaf.to_bits_le()),
            "'{}' (an input or output ID) does not belong to '{}' (a function or transition)",
            self.transition_leaf.id(),
            self.transaction_leaf.id()
        );

        // Ensure the transaction leaf is correct.
        ensure!(
            *self.transaction_leaf.id() == *N::hash_bhp512(&(*self.transition_root, self.tcm).to_bits_le())?,
            "Transaction leaf id '{}' is incorrect. Double-check the tcm and transition root.",
            self.transaction_leaf.id()
        );

        // Ensure the transaction leaf variant is 1 (Transaction::Execution).
        ensure!(self.transaction_leaf.variant() == 1, "Transaction leaf variant must be 1 (Transaction::Execution)");
        // Ensure the transaction path is valid.
        ensure!(
            N::verify_merkle_path_bhp(
                &self.transaction_path,
                &self.transaction_id,
                &self.transaction_leaf.to_bits_le()
            ),
            "'{}' (a function or transition) does not belong to transaction '{}'",
            self.transaction_leaf.id(),
            self.transaction_id
        );

        if is_global {
            // Ensure the header leaf index is 1 (Header::transactions_root).
            ensure!(self.header_leaf.index() == 1, "Header leaf index must be 1 (Header::transactions_root)");
            // Ensure the transactions path is valid.
            ensure!(
                N::verify_merkle_path_bhp(
                    &self.transactions_path,
                    &self.header_leaf.id(),
                    &self.transaction_id.to_bits_le()
                ),
                "Transaction '{}' does not belong to '{}' (a header leaf)",
                self.transaction_id,
                self.header_leaf
            );
            // Ensure the header path is valid.
            ensure!(
                N::verify_merkle_path_bhp(&self.header_path, &self.header_root, &self.header_leaf.to_bits_le()),
                "'{}' (a header leaf) does not belong to '{}' (a block header)",
                self.header_leaf,
                self.block_hash
            );
            // Ensure the block hash is correct.
            ensure!(
                *self.block_hash == N::hash_bhp1024(&to_bits_le![(*self.previous_block_hash), self.header_root])?,
                "Block hash '{}' is incorrect. Double-check the previous block hash and block header root.",
                self.block_hash
            );
            // Ensure the global state root is correct.
            ensure!(
                N::verify_merkle_path_bhp(&self.block_path, &self.global_state_root, &self.block_hash.to_bits_le()),
                "'{}' (a block hash) does not belong to '{}' (a global state root)",
                self.block_hash,
                self.global_state_root
            );
        } else {
            // Ensure the local state root is correct.
            ensure!(
                *self.transaction_id == local_state_root,
                "'{}' (a decoded transaction ID) does not match the '{local_state_root}' (a local state root)",
                *self.transaction_id
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::{prelude::TestRng, MainnetV0};

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: usize = 100;

    #[test]
    fn test_verify_global() {
        let rng = &mut TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample the state path.
            let state_path =
                crate::state_path::test_helpers::sample_global_state_path::<CurrentNetwork>(None, rng).unwrap();
            // Sample the local state root.
            let local_state_root = Field::rand(rng);

            // Ensure the state path is valid.
            state_path.verify(true, local_state_root).unwrap();
            // Ensure the state path is *not* valid for a random local state root.
            state_path.verify(false, local_state_root).unwrap_err();
        }
    }

    #[test]
    fn test_verify_local() {
        let rng = &mut TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample the state path.
            let state_path =
                crate::state_path::test_helpers::sample_local_state_path::<CurrentNetwork>(None, rng).unwrap();
            // Retrieve the local state root.
            let local_state_root = **state_path.transaction_id();

            // Ensure the state path is valid.
            state_path.verify(false, local_state_root).unwrap();
            // Ensure the state path does *not* match a random local state root.
            state_path.verify(false, Field::rand(rng)).unwrap_err();
            // Ensure the state path does *not* match to the random global state root.
            state_path.verify(true, local_state_root).unwrap_err();
            // Ensure the state path does *not* match to the random global state root.
            state_path.verify(true, Field::rand(rng)).unwrap_err();
        }
    }

    #[test]
    fn test_verify_new_local() {
        let rng = &mut TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample the state path.
            let state_path =
                crate::state_path::test_helpers::sample_local_state_path::<CurrentNetwork>(None, rng).unwrap();

            // Initialize the state path using `new_local`.
            let new_local_state_path = StatePath::new_local(
                state_path.global_state_root(),
                *state_path.transaction_id(),
                state_path.transaction_path().clone(),
                *state_path.transaction_leaf(),
                *state_path.transition_root(),
                *state_path.tcm(),
                state_path.transition_path().clone(),
                *state_path.transition_leaf(),
            )
            .unwrap();

            // Retrieve the local state root.
            let local_state_root = **new_local_state_path.transaction_id();

            // Ensure the state path is valid.
            new_local_state_path.verify(false, local_state_root).unwrap();
            // Ensure the state path does *not* match a random local state root.
            new_local_state_path.verify(false, Field::rand(rng)).unwrap_err();
            // Ensure the state path does *not* match to the random global state root.
            new_local_state_path.verify(true, local_state_root).unwrap_err();
            // Ensure the state path does *not* match to the random global state root.
            new_local_state_path.verify(true, Field::rand(rng)).unwrap_err();
        }
    }
}
