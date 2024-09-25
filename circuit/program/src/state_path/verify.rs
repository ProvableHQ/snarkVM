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

impl<A: Aleo> StatePath<A> {
    /// Returns `true` if the state path is valid.
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
    pub fn verify(&self, is_global: &Boolean<A>, local_state_root: &Field<A>) -> Boolean<A> {
        // Ensure the transition path is valid.
        let check_transition_path =
            A::verify_merkle_path_bhp(&self.transition_path, &self.transition_root, &self.transition_leaf.to_bits_le())
                & self.transition_leaf.variant().is_equal(&U8::constant(console::U8::new(3))); // Variant = 3 (Input::Record)

        // Ensure the transaction leaf is valid.
        let check_transaction_leaf =
            A::hash_bhp512(&(&self.transition_root, &self.tcm).to_bits_le()).is_equal(self.transaction_leaf.id());

        // Ensure the transaction path is valid.
        let check_transaction_path = A::verify_merkle_path_bhp(
            &self.transaction_path,
            &self.transaction_id,
            &self.transaction_leaf.to_bits_le(),
        ) & self.transaction_leaf.variant().is_equal(&U8::one()); // Variant = 1 (Transaction::Execution)

        // Ensure the transactions path is valid.
        let check_transactions_path = A::verify_merkle_path_bhp(
            &self.transactions_path,
            self.header_leaf.id(),
            &self.transaction_id.to_bits_le(),
        );

        // Ensure the header path is valid.
        let check_header_path =
            A::verify_merkle_path_bhp(&self.header_path, &self.header_root, &self.header_leaf.to_bits_le())
                & self.header_leaf.index().is_equal(&U8::one()); // Index = 1 (Header::transactions_root)

        // Construct the block hash preimage.
        let mut block_hash_preimage = self.previous_block_hash.to_bits_le();
        self.header_root.write_bits_le(&mut block_hash_preimage);

        // Ensure the block path is valid.
        let check_block_hash = A::hash_bhp1024(&block_hash_preimage).is_equal(&self.block_hash);

        // Ensure the global state root is correct.
        let check_state_root =
            A::verify_merkle_path_bhp(&self.block_path, &self.global_state_root, &self.block_hash.to_bits_le());

        // Combine the transition and transaction path checks.
        let check_transition_and_transaction_path =
            check_transition_path & check_transaction_path & check_transaction_leaf;

        // Check the state path.
        let check_local = &check_transition_and_transaction_path & local_state_root.is_equal(&self.transaction_id);
        let check_global = check_transition_and_transaction_path
            & check_transactions_path
            & check_header_path
            & check_block_hash
            & check_state_root;

        // If the state path is for a global root, return 'check_global'. Else, return 'check_local'.
        Boolean::ternary(is_global, &check_global, &check_local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Circuit;
    use snarkvm_utilities::rand::{TestRng, Uniform};

    type CurrentNetwork = <Circuit as Environment>::Network;

    const ITERATIONS: usize = 20;

    fn check_verify_global(
        mode: Mode,
        is_global: bool,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample the console state path.
            let console_state_path =
                console::state_path::test_helpers::sample_global_state_path::<CurrentNetwork>(None, rng).unwrap();
            // Sample the local state root.
            let local_state_root = console::Field::rand(rng);

            // Ensure the console state path is valid.
            console_state_path.verify(true, local_state_root).unwrap();

            Circuit::scope(format!("Verify global state path {mode} (is_global: {is_global})"), || {
                // Inject the is_global boolean.
                let circuit_is_global = Boolean::new(mode, is_global);
                // Inject the local state root.
                let circuit_local_state_root = Field::new(mode, local_state_root);
                // Inject the state path.
                let circuit_state_path = StatePath::<Circuit>::new(mode, console_state_path.clone());

                // Ensure the state path is valid.
                let is_valid = circuit_state_path.verify(&circuit_is_global, &circuit_local_state_root);
                match is_global {
                    true => assert!(is_valid.eject_value()),
                    false => assert!(!is_valid.eject_value()),
                }

                assert!(Circuit::is_satisfied());
                // TODO (howardwu): Resolve skipping the cost count checks for the burn-in round.
                if i > 0 {
                    assert_scope!(num_constants, num_public, num_private, num_constraints);
                }
            });

            Circuit::reset();
        }
        Ok(())
    }

    fn check_verify_local(
        mode: Mode,
        is_global: bool,
        is_valid_local_root: bool,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample the console state path.
            let console_state_path =
                console::state_path::test_helpers::sample_local_state_path::<CurrentNetwork>(None, rng).unwrap();
            // Sample the local state root.
            let local_state_root = **console_state_path.transaction_id();

            // Ensure the console state path is valid.
            console_state_path.verify(false, local_state_root).unwrap();

            Circuit::scope(
                format!(
                    "Verify local state path {mode} (is_global: {is_global}, is_valid_local_root: {is_valid_local_root})"
                ),
                || {
                    // Inject the is_global boolean.
                    let circuit_is_global = Boolean::new(mode, is_global);
                    // Inject the local state root.
                    let circuit_local_state_root = if is_valid_local_root {
                        Field::new(mode, local_state_root)
                    } else {
                        Field::new(mode, console::Field::rand(rng))
                    };

                    // Inject the state path.
                    let circuit_state_path = StatePath::<Circuit>::new(mode, console_state_path.clone());

                    // Ensure the state path is valid.
                    let is_valid = circuit_state_path.verify(&circuit_is_global, &circuit_local_state_root);
                    match (is_global, is_valid_local_root) {
                        (false, true) => assert!(is_valid.eject_value()),
                        _ => assert!(!is_valid.eject_value()),
                    }

                    assert!(Circuit::is_satisfied());
                    // TODO (howardwu): Resolve skipping the cost count checks for the burn-in round.
                    if i > 0 {
                        assert_scope!(num_constants, num_public, num_private, num_constraints);
                    }
                },
            );

            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_state_path_verify_global_constant() -> Result<()> {
        check_verify_global(Mode::Constant, true, 112709, 1, 2, 2)?;
        check_verify_global(Mode::Constant, false, 112709, 1, 2, 2)
    }

    #[test]
    fn test_state_path_verify_global_public() -> Result<()> {
        check_verify_global(Mode::Public, true, 29450, 453, 130867, 131522)?;
        check_verify_global(Mode::Public, false, 29450, 453, 130867, 131522)
    }

    #[test]
    fn test_state_path_verify_global_private() -> Result<()> {
        check_verify_global(Mode::Private, true, 29450, 1, 131319, 131522)?;
        check_verify_global(Mode::Private, false, 29450, 1, 131319, 131522)
    }

    #[test]
    fn test_state_path_verify_local_constant() -> Result<()> {
        check_verify_local(Mode::Constant, false, true, 112709, 1, 2, 2)?;
        check_verify_local(Mode::Constant, false, false, 112709, 1, 2, 2)?;
        check_verify_local(Mode::Constant, true, true, 112709, 1, 2, 2)?;
        check_verify_local(Mode::Constant, true, false, 112709, 1, 2, 2)
    }

    #[test]
    fn test_state_path_verify_local_public() -> Result<()> {
        check_verify_local(Mode::Public, false, true, 29450, 453, 130867, 131522)?;
        check_verify_local(Mode::Public, false, false, 29450, 453, 130867, 131522)?;
        check_verify_local(Mode::Public, true, true, 29450, 453, 130867, 131522)?;
        check_verify_local(Mode::Public, true, false, 29450, 453, 130867, 131522)
    }

    #[test]
    fn test_state_path_verify_local_private() -> Result<()> {
        check_verify_local(Mode::Private, false, true, 29450, 1, 131319, 131522)?;
        check_verify_local(Mode::Private, false, false, 29450, 1, 131319, 131522)?;
        check_verify_local(Mode::Private, true, true, 29450, 1, 131319, 131522)?;
        check_verify_local(Mode::Private, true, false, 29450, 1, 131319, 131522)
    }
}
