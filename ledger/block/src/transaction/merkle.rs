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

impl<N: Network> Transaction<N> {
    /// The maximum number of transitions allowed in a transaction.
    pub const MAX_TRANSITIONS: usize = usize::pow(2, TRANSACTION_DEPTH as u32);

    /// Returns the transaction root, by computing the root for a Merkle tree of the transition IDs.
    pub fn to_root(&self) -> Result<Field<N>> {
        Ok(*self.to_tree()?.root())
    }

    /// Returns the Merkle leaf for the given ID of a function or transition in the transaction.
    pub fn to_leaf(&self, id: &Field<N>) -> Result<TransactionLeaf<N>> {
        match self {
            Self::Deploy(_, _, deployment, fee) => {
                // Check if the ID is the transition ID for the fee.
                if *id == **fee.id() {
                    // Return the transaction leaf.
                    return Ok(TransactionLeaf::new_fee(
                        u16::try_from(deployment.program().functions().len())?, // The last index.
                        *id,
                    ));
                }

                // Iterate through the functions in the deployment.
                for (index, function) in deployment.program().functions().values().enumerate() {
                    // Check if the function hash matches the given ID.
                    if *id == N::hash_bhp1024(&function.to_bytes_le()?.to_bits_le())? {
                        // Return the transaction leaf.
                        return Ok(TransactionLeaf::new_deployment(u16::try_from(index)?, *id));
                    }
                }
                // Error if the function hash was not found.
                bail!("Function hash not found in deployment transaction");
            }
            Self::Execute(_, execution, fee) => {
                // Check if the ID is the transition ID for the fee.
                if let Some(fee) = fee {
                    if *id == **fee.id() {
                        // Return the transaction leaf.
                        return Ok(TransactionLeaf::new_execution(
                            u16::try_from(execution.len())?, // The last index.
                            *id,
                        ));
                    }
                }

                // Iterate through the transitions in the execution.
                for (index, transition) in execution.transitions().enumerate() {
                    // Check if the transition ID matches the given ID.
                    if *id == **transition.id() {
                        // Return the transaction leaf.
                        return Ok(TransactionLeaf::new_execution(u16::try_from(index)?, *id));
                    }
                }
                // Error if the transition ID was not found.
                bail!("Transition ID not found in execution transaction");
            }
            Self::Fee(_, fee) => {
                if *id == **fee.id() {
                    // Return the transaction leaf.
                    return Ok(TransactionLeaf::new_fee(0, **fee.id()));
                }
                // Error if the transition ID was not found.
                bail!("Transition ID not found in fee transaction");
            }
        }
    }

    /// Returns the Merkle path for the transaction leaf.
    pub fn to_path(&self, leaf: &TransactionLeaf<N>) -> Result<TransactionPath<N>> {
        // Compute the Merkle path.
        self.to_tree()?.prove(leaf.index() as usize, &leaf.to_bits_le())
    }

    /// The Merkle tree of transition IDs for the transaction.
    pub fn to_tree(&self) -> Result<TransactionTree<N>> {
        match self {
            // Compute the deployment tree.
            Transaction::Deploy(_, _, deployment, fee) => Self::deployment_tree(deployment, Some(fee)),
            // Compute the execution tree.
            Transaction::Execute(_, execution, fee) => Self::execution_tree(execution, fee),
            // Compute the fee tree.
            Transaction::Fee(_, fee) => Self::fee_tree(fee),
        }
    }
}

impl<N: Network> Transaction<N> {
    /// Returns the Merkle tree for the given deployment.
    pub fn deployment_tree(deployment: &Deployment<N>, fee: Option<&Fee<N>>) -> Result<TransactionTree<N>> {
        // Ensure the number of leaves is within the Merkle tree size.
        Self::check_deployment_size(deployment)?;
        // Retrieve the program.
        let program = deployment.program();
        // Prepare the leaves.
        let leaves = program.functions().values().enumerate().map(|(index, function)| {
            // Construct the transaction leaf.
            Ok(TransactionLeaf::new_deployment(
                u16::try_from(index)?,
                N::hash_bhp1024(&to_bits_le![program.id(), function.to_bytes_le()?])?,
            )
            .to_bits_le())
        });
        // If the fee is present, add it to the leaves.
        let leaves = match fee {
            Some(fee) => {
                // Construct the transaction leaf.
                let leaf = TransactionLeaf::new_fee(
                    u16::try_from(program.functions().len())?, // The last index.
                    **fee.transition_id(),
                )
                .to_bits_le();
                // Add the leaf to the leaves.
                leaves.chain([Ok(leaf)].into_iter()).collect::<Result<Vec<_>>>()?
            }
            None => leaves.collect::<Result<Vec<_>>>()?,
        };
        // Compute the deployment tree.
        N::merkle_tree_bhp::<TRANSACTION_DEPTH>(&leaves)
    }

    /// Returns the Merkle tree for the given execution.
    pub fn execution_tree(execution: &Execution<N>, fee: &Option<Fee<N>>) -> Result<TransactionTree<N>> {
        Self::transitions_tree(execution.transitions(), fee)
    }

    /// Returns the Merkle tree for the given transitions.
    pub fn transitions_tree<'a>(
        transitions: impl ExactSizeIterator<Item = &'a Transition<N>>,
        fee: &Option<Fee<N>>,
    ) -> Result<TransactionTree<N>> {
        // Retrieve the number of transitions.
        let num_transitions = transitions.len();
        // Ensure the number of leaves is within the Merkle tree size.
        Self::check_execution_size(num_transitions)?;
        // Prepare the leaves.
        let leaves = transitions.enumerate().map(|(index, transition)| {
            // Construct the transaction leaf.
            Ok::<_, Error>(TransactionLeaf::new_execution(u16::try_from(index)?, **transition.id()).to_bits_le())
        });
        // If the fee is present, add it to the leaves.
        let leaves = match fee {
            Some(fee) => {
                // Construct the transaction leaf.
                let leaf = TransactionLeaf::new_fee(
                    u16::try_from(num_transitions)?, // The last index.
                    **fee.transition_id(),
                )
                .to_bits_le();
                // Add the leaf to the leaves.
                leaves.chain([Ok(leaf)].into_iter()).collect::<Result<Vec<_>, _>>()?
            }
            None => leaves.collect::<Result<Vec<_>, _>>()?,
        };

        // Compute the execution tree.
        N::merkle_tree_bhp::<TRANSACTION_DEPTH>(&leaves)
    }

    /// Returns the Merkle tree for the given fee.
    pub fn fee_tree(fee: &Fee<N>) -> Result<TransactionTree<N>> {
        // Construct the transaction leaf.
        let leaf = TransactionLeaf::new_fee(0u16, **fee.transition_id()).to_bits_le();
        // Compute the execution tree.
        N::merkle_tree_bhp::<TRANSACTION_DEPTH>(&[leaf])
    }

    /// Returns `true` if the deployment is within the size bounds.
    pub fn check_deployment_size(deployment: &Deployment<N>) -> Result<()> {
        // Retrieve the program.
        let program = deployment.program();
        // Retrieve the functions.
        let functions = program.functions();
        // Retrieve the verifying keys.
        let verifying_keys = deployment.verifying_keys();

        // Ensure the number of functions and verifying keys match.
        ensure!(
            functions.len() == verifying_keys.len(),
            "Number of functions ('{}') and verifying keys ('{}') do not match",
            functions.len(),
            verifying_keys.len()
        );
        // Ensure the number of functions is within the allowed range.
        ensure!(
            functions.len() < Self::MAX_TRANSITIONS, // Note: Observe we hold back 1 for the fee.
            "Deployment must contain less than {} functions, found {}",
            Self::MAX_TRANSITIONS,
            functions.len()
        );
        Ok(())
    }

    /// Returns `true` if the execution is within the size bounds.
    pub fn check_execution_size(num_transitions: usize) -> Result<()> {
        // Ensure there are transitions.
        ensure!(num_transitions > 0, "Execution must contain at least one transition");
        // Ensure the number of functions is within the allowed range.
        ensure!(
            num_transitions < Self::MAX_TRANSITIONS, // Note: Observe we hold back 1 for the fee.
            "Execution must contain less than {num_transitions} transitions, found {}",
            Self::MAX_TRANSITIONS,
        );
        Ok(())
    }
}
