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

mod bytes;
mod serialize;
mod string;

use console::{network::prelude::*, program::Request, types::Field};
use snarkvm_ledger_block::{Input, Transaction, Transition};
use snarkvm_synthesizer_program::StackTrait;

use indexmap::IndexMap;
#[cfg(feature = "locktick")]
use locktick::parking_lot::RwLock;
#[cfg(not(feature = "locktick"))]
use parking_lot::RwLock;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

#[derive(Clone)]
pub struct Authorization<N: Network> {
    /// The authorized requests.
    requests: Arc<RwLock<VecDeque<Request<N>>>>,
    /// The authorized transitions.
    transitions: Arc<RwLock<IndexMap<N::TransitionID, Transition<N>>>>,
}

impl<N: Network> Authorization<N> {
    /// Initialize a new `Authorization` instance, with the given request.
    pub fn new(request: Request<N>) -> Self {
        Self { requests: Arc::new(RwLock::new(VecDeque::from(vec![request]))), transitions: Default::default() }
    }

    /// Returns a new and independent replica of the authorization.
    pub fn replicate(&self) -> Self {
        Self {
            requests: Arc::new(RwLock::new(self.requests.read().clone())),
            transitions: Arc::new(RwLock::new(self.transitions.read().clone())),
        }
    }
}

impl<N: Network> TryFrom<(Vec<Request<N>>, Vec<Transition<N>>)> for Authorization<N> {
    type Error = Error;

    /// Initialize an `Authorization` instance, with the given requests and transitions.
    ///
    /// Note: This method is used primarily for serialization, and requires the
    /// number of requests and transitions to match. Requests are added to an authorization
    /// in the pre-order traversal sequence (parents before children), while transitions
    /// are added to an authorization in the post-order traversal sequence (children before parents).
    /// This method takes in as input requests and transitions in pre-order and post-order traversal
    /// sequence, respectively, which is why the requests and transitions need to be checked independent of the ordering
    fn try_from((requests, transitions): (Vec<Request<N>>, Vec<Transition<N>>)) -> Result<Self> {
        // Ensure the number of requests and transitions matches.
        ensure!(
            requests.len() == transitions.len(),
            "The number of requests ({}) and transitions ({}) must match in the authorization.",
            requests.len(),
            transitions.len()
        );

        // Build a map of transition commitments to their request indices
        let tcm_indices: HashMap<_, _> = requests.iter().enumerate().map(|(i, request)| (request.tcm(), i)).collect();

        // Ensure the requests and transitions match
        for (index, transition) in transitions.iter().enumerate() {
            let request_idx = tcm_indices
                .get(&transition.tcm())
                .copied()
                .ok_or_else(|| anyhow!("Missing request for transition {}", transition.id()))?;

            ensure_request_and_transition_matches(index, &requests[request_idx], transition)?;
        }
        // Return the new `Authorization` instance.
        Ok(Self {
            requests: Arc::new(RwLock::new(VecDeque::from(requests))),
            transitions: Arc::new(RwLock::new(IndexMap::from_iter(
                transitions.into_iter().map(|transition| (*transition.id(), transition)),
            ))),
        })
    }
}

impl<N: Network> Authorization<N> {
    /// Returns `true` if the authorization is for call to `credits.aleo/fee_private`.
    pub fn is_fee_private(&self) -> bool {
        let requests = self.requests.read();
        match requests.len() {
            1 => {
                let program_id = requests[0].program_id().to_string();
                let function_name = requests[0].function_name().to_string();
                &program_id == "credits.aleo" && &function_name == "fee_private"
            }
            _ => false,
        }
    }

    /// Returns `true` if the authorization is for call to `credits.aleo/fee_public`.
    pub fn is_fee_public(&self) -> bool {
        let requests = self.requests.read();
        match requests.len() {
            1 => {
                let program_id = requests[0].program_id().to_string();
                let function_name = requests[0].function_name().to_string();
                &program_id == "credits.aleo" && &function_name == "fee_public"
            }
            _ => false,
        }
    }

    /// Returns `true` if the authorization is for call to `credits.aleo/split`.
    pub fn is_split(&self) -> bool {
        let requests = self.requests.read();
        match requests.len() {
            1 => {
                let program_id = requests[0].program_id().to_string();
                let function_name = requests[0].function_name().to_string();
                &program_id == "credits.aleo" && &function_name == "split"
            }
            _ => false,
        }
    }

    /// Returns `true` if the authorization is for call to `credits.aleo/upgrade`.
    pub fn is_upgrade(&self) -> bool {
        let requests = self.requests.read();
        match requests.len() {
            1 => {
                let program_id = requests[0].program_id().to_string();
                let function_name = requests[0].function_name().to_string();
                &program_id == "credits.aleo" && &function_name == "upgrade"
            }
            _ => false,
        }
    }

    /// Checks whether the authorization is for a valid program edition.
    pub fn check_valid_edition(&self, process: &crate::Process<N>, _consensus_version: ConsensusVersion) -> Result<()> {
        // Determine the root transition's program ID.
        let transitions = self.transitions.read();
        let program_ids = transitions.iter().map(|(_, t)| t.program_id());
        for program_id in program_ids {
            // There is only one credits.aleo edition, so we can safely skip this case.
            if program_id.to_string() != "credits.aleo" {
                // Get the stack.
                let stack = process.get_stack(program_id)?;
                // Get the program edition.
                let _program_edition = *stack.program_edition();
                // If we're past `ConsensusVersion::V8` but before `ConsensusVersion::V9`, ensure new stacks are not on edition 0.
                // Note. This check does not apply to programs with constructors.
                #[cfg(not(any(test, feature = "test")))]
                if _consensus_version >= ConsensusVersion::V8
                    && _program_edition == 0
                    && !stack.program().contains_constructor()
                {
                    bail!("Cannot execute {program_id} on edition {_program_edition}");
                }
            }
        }
        Ok(())
    }

    /// Checks whether the authorization creates valid records.
    pub fn check_valid_records(&self, consensus_version: ConsensusVersion) -> Result<()> {
        let transitions = self.transitions.read();
        // Collect the transition's output records.
        let output_records = transitions
            .values()
            .flat_map(|transition| transition.outputs().iter().filter_map(|output| output.record()));
        // Ensure the records are valid.
        for (_, record) in output_records {
            // if the consensus version is on or after `ConsensusVersion::V8`, ensure the output record is on Version 1.
            if consensus_version >= ConsensusVersion::V8 {
                ensure!(record.version().is_one(), "Output record must be Version 1 on or after Consensus V8");
            }
            // We do not impose checks on record versions before
            // `ConsensusVersion::V8`, to avoid:
            // - breaking snarkOS tests which have version 1 records in their genesis blocks.
            // - breaking production wallets which use version 0 records before ConsensusVersion::V8.
        }
        Ok(())
    }
}

impl<N: Network> Authorization<N> {
    /// Returns the next `Request` in the authorization.
    pub fn peek_next(&self) -> Result<Request<N>> {
        self.requests.read().front().cloned().ok_or_else(|| anyhow!("Failed to peek at the next request."))
    }

    /// Returns the next `Request` from the authorization.
    pub fn next(&self) -> Result<Request<N>> {
        self.requests.write().pop_front().ok_or_else(|| anyhow!("No more requests in the authorization."))
    }

    /// Returns the `Request` at the given index.
    pub fn get(&self, index: usize) -> Result<Request<N>> {
        self.requests.read().get(index).cloned().ok_or_else(|| anyhow!("Attempted to get missing request {index}."))
    }

    /// Returns the number of `Request`s in the authorization.
    pub fn len(&self) -> usize {
        self.requests.read().len()
    }

    /// Return `true` if the authorization is empty.
    pub fn is_empty(&self) -> bool {
        self.requests.read().is_empty()
    }

    /// Appends the given `Request` to the authorization.
    pub fn push(&self, request: Request<N>) -> Result<()> {
        // Check that the number of requests is less than the maximum.
        ensure!(
            self.len() < Transaction::<N>::MAX_TRANSITIONS,
            "The number of requests in the authorization must be less than '{}'.",
            Transaction::<N>::MAX_TRANSITIONS
        );
        // Append the request to the authorization.
        self.requests.write().push_back(request);
        Ok(())
    }

    /// Returns the requests in the authorization.
    pub fn to_vec_deque(&self) -> VecDeque<Request<N>> {
        self.requests.read().clone()
    }
}

impl<N: Network> Authorization<N> {
    /// Inserts the given transition into the authorization.
    pub fn insert_transition(&self, transition: Transition<N>) -> Result<()> {
        // Ensure the transition is not already in the authorization.
        ensure!(
            !self.transitions.read().contains_key(transition.id()),
            "Transition {} is already in the authorization.",
            transition.id()
        );
        // Insert the transition into the authorization.
        self.transitions.write().insert(*transition.id(), transition);
        Ok(())
    }

    /// Returns the transitions in the authorization.
    pub fn transitions(&self) -> IndexMap<N::TransitionID, Transition<N>> {
        self.transitions.read().clone()
    }

    /// Returns the execution ID for the authorization.
    pub fn to_execution_id(&self) -> Result<Field<N>> {
        let transitions = self.transitions.read();
        if transitions.is_empty() {
            bail!("Cannot compute the execution ID for an empty authorization.");
        }
        Ok(*Transaction::transitions_tree(transitions.values())?.root())
    }
}

impl<N: Network> PartialEq for Authorization<N> {
    fn eq(&self, other: &Self) -> bool {
        let self_requests = self.requests.read();
        let other_requests = other.requests.read();

        let self_transitions = self.transitions.read();
        let other_transitions = other.transitions.read();

        *self_requests == *other_requests && *self_transitions == *other_transitions
    }
}

impl<N: Network> Eq for Authorization<N> {}

impl<N: Network> Authorization<N> {
    /// Returns the total number of inputs to the passed `Transition`s that are of
    /// type `Input::Record`.
    // This method is used to ensure consistency between `prepare_verifier_inputs`
    // and the batch-size calculation used in `execution_cost_for_authorization`.
    #[inline]
    pub fn number_of_input_records<'a>(transitions: impl ExactSizeIterator<Item = &'a Transition<N>>) -> usize {
        transitions
            .map(|transition| transition.inputs().iter().filter(|input| matches!(input, Input::Record(_, _))).count())
            .sum()
    }
}

/// Ensures the given request and transition correspond to one another.
fn ensure_request_and_transition_matches<N: Network>(
    index: usize,
    request: &Request<N>,
    transition: &Transition<N>,
) -> Result<()> {
    // Ensure the request and transition have the same program ID.
    ensure!(
        request.program_id() == transition.program_id(),
        "The request ({}) and transition ({}) at index {index} must have the same program ID in the authorization.",
        request.program_id(),
        transition.program_id(),
    );
    // Ensure the request and transition have the same function name.
    ensure!(
        request.function_name() == transition.function_name(),
        "The request ({}) and transition ({}) at index {index} must have the same function name in the authorization.",
        request.function_name(),
        transition.function_name(),
    );
    // Ensure the request and transition have the same number of inputs.
    ensure!(
        request.input_ids().len() == transition.input_ids().len(),
        "The request ({}) and transition ({}) at index {index} must have the same number of inputs in the authorization.",
        request.input_ids().len(),
        transition.input_ids().len(),
    );
    // Ensure the request and transition have the same 'tpk'.
    ensure!(
        request.to_tpk() == *transition.tpk(),
        "The request ({}) and transition ({}) at index {index} must have the same 'tpk' in the authorization.",
        request.to_tpk(),
        *transition.tpk(),
    );
    // Ensure the request and transition have the same 'tcm'.
    ensure!(
        request.tcm() == transition.tcm(),
        "The request ({}) and transition ({}) at index {index} must have the same 'tcm' in the authorization.",
        request.tcm(),
        transition.tcm(),
    );
    // Ensure the request and transition have the same 'scm'.
    ensure!(
        request.scm() == transition.scm(),
        "The request ({}) and transition ({}) at index {index} must have the same 'scm' in the authorization.",
        request.scm(),
        transition.scm(),
    );
    Ok(())
}

#[cfg(feature = "test")]
impl<N: Network> Authorization<N> {
    /// Initialize an `Authorization` instance with the given requests and
    /// transitions without performing any consistency checks between them.
    pub fn from_unchecked((requests, transitions): (Vec<Request<N>>, Vec<Transition<N>>)) -> Self {
        Authorization {
            requests: Arc::new(RwLock::new(VecDeque::from(requests))),
            transitions: Arc::new(RwLock::new(IndexMap::from_iter(
                transitions.into_iter().map(|transition| (*transition.id(), transition)),
            ))),
        }
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use crate::{Identifier, Process, ProgramID, Value};
    use console::account::{Address, PrivateKey};

    type CurrentNetwork = console::network::MainnetV0;
    type CurrentAleo = circuit::AleoV0;

    /// Returns a sample authorization.
    pub fn sample_authorization(rng: &mut TestRng) -> Authorization<CurrentNetwork> {
        // Initialize the process.
        let process = Process::<CurrentNetwork>::load().unwrap();

        // Sample a private key.
        let private_key = PrivateKey::new(rng).unwrap();
        // Sample a base fee in microcredits.
        let base_fee_in_microcredits = rng.gen_range(1_000_000..u64::MAX / 2);
        // Sample a priority fee in microcredits.
        let priority_fee_in_microcredits = rng.gen_range(0..u64::MAX / 2);
        // Sample a deployment or execution ID.
        let deployment_or_execution_id = Field::rand(rng);

        // Compute the authorization.
        let authorization = process
            .authorize_fee_public::<CurrentAleo, _>(
                &private_key,
                base_fee_in_microcredits,
                priority_fee_in_microcredits,
                deployment_or_execution_id,
                rng,
            )
            .unwrap();
        assert!(authorization.is_fee_public(), "Authorization must be for a call to 'credits.aleo/fee_public'");
        authorization
    }

    #[test]
    fn test_single_transition_authorization_deserialization() {
        let rng = &mut TestRng::default();

        // Sample a private key.
        let private_key = PrivateKey::new(rng).unwrap();

        // Initialize the process.
        let process = Process::<CurrentNetwork>::load().unwrap();

        // Specify the program ID
        let program_id = ProgramID::<CurrentNetwork>::from_str("credits.aleo").unwrap();

        // Specify the function name
        let function_name = Identifier::<CurrentNetwork>::from_str("transfer_public").unwrap();

        // Generate the inputs
        let destination =
            Value::<CurrentNetwork>::from_str(&format!("{}", Address::try_from(private_key).unwrap())).unwrap();
        let amount = Value::<CurrentNetwork>::from_str("1u64").unwrap();

        // Generate the Authorization
        let authorization = process
            .authorize::<CurrentAleo, _>(
                &private_key,
                &program_id,
                &function_name,
                vec![destination, amount].iter(),
                rng,
            )
            .unwrap();

        // Assert there is only 1 transition
        assert!(authorization.transitions().len() == 1);

        // Serialize the Authorization into a String
        let authorization_serialized = authorization.to_string();

        // Attempt to deserialize the Authorization from String
        let deserialization_result = Authorization::<CurrentNetwork>::from_str(&authorization_serialized);

        // Assert that the deserialization result is Ok
        assert!(deserialization_result.is_ok());
    }
}
