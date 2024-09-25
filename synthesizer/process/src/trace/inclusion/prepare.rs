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

macro_rules! prepare_impl {
    ($self:ident, $transitions:ident, $query:ident, $current_state_root:ident, $get_state_path_for_commitment:ident $(, $await:ident)?) => {{
        // Ensure the number of leaves is within the Merkle tree size.
        Transaction::<N>::check_execution_size($transitions.len())?;

        // Initialize an empty transaction tree.
        let mut transaction_tree = N::merkle_tree_bhp::<TRANSACTION_DEPTH>(&[])?;
        // Initialize a vector for the assignments.
        let mut assignments = vec![];

        // Retrieve the global state root.
        let global_state_root = {
            $query.$current_state_root()
            $(.$await)?
        }?;

        // Ensure the global state root is not zero.
        if *global_state_root == Field::zero() {
            bail!("Inclusion expected the global state root in the execution to *not* be zero")
        }

        for (transition_index, transition) in $transitions.iter().enumerate() {
            // Construct the transaction leaf.
            let transaction_leaf = TransactionLeaf::new_execution(transition_index as u16, **transition.id());

            // Process the input tasks.
            match $self.input_tasks.get(transition.id()) {
                Some(tasks) => {
                    for task in tasks {
                        // Retrieve the local state root.
                        let local_state_root = (*transaction_tree.root()).into();

                        // Construct the state path.
                        let state_path = match &task.local {
                            Some((transaction_leaf, transition_root, tcm, transition_path, transition_leaf)) => {
                                // Compute the transaction path.
                                let transaction_path =
                                    transaction_tree.prove(transaction_leaf.index() as usize, &transaction_leaf.to_bits_le())?;
                                // Output the state path.
                                StatePath::new_local(
                                    global_state_root,
                                    local_state_root,
                                    transaction_path,
                                    *transaction_leaf,
                                    *transition_root,
                                    *tcm,
                                    transition_path.clone(),
                                    *transition_leaf,
                                )?
                            }
                            None => {
                                $query.$get_state_path_for_commitment(&task.commitment)
                                $(.$await)?
                            }?
                        };

                        // Ensure the global state root is the same across iterations.
                        if global_state_root != state_path.global_state_root() {
                            bail!("Inclusion expected the global state root to be the same across iterations")
                        }

                        // Construct the assignment for the state path.
                        let assignment = InclusionAssignment::new(
                            state_path,
                            task.commitment,
                            task.gamma,
                            task.serial_number,
                            local_state_root,
                            task.local.is_none(), // Equivalent to 'is_global'
                        );

                        // Add the assignment to the assignments.
                        assignments.push(assignment);
                    }
                }
                None => bail!("Missing input tasks for transition {} in inclusion", transition.id()),
            }

            // Insert the leaf into the transaction tree.
            transaction_tree.append(&[transaction_leaf.to_bits_le()])?;
        }

        Ok((assignments, global_state_root))
    }};
}

impl<N: Network> Inclusion<N> {
    /// Returns the inclusion assignments for the given transitions.
    pub fn prepare(
        &self,
        transitions: &[Transition<N>],
        query: impl QueryTrait<N>,
    ) -> Result<(Vec<InclusionAssignment<N>>, N::StateRoot)> {
        prepare_impl!(self, transitions, query, current_state_root, get_state_path_for_commitment)
    }

    /// Returns the inclusion assignments for the given transitions.
    #[cfg(feature = "async")]
    pub async fn prepare_async(
        &self,
        transitions: &[Transition<N>],
        query: impl QueryTrait<N>,
    ) -> Result<(Vec<InclusionAssignment<N>>, N::StateRoot)> {
        prepare_impl!(self, transitions, query, current_state_root_async, get_state_path_for_commitment_async, await)
    }
}
