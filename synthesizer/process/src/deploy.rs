// Copyright (C) 2019-2023 Aleo Systems Inc.
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

    /// Adds the newly-deployed program.
    /// This method assumes the given deployment **is valid**.
    #[inline]
    pub fn load_deployment(&mut self, deployment: &Deployment<N>) -> Result<()> {
        let timer = timer!("Process::load_deployment");

        // Compute the program stack.
        let stack = Stack::new(self, deployment.program())?;
        lap!(timer, "Compute the stack");

        // Insert the verifying keys.
        for (function_name, (verifying_key, _)) in deployment.verifying_keys() {
            stack.insert_verifying_key(function_name, verifying_key.clone())?;
        }
        lap!(timer, "Insert the verifying keys");

        // Add the stack to the process.
        self.add_stack(stack);

        finish!(timer);

        Ok(())
    }
}
