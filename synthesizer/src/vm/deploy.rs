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

use super::*;

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// Returns a new deploy transaction.
    ///
    /// If a `fee_record` is provided, then a private fee will be included in the transaction;
    /// otherwise, a public fee will be included in the transaction.
    ///
    /// The `priority_fee_in_microcredits` is an additional fee **on top** of the deployment fee.
    pub fn deploy<R: Rng + CryptoRng>(
        &self,
        private_key: &PrivateKey<N>,
        program: &Program<N>,
        fee_record: Option<Record<N, Plaintext<N>>>,
        priority_fee_in_microcredits: u64,
        query: Option<&dyn QueryTrait<N>>,
        rng: &mut R,
    ) -> Result<Transaction<N>> {
        // Compute the deployment.
        let mut deployment = self.deploy_raw(program, rng)?;
        // Ensure the transaction is not empty.
        ensure!(!deployment.program().functions().is_empty(), "Attempted to create an empty transaction deployment");
        // Get a default query if one is not provided.
        let query = match query {
            Some(q) => q,
            None => &Query::VM(self.block_store().clone()),
        };
        // If the `CONSENSUS_VERSION` is less than `V9`, unset the program checksum and the owner.
        // Otherwise, swap the default owner with the address of the private key.
        let consensus_version = N::CONSENSUS_VERSION(query.current_block_height()?)?;
        if consensus_version < ConsensusVersion::V9 {
            deployment.set_program_checksum_raw(None);
            deployment.set_program_owner_raw(None)
        } else {
            deployment.set_program_checksum_raw(Some(deployment.program().to_checksum()));
            deployment.set_program_owner_raw(Some(Address::try_from(private_key)?));
        }
        // Compute the deployment ID.
        let deployment_id = deployment.to_deployment_id()?;
        // Construct the owner.
        let owner = ProgramOwner::new(private_key, deployment_id, rng)?;

        // Compute the minimum deployment cost.
        let (minimum_deployment_cost, _) = deployment_cost(&self.process().read(), &deployment)?;
        // Authorize the fee.
        let fee_authorization = match fee_record {
            Some(record) => self.authorize_fee_private(
                private_key,
                record,
                minimum_deployment_cost,
                priority_fee_in_microcredits,
                deployment_id,
                rng,
            )?,
            None => self.authorize_fee_public(
                private_key,
                minimum_deployment_cost,
                priority_fee_in_microcredits,
                deployment_id,
                rng,
            )?,
        };
        // Compute the fee.
        let fee = self.execute_fee_authorization(fee_authorization, Some(query), rng)?;

        // Return the deploy transaction.
        Transaction::from_deployment(owner, deployment, fee)
    }
}

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// Returns a deployment for the given program.
    #[inline]
    pub(super) fn deploy_raw<R: Rng + CryptoRng>(&self, program: &Program<N>, rng: &mut R) -> Result<Deployment<N>> {
        macro_rules! logic {
            ($process:expr, $network:path, $aleo:path) => {{
                // Prepare the program.
                let program = cast_ref!(&program as Program<$network>);
                // Compute the deployment.
                let deployment = $process.deploy::<$aleo, _>(program, rng)?;
                // Prepare the deployment.
                Ok(cast_ref!(deployment as Deployment<N>).clone())
            }};
        }

        // Compute the deployment.
        let timer = timer!("VM::deploy_raw");
        let result = process!(self, logic);
        finish!(timer, "Compute the deployment");
        result
    }
}
