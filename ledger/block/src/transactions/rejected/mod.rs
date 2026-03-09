// Copyright (c) 2019-2026 Provable Inc.
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

use super::*;

use crate::{Deployment, Execution, Fee};

/// A wrapper around the rejected deployment or execution.
#[derive(Clone, PartialEq, Eq)]
pub enum Rejected<N: Network> {
    Deployment(ProgramOwner<N>, Box<Deployment<N>>, Option<RejectedReason<N>>),
    Execution(Box<Execution<N>>, Option<RejectedReason<N>>),
}

impl<N: Network> Rejected<N> {
    /// Initializes a rejected deployment.
    pub fn new_deployment(
        program_owner: ProgramOwner<N>,
        deployment: Deployment<N>,
        rejected_reason: Option<RejectedReason<N>>,
    ) -> Self {
        Self::Deployment(program_owner, Box::new(deployment), rejected_reason)
    }

    /// Initializes a rejected execution.
    pub fn new_execution(execution: Execution<N>, rejected_reason: Option<RejectedReason<N>>) -> Self {
        Self::Execution(Box::new(execution), rejected_reason)
    }

    /// Returns true if the rejected transaction is a deployment.
    pub fn is_deployment(&self) -> bool {
        matches!(self, Self::Deployment(..))
    }

    /// Returns true if the rejected transaction is an execution.
    pub fn is_execution(&self) -> bool {
        matches!(self, Self::Execution(..))
    }

    /// Returns the program owner of the rejected deployment.
    pub fn program_owner(&self) -> Option<&ProgramOwner<N>> {
        match self {
            Self::Deployment(program_owner, _, _) => Some(program_owner),
            Self::Execution(_, _) => None,
        }
    }

    /// Returns the rejected deployment.
    pub fn deployment(&self) -> Option<&Deployment<N>> {
        match self {
            Self::Deployment(_, deployment, _) => Some(deployment),
            Self::Execution(_, _) => None,
        }
    }

    /// Returns the rejected execution.
    pub fn execution(&self) -> Option<&Execution<N>> {
        match self {
            Self::Deployment(_, _, _) => None,
            Self::Execution(execution, _) => Some(execution),
        }
    }

    /// Returns the rejected reason.
    pub fn rejected_reason(&self) -> &Option<RejectedReason<N>> {
        match self {
            Self::Deployment(_, _, rejected_reason) => rejected_reason,
            Self::Execution(_, rejected_reason) => rejected_reason,
        }
    }

    /// Returns the rejected ID.
    pub fn to_id(&self) -> Result<Field<N>> {
        match self {
            Self::Deployment(_, deployment, _) => deployment.to_deployment_id(),
            Self::Execution(execution, _) => execution.to_execution_id(),
        }
    }

    /// Returns the unconfirmed transaction ID, which is defined as the transaction ID prior to confirmation.
    /// When a transaction is rejected, its fee transition is used to construct the confirmed transaction ID,
    /// changing the original transaction ID.
    pub fn to_unconfirmed_id(&self, fee: &Option<Fee<N>>) -> Result<Field<N>> {
        // Compute the deployment or execution tree.
        let tree = match self {
            Self::Deployment(_, deployment, _) => Transaction::deployment_tree(deployment)?,
            Self::Execution(execution, _) => Transaction::execution_tree(execution)?,
        };
        // Construct the transaction tree and return the unconfirmed transaction ID.
        Ok(*Transaction::transaction_tree(tree, fee.as_ref())?.root())
    }
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use console::{account::PrivateKey, network::MainnetV0};

    type CurrentNetwork = MainnetV0;

    /// Samples a rejected deployment.
    pub(crate) fn sample_rejected_deployment(
        version: u8,
        edition: u16,
        is_fee_private: bool,
        has_rejected_reason: bool,
        rng: &mut TestRng,
    ) -> Rejected<CurrentNetwork> {
        // Sample a deploy transaction.
        let deployment = match crate::transaction::test_helpers::sample_deployment_transaction(
            version,
            edition,
            is_fee_private,
            rng,
        ) {
            Transaction::Deploy(_, _, _, deployment, _) => (*deployment).clone(),
            _ => unreachable!(),
        };

        // Sample a new program owner.
        let private_key = PrivateKey::new(rng).unwrap();
        let deployment_id = deployment.to_deployment_id().unwrap();
        let program_owner = ProgramOwner::new(&private_key, deployment_id, rng).unwrap();

        // Sample a rejected reason for the deployment.
        let rejected_reason = match has_rejected_reason {
            true => Some(RejectedReason::DuplicateProgramID(*deployment.program_id())),
            false => None,
        };

        // Return the rejected deployment.
        Rejected::new_deployment(program_owner, deployment, rejected_reason)
    }

    /// Samples a rejected execution.
    pub(crate) fn sample_rejected_execution(
        is_fee_private: bool,
        has_rejected_reason: bool,
        rng: &mut TestRng,
    ) -> Rejected<CurrentNetwork> {
        // Sample an execute transaction.
        let execution =
            match crate::transaction::test_helpers::sample_execution_transaction_with_fee(is_fee_private, rng, 0) {
                Transaction::Execute(_, _, execution, _) => execution,
                _ => unreachable!(),
            };

        // Sample a rejected reason for the execution.
        let rejected_reason = match has_rejected_reason {
            true => Some(RejectedReason::NonFinalize(
                Some(("credits.aleo".parse::<ProgramID<CurrentNetwork>>().unwrap(), 0u16)),
                Some("transfer_public".parse::<Identifier<CurrentNetwork>>().unwrap()),
            )),
            false => None,
        };

        // Return the rejected execution.
        Rejected::new_execution(*execution, rejected_reason)
    }

    /// Sample a list of randomly rejected transactions.
    pub(crate) fn sample_rejected_transactions() -> Vec<Rejected<CurrentNetwork>> {
        let rng = &mut TestRng::default();

        vec![
            sample_rejected_deployment(1, 0, true, false, rng),
            sample_rejected_deployment(1, 0, false, false, rng),
            sample_rejected_deployment(2, 0, true, true, rng),
            sample_rejected_deployment(2, 0, false, true, rng),
            sample_rejected_deployment(1, 1, true, false, rng),
            sample_rejected_deployment(1, 1, false, false, rng),
            sample_rejected_deployment(2, 1, true, true, rng),
            sample_rejected_deployment(2, 1, true, false, rng),
            sample_rejected_deployment(2, 1, false, true, rng),
            sample_rejected_deployment(2, 1, false, false, rng),
            sample_rejected_execution(true, true, rng),
            sample_rejected_execution(true, false, rng),
            sample_rejected_execution(false, true, rng),
            sample_rejected_execution(false, false, rng),
        ]
    }
}
