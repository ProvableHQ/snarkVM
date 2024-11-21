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

impl<N: Network> Process<N> {
    /// Deploys the given program ID, if it does not exist.
    #[inline]
    pub fn deploy<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(
        &self,
        program: &Program<N>,
        rng: &mut R,
    ) -> Result<Deployment<N>> {
        let timer = timer!("Process::deploy");

        // Compute the stack.
        let stack = Stack::new(self, program)?;
        lap!(timer, "Compute the stack");

        // Return the deployment.
        let deployment = stack.deploy::<A, R>(rng);
        lap!(timer, "Construct the deployment");

        finish!(timer);

        deployment
    }

    /// Loads the stack and imported stacks for the given program ID into memory.
    #[inline]
    pub fn load_stack(&self, program_id: impl TryInto<ProgramID<N>>) -> Result<Arc<Stack<N>>> {
        let program_id = program_id.try_into().map_err(|_| anyhow!("Invalid program ID"))?;
        debug!("Lazy loading stack for {program_id}");
        // Retrieve the stores.
        let store = self.store.as_ref().ok_or_else(|| anyhow!("Failed to get store"))?;
        // Retrieve the transaction store.
        let transaction_store = store.transaction_store();

        // Retrieve the deployment store.
        let deployment_store = transaction_store.deployment_store();
        // Retrieve the transaction ID.
        let transaction_id = deployment_store
            .find_transaction_id_from_program_id(&program_id)
            .map_err(|e| anyhow!("Program ID not found in storage: {e}"))?
            .ok_or_else(|| anyhow!("Program ID not found in storage"))?;

        // Retrieve the deployment from the transaction ID.
        let deployment = match transaction_store.get_deployment(&transaction_id)? {
            Some(deployment) => deployment,
            None => bail!("Deployment transaction '{transaction_id}' is not found in storage."),
        };

        // Load the deployment into memory and return it.
        // When initializing the corresponding Stack, each import Stack will be loaded recursively.
        self.load_deployment(deployment)
    }

    /// Constructs, loads and returns the Stack from the deployment.
    /// This method assumes the given deployment **is valid**.
    #[inline]
    fn load_deployment(&self, deployment: Deployment<N>) -> Result<Arc<Stack<N>>> {
        let timer = timer!("Process::load_deployment");

        // Compute the program stack.
        let stack = Stack::new(self, deployment.program())?;
        lap!(timer, "Compute the stack");

        // Insert the verifying keys.
        for (function_name, (verifying_key, _)) in deployment.verifying_keys() {
            stack.insert_verifying_key(function_name, verifying_key.clone())?;
        }
        lap!(timer, "Insert the verifying keys");

        // Wrap the stack in an Arc.
        let stack = Arc::new(stack);

        // Add the stack to the process.
        self.add_stack(stack.clone())?;

        finish!(timer);

        Ok(stack)
    }
}
