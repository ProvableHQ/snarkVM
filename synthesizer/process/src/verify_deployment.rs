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
    /// Verifies the given deployment is ordered.
    #[inline]
    pub fn verify_deployment<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(
        &self,
        deployment: &Deployment<N>,
        rng: &mut R,
    ) -> Result<()> {
        let timer = timer!("Process::verify_deployment");

        // Retrieve the program ID.
        let program_id = deployment.program().id();
        // Ensure the program does not already exist in the process.
        ensure!(!self.contains_program(program_id), "Program '{program_id}' already exists");

        // Ensure the program is well-formed, by computing the stack.
        let stack = Stack::new(self, deployment.program())?;
        lap!(timer, "Compute the stack");

        // Ensure the verifying keys are well-formed and the certificates are valid.
        let verification = stack.verify_deployment::<A, R>(deployment, rng);
        lap!(timer, "Verify the deployment");

        finish!(timer);
        verification
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type CurrentAleo = circuit::network::AleoV0;

    /// Use `cargo test profiler --features timer` to run this test.
    #[ignore]
    #[test]
    fn test_profiler() -> Result<()> {
        let rng = &mut TestRng::default();

        // Initialize the process.
        let process = Process::load()?;

        // Fetch the large program to deploy.
        let large_program = Program::from_str(include_str!("./resources/large_functions.aleo"))?;

        // Create a deployment for the program.
        let deployment = process.deploy::<CurrentAleo, _>(&large_program, rng)?;

        // Verify the deployment.
        assert!(process.verify_deployment::<CurrentAleo, _>(&deployment, rng).is_ok());

        bail!("\n\nRemember to #[ignore] this test!\n\n")
    }
}
