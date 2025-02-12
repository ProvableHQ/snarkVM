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

mod deployment;
pub use deployment::*;

mod execution;
pub use execution::*;

mod fee;
pub use fee::*;

mod bytes;
mod merkle;
mod serialize;
mod string;

use crate::Transition;
use console::{
    network::prelude::*,
    program::{Ciphertext, ProgramOwner, Record, TRANSACTION_DEPTH, TransactionLeaf, TransactionPath, TransactionTree},
    types::{Field, Group, U64},
};

#[derive(Clone, PartialEq, Eq)]
pub enum Transaction<N: Network> {
    /// The deploy transaction publishes an Aleo program to the network.
    Deploy(N::TransactionID, ProgramOwner<N>, Box<Deployment<N>>, Fee<N>),
    /// The execute transaction represents a call to an Aleo program.
    Execute(N::TransactionID, Execution<N>, Option<Fee<N>>),
    /// The fee transaction represents a fee paid to the network, used for rejected transactions.
    Fee(N::TransactionID, Fee<N>),
}

impl<N: Network> Transaction<N> {
    /// Initializes a new deployment transaction.
    pub fn from_deployment(owner: ProgramOwner<N>, deployment: Deployment<N>, fee: Fee<N>) -> Result<Self> {
        // Ensure the transaction is not empty.
        ensure!(!deployment.program().functions().is_empty(), "Attempted to create an empty deployment transaction");
        // Compute the transaction ID.
        let id = *Self::deployment_tree(&deployment, Some(&fee))?.root();
        // Compute the deployment ID.
        let deployment_id = deployment.to_deployment_id()?;
        // Ensure the owner signed the correct transaction ID.
        ensure!(owner.verify(deployment_id), "Attempted to create a deployment transaction with an invalid owner");
        // Construct the deployment transaction.
        Ok(Self::Deploy(id.into(), owner, Box::new(deployment), fee))
    }

    /// Initializes a new execution transaction.
    pub fn from_execution(execution: Execution<N>, fee: Option<Fee<N>>) -> Result<Self> {
        // Ensure the transaction is not empty.
        ensure!(!execution.is_empty(), "Attempted to create an empty execution transaction");
        // Compute the transaction ID.
        let id = *Self::execution_tree(&execution, &fee)?.root();
        // Construct the execution transaction.
        Ok(Self::Execute(id.into(), execution, fee))
    }

    /// Initializes a new fee transaction.
    pub fn from_fee(fee: Fee<N>) -> Result<Self> {
        // Ensure the fee is nonzero.
        ensure!(!fee.is_zero()?, "Attempted to create a zero fee transaction");
        // Compute the transaction ID.
        let id = *Self::fee_tree(&fee)?.root();
        // Construct the execution transaction.
        Ok(Self::Fee(id.into(), fee))
    }
}

impl<N: Network> Transaction<N> {
    /// Returns `true` if the transaction is a deploy transaction.
    #[inline]
    pub const fn is_deploy(&self) -> bool {
        matches!(self, Self::Deploy(..))
    }

    /// Returns `true` if the transaction is an execute transaction.
    #[inline]
    pub const fn is_execute(&self) -> bool {
        matches!(self, Self::Execute(..))
    }

    /// Returns `true` if the transaction is a fee transaction.
    #[inline]
    pub const fn is_fee(&self) -> bool {
        matches!(self, Self::Fee(..))
    }
}

impl<N: Network> Transaction<N> {
    /// Returns `true` if this transaction contains a call to `credits.aleo/split`.
    #[inline]
    pub fn contains_split(&self) -> bool {
        match self {
            // Case 1 - The transaction contains a transition that calls 'credits.aleo/split'.
            Transaction::Execute(_, execution, _) => execution.transitions().any(|transition| transition.is_split()),
            // Otherwise, return 'false'.
            _ => false,
        }
    }
}

impl<N: Network> Transaction<N> {
    /// Returns `Some(owner)` if the transaction is a deployment. Otherwise, returns `None`.
    #[inline]
    pub fn owner(&self) -> Option<&ProgramOwner<N>> {
        match self {
            Self::Deploy(_, owner, _, _) => Some(owner),
            _ => None,
        }
    }

    /// Returns `Some(deployment)` if the transaction is a deployment. Otherwise, returns `None`.
    #[inline]
    pub fn deployment(&self) -> Option<&Deployment<N>> {
        match self {
            Self::Deploy(_, _, deployment, _) => Some(deployment.as_ref()),
            _ => None,
        }
    }

    /// Returns `Some(execution)` if the transaction is an execution. Otherwise, returns `None`.
    #[inline]
    pub fn execution(&self) -> Option<&Execution<N>> {
        match self {
            Self::Execute(_, execution, _) => Some(execution),
            _ => None,
        }
    }
}

/// A helper enum for iterators and consuming iterators over a transaction.
enum IterWrap<T, I1: Iterator<Item = T>, I2: Iterator<Item = T>, I3: Iterator<Item = T>> {
    Deploy(I1),
    Execute(I2),
    Fee(I3),
}

impl<T, I1: Iterator<Item = T>, I2: Iterator<Item = T>, I3: Iterator<Item = T>> Iterator for IterWrap<T, I1, I2, I3> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Deploy(iter) => iter.next(),
            Self::Execute(iter) => iter.next(),
            Self::Fee(iter) => iter.next(),
        }
    }
}

impl<T, I1: DoubleEndedIterator<Item = T>, I2: DoubleEndedIterator<Item = T>, I3: DoubleEndedIterator<Item = T>>
    DoubleEndedIterator for IterWrap<T, I1, I2, I3>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Deploy(iter) => iter.next_back(),
            Self::Execute(iter) => iter.next_back(),
            Self::Fee(iter) => iter.next_back(),
        }
    }
}

impl<N: Network> Transaction<N> {
    /// Returns the transaction ID.
    pub const fn id(&self) -> N::TransactionID {
        match self {
            Self::Deploy(id, ..) => *id,
            Self::Execute(id, ..) => *id,
            Self::Fee(id, ..) => *id,
        }
    }

    /// Returns the transaction total fee.
    pub fn fee_amount(&self) -> Result<U64<N>> {
        match self {
            Self::Deploy(_, _, _, fee) => fee.amount(),
            Self::Execute(_, _, Some(fee)) => fee.amount(),
            Self::Execute(_, _, None) => Ok(U64::zero()),
            Self::Fee(_, fee) => fee.amount(),
        }
    }

    /// Returns the transaction base fee.
    pub fn base_fee_amount(&self) -> Result<U64<N>> {
        match self {
            Self::Deploy(_, _, _, fee) => fee.base_amount(),
            Self::Execute(_, _, Some(fee)) => fee.base_amount(),
            Self::Execute(_, _, None) => Ok(U64::zero()),
            Self::Fee(_, fee) => fee.base_amount(),
        }
    }

    /// Returns the transaction priority fee.
    pub fn priority_fee_amount(&self) -> Result<U64<N>> {
        match self {
            Self::Deploy(_, _, _, fee) => fee.priority_amount(),
            Self::Execute(_, _, Some(fee)) => fee.priority_amount(),
            Self::Execute(_, _, None) => Ok(U64::zero()),
            Self::Fee(_, fee) => fee.priority_amount(),
        }
    }

    /// Returns the fee transition.
    pub fn fee_transition(&self) -> Option<Fee<N>> {
        match self {
            Self::Deploy(_, _, _, fee) => Some(fee.clone()),
            Self::Execute(_, _, fee) => fee.clone(),
            Self::Fee(_, fee) => Some(fee.clone()),
        }
    }
}

impl<N: Network> Transaction<N> {
    /// Returns `true` if the transaction contains the given transition ID.
    pub fn contains_transition(&self, transition_id: &N::TransitionID) -> bool {
        match self {
            // Check the fee.
            Self::Deploy(_, _, _, fee) => fee.id() == transition_id,
            // Check the execution and fee.
            Self::Execute(_, execution, fee) => {
                execution.contains_transition(transition_id)
                    || fee.as_ref().map_or(false, |fee| fee.id() == transition_id)
            }
            // Check the fee.
            Self::Fee(_, fee) => fee.id() == transition_id,
        }
    }

    /// Returns `true` if the transaction contains the given serial number.
    pub fn contains_serial_number(&self, serial_number: &Field<N>) -> bool {
        self.transitions().any(|transition| transition.contains_serial_number(serial_number))
    }

    /// Returns `true` if the transaction contains the given commitment.
    pub fn contains_commitment(&self, commitment: &Field<N>) -> bool {
        self.transitions().any(|transition| transition.contains_commitment(commitment))
    }
}

impl<N: Network> Transaction<N> {
    /// Returns the transition with the corresponding transition ID, if it exists.
    pub fn find_transition(&self, transition_id: &N::TransitionID) -> Option<&Transition<N>> {
        match self {
            // Check the fee.
            Self::Deploy(_, _, _, fee) => match fee.id() == transition_id {
                true => Some(fee.transition()),
                false => None,
            },
            // Check the execution and fee.
            Self::Execute(_, execution, fee) => execution.get_transition(transition_id).or_else(|| {
                fee.as_ref().and_then(|fee| match fee.id() == transition_id {
                    true => Some(fee.transition()),
                    false => None,
                })
            }),
            // Check the fee.
            Self::Fee(_, fee) => match fee.id() == transition_id {
                true => Some(fee.transition()),
                false => None,
            },
        }
    }

    /// Returns the transition for the given serial number, if it exists.
    pub fn find_transition_for_serial_number(&self, serial_number: &Field<N>) -> Option<&Transition<N>> {
        self.transitions().find(|transition| transition.contains_serial_number(serial_number))
    }

    /// Returns the transition for the given commitment, if it exists.
    pub fn find_transition_for_commitment(&self, commitment: &Field<N>) -> Option<&Transition<N>> {
        self.transitions().find(|transition| transition.contains_commitment(commitment))
    }

    /// Returns the record with the corresponding commitment, if it exists.
    pub fn find_record(&self, commitment: &Field<N>) -> Option<&Record<N, Ciphertext<N>>> {
        self.transitions().find_map(|transition| transition.find_record(commitment))
    }
}

impl<N: Network> Transaction<N> {
    /// Returns an iterator over the transition IDs, for all transitions.
    pub fn transition_ids(&self) -> impl '_ + DoubleEndedIterator<Item = &N::TransitionID> {
        self.transitions().map(Transition::id)
    }

    /// Returns an iterator over all transitions.
    pub fn transitions(&self) -> impl '_ + DoubleEndedIterator<Item = &Transition<N>> {
        match self {
            Self::Deploy(_, _, _, fee) => IterWrap::Deploy(Some(fee.transition()).into_iter()),
            Self::Execute(_, execution, fee) => {
                IterWrap::Execute(execution.transitions().chain(fee.as_ref().map(|fee| fee.transition())))
            }
            Self::Fee(_, fee) => IterWrap::Fee(Some(fee.transition()).into_iter()),
        }
    }

    /* Input */

    /// Returns an iterator over the input IDs, for all transition inputs that are records.
    pub fn input_ids(&self) -> impl '_ + Iterator<Item = &Field<N>> {
        self.transitions().flat_map(Transition::input_ids)
    }

    /// Returns an iterator over the serial numbers, for all transition inputs that are records.
    pub fn serial_numbers(&self) -> impl '_ + Iterator<Item = &Field<N>> {
        self.transitions().flat_map(Transition::serial_numbers)
    }

    /// Returns an iterator over the tags, for all transition inputs that are records.
    pub fn tags(&self) -> impl '_ + Iterator<Item = &Field<N>> {
        self.transitions().flat_map(Transition::tags)
    }

    /* Output */

    /// Returns an iterator over the output IDs, for all transition inputs that are records.
    pub fn output_ids(&self) -> impl '_ + Iterator<Item = &Field<N>> {
        self.transitions().flat_map(Transition::output_ids)
    }

    /// Returns an iterator over the commitments, for all transition outputs that are records.
    pub fn commitments(&self) -> impl '_ + Iterator<Item = &Field<N>> {
        self.transitions().flat_map(Transition::commitments)
    }

    /// Returns an iterator over the records, for all transition outputs that are records.
    pub fn records(&self) -> impl '_ + Iterator<Item = (&Field<N>, &Record<N, Ciphertext<N>>)> {
        self.transitions().flat_map(Transition::records)
    }

    /// Returns an iterator over the nonces, for all transition outputs that are records.
    pub fn nonces(&self) -> impl '_ + Iterator<Item = &Group<N>> {
        self.transitions().flat_map(Transition::nonces)
    }

    /// Returns an iterator over the transition public keys, for all transitions.
    pub fn transition_public_keys(&self) -> impl '_ + DoubleEndedIterator<Item = &Group<N>> {
        self.transitions().map(Transition::tpk)
    }

    /// Returns an iterator over the transition commitments, for all transitions.
    pub fn transition_commitments(&self) -> impl '_ + DoubleEndedIterator<Item = &Field<N>> {
        self.transitions().map(Transition::tcm)
    }
}

impl<N: Network> Transaction<N> {
    /// Returns a consuming iterator over the transition IDs, for all transitions.
    pub fn into_transition_ids(self) -> impl Iterator<Item = N::TransitionID> {
        self.into_transitions().map(Transition::into_id)
    }

    /// Returns a consuming iterator over all transitions.
    pub fn into_transitions(self) -> impl DoubleEndedIterator<Item = Transition<N>> {
        match self {
            Self::Deploy(_, _, _, fee) => IterWrap::Deploy(Some(fee.into_transition()).into_iter()),
            Self::Execute(_, execution, fee) => {
                IterWrap::Execute(execution.into_transitions().chain(fee.map(|fee| fee.into_transition())))
            }
            Self::Fee(_, fee) => IterWrap::Fee(Some(fee.into_transition()).into_iter()),
        }
    }

    /// Returns a consuming iterator over the transition public keys, for all transitions.
    pub fn into_transition_public_keys(self) -> impl DoubleEndedIterator<Item = Group<N>> {
        self.into_transitions().map(Transition::into_tpk)
    }

    /// Returns a consuming iterator over the tags, for all transition inputs that are records.
    pub fn into_tags(self) -> impl Iterator<Item = Field<N>> {
        self.into_transitions().flat_map(Transition::into_tags)
    }

    /// Returns a consuming iterator over the serial numbers, for all transition inputs that are records.
    pub fn into_serial_numbers(self) -> impl Iterator<Item = Field<N>> {
        self.into_transitions().flat_map(Transition::into_serial_numbers)
    }

    /// Returns a consuming iterator over the commitments, for all transition outputs that are records.
    pub fn into_commitments(self) -> impl Iterator<Item = Field<N>> {
        self.into_transitions().flat_map(Transition::into_commitments)
    }

    /// Returns a consuming iterator over the records, for all transition outputs that are records.
    pub fn into_records(self) -> impl Iterator<Item = (Field<N>, Record<N, Ciphertext<N>>)> {
        self.into_transitions().flat_map(Transition::into_records)
    }

    /// Returns a consuming iterator over the nonces, for all transition outputs that are records.
    pub fn into_nonces(self) -> impl Iterator<Item = Group<N>> {
        self.into_transitions().flat_map(Transition::into_nonces)
    }
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use console::{account::PrivateKey, network::MainnetV0, program::ProgramOwner};

    type CurrentNetwork = MainnetV0;

    /// Samples a random deployment transaction with a private or public fee.
    pub fn sample_deployment_transaction(is_fee_private: bool, rng: &mut TestRng) -> Transaction<CurrentNetwork> {
        // Sample a private key.
        let private_key = PrivateKey::new(rng).unwrap();
        // Sample a deployment.
        let deployment = crate::transaction::deployment::test_helpers::sample_deployment(rng);

        // Compute the deployment ID.
        let deployment_id = deployment.to_deployment_id().unwrap();
        // Construct a program owner.
        let owner = ProgramOwner::new(&private_key, deployment_id, rng).unwrap();

        // Sample the fee.
        let fee = match is_fee_private {
            true => crate::transaction::fee::test_helpers::sample_fee_private(deployment_id, rng),
            false => crate::transaction::fee::test_helpers::sample_fee_public(deployment_id, rng),
        };

        // Construct a deployment transaction.
        Transaction::from_deployment(owner, deployment, fee).unwrap()
    }

    /// Samples a random execution transaction with a private or public fee.
    pub fn sample_execution_transaction_with_fee(
        is_fee_private: bool,
        rng: &mut TestRng,
    ) -> Transaction<CurrentNetwork> {
        // Sample an execution.
        let execution = crate::transaction::execution::test_helpers::sample_execution(rng);
        // Compute the execution ID.
        let execution_id = execution.to_execution_id().unwrap();

        // Sample the fee.
        let fee = match is_fee_private {
            true => crate::transaction::fee::test_helpers::sample_fee_private(execution_id, rng),
            false => crate::transaction::fee::test_helpers::sample_fee_public(execution_id, rng),
        };

        // Construct an execution transaction.
        Transaction::from_execution(execution, Some(fee)).unwrap()
    }

    /// Samples a random fee transaction.
    pub fn sample_private_fee_transaction(rng: &mut TestRng) -> Transaction<CurrentNetwork> {
        // Sample a fee.
        let fee = crate::transaction::fee::test_helpers::sample_fee_private_hardcoded(rng);
        // Construct a fee transaction.
        Transaction::from_fee(fee).unwrap()
    }

    /// Samples a random fee transaction.
    pub fn sample_fee_public_transaction(rng: &mut TestRng) -> Transaction<CurrentNetwork> {
        // Sample a fee.
        let fee = crate::transaction::fee::test_helpers::sample_fee_public_hardcoded(rng);
        // Construct a fee transaction.
        Transaction::from_fee(fee).unwrap()
    }
}
