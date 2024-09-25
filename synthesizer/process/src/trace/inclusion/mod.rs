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

mod prepare;

#[cfg(debug_assertions)]
use crate::Stack;

use console::{
    network::prelude::*,
    program::{InputID, StatePath, TransactionLeaf, TransitionLeaf, TransitionPath, TRANSACTION_DEPTH},
    types::{Field, Group},
};
use ledger_block::{Input, Output, Transaction, Transition};
use ledger_query::QueryTrait;

use std::collections::HashMap;

#[derive(Clone, Debug)]
struct InputTask<N: Network> {
    /// The commitment.
    commitment: Field<N>,
    /// The gamma value.
    gamma: Group<N>,
    /// The serial number.
    serial_number: Field<N>,
    /// Contains the local transaction leaf, local transition root, local transition tcm, local transition path,
    /// and local transition leaf, if this input is a record from a previous local transition.
    local: Option<(TransactionLeaf<N>, Field<N>, Field<N>, TransitionPath<N>, TransitionLeaf<N>)>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Inclusion<N: Network> {
    /// A map of `transition IDs` to a list of `input tasks`.
    input_tasks: HashMap<N::TransitionID, Vec<InputTask<N>>>,
    /// A map of `commitments` to `(local transaction leaf, local transition root, local transition tcm, local transition path, local transition leaf)` pairs.
    output_commitments:
        HashMap<Field<N>, (TransactionLeaf<N>, Field<N>, Field<N>, TransitionPath<N>, TransitionLeaf<N>)>,
}

impl<N: Network> Inclusion<N> {
    /// Initializes a new `Inclusion` instance.
    pub fn new() -> Self {
        Self { input_tasks: HashMap::new(), output_commitments: HashMap::new() }
    }

    /// Inserts the transition to build state for the inclusion task.
    pub fn insert_transition(&mut self, input_ids: &[InputID<N>], transition: &Transition<N>) -> Result<()> {
        // Ensure the transition inputs and input IDs are the same length.
        if input_ids.len() != transition.inputs().len() {
            bail!("Inclusion expected the same number of input IDs as transition inputs")
        }

        // Retrieve the transition index.
        let transition_index = u16::try_from(self.input_tasks.len())?;

        // Initialize the input tasks.
        let input_tasks = self.input_tasks.entry(*transition.id()).or_default();

        // Process the inputs.
        for input_id in input_ids {
            // Filter the inputs for records.
            if let InputID::Record(commitment, gamma, serial_number, ..) = input_id {
                // Add the record to the input tasks.
                input_tasks.push(InputTask {
                    commitment: *commitment,
                    gamma: *gamma,
                    serial_number: *serial_number,
                    local: self.output_commitments.get(commitment).cloned(),
                });
            }
        }

        if !transition.outputs().is_empty() {
            // Compute the transaction leaf.
            let transaction_leaf = TransactionLeaf::new_execution(transition_index, **transition.id());
            // Compute the transition root.
            let transition_root = transition.to_root()?;
            // Fetch the tcm.
            let tcm = *transition.tcm();

            // Process the outputs.
            for (index, output) in transition.outputs().iter().enumerate() {
                // Filter the outputs for records.
                if let Output::Record(commitment, ..) = output {
                    // Compute the output index.
                    let output_index = u8::try_from(input_ids.len().saturating_add(index))?;
                    // Compute the transition leaf.
                    let transition_leaf = output.to_transition_leaf(output_index);
                    // Compute the transition path.
                    let transition_path = transition.to_path(&transition_leaf)?;
                    // Add the record's local Merklization to the output commitments.
                    self.output_commitments.insert(
                        *commitment,
                        (transaction_leaf, transition_root, tcm, transition_path, transition_leaf),
                    );
                }
            }
        }

        Ok(())
    }
}

impl<N: Network> Inclusion<N> {
    /// Returns the verifier public inputs for the given global state root and transitions.
    pub fn prepare_verifier_inputs<'a>(
        global_state_root: N::StateRoot,
        transitions: impl ExactSizeIterator<Item = &'a Transition<N>>,
    ) -> Result<Vec<Vec<N::Field>>> {
        // Determine the number of transitions.
        let num_transitions = transitions.len();

        // Initialize an empty transaction tree.
        let mut transaction_tree = N::merkle_tree_bhp::<TRANSACTION_DEPTH>(&[])?;
        // Initialize a vector for the batch verifier inputs.
        let mut batch_verifier_inputs = vec![];

        // Construct the batch verifier inputs.
        for (transition_index, transition) in transitions.enumerate() {
            // Retrieve the local state root.
            let local_state_root = *transaction_tree.root();

            // Iterate through the inputs.
            for input in transition.inputs() {
                // Filter the inputs for records.
                if let Input::Record(serial_number, _) = input {
                    // Add the public inputs to the batch verifier inputs.
                    let verifier_inputs =
                        vec![N::Field::one(), **global_state_root, *local_state_root, **serial_number];
                    batch_verifier_inputs.push(verifier_inputs);
                }
            }

            // If this is not the last transition, append the transaction leaf to the transaction tree.
            if transition_index + 1 != num_transitions {
                // Construct the transaction leaf.
                let leaf = TransactionLeaf::new_execution(u16::try_from(transition_index)?, **transition.id());
                // Insert the leaf into the transaction tree.
                transaction_tree.append(&[leaf.to_bits_le()])?;
            }
        }

        // Ensure the global state root is not zero.
        if batch_verifier_inputs.is_empty() && *global_state_root == Field::zero() {
            bail!("Inclusion expected the global state root in the execution to *not* be zero")
        }

        Ok(batch_verifier_inputs)
    }
}

#[derive(Clone, Debug)]
pub struct InclusionAssignment<N: Network> {
    pub(crate) state_path: StatePath<N>,
    commitment: Field<N>,
    gamma: Group<N>,
    serial_number: Field<N>,
    local_state_root: N::TransactionID,
    is_global: bool,
}

impl<N: Network> InclusionAssignment<N> {
    /// Initializes a new inclusion assignment.
    pub fn new(
        state_path: StatePath<N>,
        commitment: Field<N>,
        gamma: Group<N>,
        serial_number: Field<N>,
        local_state_root: N::TransactionID,
        is_global: bool,
    ) -> Self {
        Self { state_path, commitment, gamma, serial_number, local_state_root, is_global }
    }

    /// The circuit for state path verification.
    ///
    /// # Diagram
    /// The `[[ ]]` notation is used to denote public inputs.
    /// ```ignore
    ///             [[ global_state_root ]] || [[ local_state_root ]]
    ///                        |                          |
    ///                        -------- is_global --------
    ///                                     |
    ///                                state_path
    ///                                    |
    /// [[ serial_number ]] := Commit( commitment || Hash( COFACTOR * gamma ) )
    /// ```
    pub fn to_circuit_assignment<A: circuit::Aleo<Network = N>>(&self) -> Result<circuit::Assignment<N::Field>> {
        use circuit::Inject;

        // Ensure the circuit environment is clean.
        assert_eq!(A::count(), (0, 1, 0, 0, (0, 0, 0)));
        A::reset();

        // Inject the state path as `Mode::Private` (with a global state root as `Mode::Public`).
        let state_path = circuit::StatePath::<A>::new(circuit::Mode::Private, self.state_path.clone());
        // Inject the commitment as `Mode::Private`.
        let commitment = circuit::Field::<A>::new(circuit::Mode::Private, self.commitment);
        // Inject the gamma as `Mode::Private`.
        let gamma = circuit::Group::<A>::new(circuit::Mode::Private, self.gamma);

        // Inject the local state root as `Mode::Public`.
        let local_state_root = circuit::Field::<A>::new(circuit::Mode::Public, *self.local_state_root);
        // Inject the 'is_global' flag as `Mode::Private`.
        let is_global = circuit::Boolean::<A>::new(circuit::Mode::Private, self.is_global);

        // Inject the serial number as `Mode::Public`.
        let serial_number = circuit::Field::<A>::new(circuit::Mode::Public, self.serial_number);
        // Compute the candidate serial number.
        let candidate_serial_number =
            circuit::Record::<A, circuit::Plaintext<A>>::serial_number_from_gamma(&gamma, commitment.clone());
        // Enforce that the candidate serial number is equal to the serial number.
        A::assert_eq(candidate_serial_number, serial_number);

        // Enforce the starting leaf is the claimed commitment.
        A::assert_eq(state_path.transition_leaf().id(), commitment);
        // Enforce the state path from leaf to root is correct.
        A::assert(state_path.verify(&is_global, &local_state_root));

        #[cfg(debug_assertions)]
        Stack::log_circuit::<A, _>(&format!("State Path for {}", self.serial_number));

        // Eject the assignment and reset the circuit environment.
        Ok(A::eject_assignment_and_reset())
    }
}
