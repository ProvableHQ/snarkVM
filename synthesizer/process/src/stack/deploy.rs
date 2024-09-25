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

use rand::{rngs::StdRng, SeedableRng};

impl<N: Network> Stack<N> {
    /// Deploys the given program ID, if it does not exist.
    #[inline]
    pub fn deploy<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(&self, rng: &mut R) -> Result<Deployment<N>> {
        let timer = timer!("Stack::deploy");

        // Ensure the program contains functions.
        ensure!(!self.program.functions().is_empty(), "Program '{}' has no functions", self.program.id());

        // Initialize a vector for the verifying keys and certificates.
        let mut verifying_keys = Vec::with_capacity(self.program.functions().len());

        for function_name in self.program.functions().keys() {
            // Synthesize the proving and verifying key.
            self.synthesize_key::<A, R>(function_name, rng)?;
            lap!(timer, "Synthesize key for {function_name}");

            // Retrieve the proving key.
            let proving_key = self.get_proving_key(function_name)?;
            // Retrieve the verifying key.
            let verifying_key = self.get_verifying_key(function_name)?;
            lap!(timer, "Retrieve the keys for {function_name}");

            // Certify the circuit.
            let certificate = Certificate::certify(&function_name.to_string(), &proving_key, &verifying_key)?;
            lap!(timer, "Certify the circuit");

            // Add the verifying key and certificate to the bundle.
            verifying_keys.push((*function_name, (verifying_key, certificate)));
        }

        finish!(timer);

        // Return the deployment.
        Deployment::new(N::EDITION, self.program.clone(), verifying_keys)
    }

    /// Checks each function in the program on the given verifying key and certificate.
    #[inline]
    pub fn verify_deployment<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(
        &self,
        deployment: &Deployment<N>,
        rng: &mut R,
    ) -> Result<()> {
        let timer = timer!("Stack::verify_deployment");

        // Sanity Checks //

        // Ensure the deployment is ordered.
        deployment.check_is_ordered()?;
        // Ensure the program in the stack and deployment matches.
        ensure!(&self.program == deployment.program(), "The stack program does not match the deployment program");

        // Check Verifying Keys //

        let program_id = self.program.id();

        // Check that the number of combined variables does not exceed the deployment limit.
        ensure!(deployment.num_combined_variables()? <= N::MAX_DEPLOYMENT_VARIABLES);
        // Check that the number of combined constraints does not exceed the deployment limit.
        ensure!(deployment.num_combined_constraints()? <= N::MAX_DEPLOYMENT_CONSTRAINTS);

        // Construct the call stacks and assignments used to verify the certificates.
        let mut call_stacks = Vec::with_capacity(deployment.verifying_keys().len());

        // The `root_tvk` is `None` when verifying the deployment of an individual circuit.
        let root_tvk = None;

        // The `caller` is `None` when verifying the deployment of an individual circuit.
        let caller = None;

        // Check that the number of functions matches the number of verifying keys.
        ensure!(
            deployment.program().functions().len() == deployment.verifying_keys().len(),
            "The number of functions in the program does not match the number of verifying keys"
        );

        // Create a seeded rng to use for input value and sub-stack generation.
        // This is needed to ensure that the verification results of deployments are consistent across all parties,
        // because currently there is a possible flakiness due to overflows in Field to Scalar casting.
        let seed = u64::from_bytes_le(&deployment.to_deployment_id()?.to_bytes_le()?[0..8])?;
        let mut seeded_rng = rand_chacha::ChaChaRng::seed_from_u64(seed);

        // Iterate through the program functions and construct the callstacks and corresponding assignments.
        for (function, (_, (verifying_key, _))) in
            deployment.program().functions().values().zip_eq(deployment.verifying_keys())
        {
            // Initialize a burner private key.
            let burner_private_key = PrivateKey::new(rng)?;
            // Compute the burner address.
            let burner_address = Address::try_from(&burner_private_key)?;
            // Retrieve the input types.
            let input_types = function.input_types();
            // Sample the inputs.
            let inputs = input_types
                .iter()
                .map(|input_type| match input_type {
                    ValueType::ExternalRecord(locator) => {
                        // Retrieve the external stack.
                        let stack = self.get_external_stack(locator.program_id())?;
                        // Sample the input.
                        stack.sample_value(&burner_address, &ValueType::Record(*locator.resource()), &mut seeded_rng)
                    }
                    _ => self.sample_value(&burner_address, input_type, &mut seeded_rng),
                })
                .collect::<Result<Vec<_>>>()?;
            lap!(timer, "Sample the inputs");
            // Sample 'is_root'.
            let is_root = true;

            // Compute the request, with a burner private key.
            let request = Request::sign(
                &burner_private_key,
                *program_id,
                *function.name(),
                inputs.into_iter(),
                &input_types,
                root_tvk,
                is_root,
                rng,
            )?;
            lap!(timer, "Compute the request for {}", function.name());
            // Initialize the assignments.
            let assignments = Assignments::<N>::default();
            // Initialize the constraint limit. Account for the constraint added after synthesis that makes the Varuna zerocheck hiding.
            let Some(constraint_limit) = verifying_key.circuit_info.num_constraints.checked_sub(1) else {
                // Since a deployment must always pay non-zero fee, it must always have at least one constraint.
                bail!("The constraint limit of 0 for function '{}' is invalid", function.name());
            };
            // Retrieve the variable limit.
            let variable_limit = verifying_key.num_variables();
            // Initialize the call stack.
            let call_stack = CallStack::CheckDeployment(
                vec![request],
                burner_private_key,
                assignments.clone(),
                Some(constraint_limit as u64),
                Some(variable_limit),
            );
            // Append the function name, callstack, and assignments.
            call_stacks.push((function.name(), call_stack, assignments));
        }

        // Verify the certificates.
        let rngs = (0..call_stacks.len()).map(|_| StdRng::from_seed(seeded_rng.gen())).collect::<Vec<_>>();
        cfg_into_iter!(call_stacks).zip_eq(deployment.verifying_keys()).zip_eq(rngs).try_for_each(
            |(((function_name, call_stack, assignments), (_, (verifying_key, certificate))), mut rng)| {
                // Synthesize the circuit.
                if let Err(err) = self.execute_function::<A, _>(call_stack, caller, root_tvk, &mut rng) {
                    bail!("Failed to synthesize the circuit for '{function_name}': {err}")
                }
                // Check the certificate.
                match assignments.read().last() {
                    None => bail!("The assignment for function '{function_name}' is missing in '{program_id}'"),
                    Some((assignment, _metrics)) => {
                        // Ensure the certificate is valid.
                        if !certificate.verify(&function_name.to_string(), assignment, verifying_key) {
                            bail!("The certificate for function '{function_name}' is invalid in '{program_id}'")
                        }
                    }
                };
                Ok(())
            },
        )?;

        finish!(timer);

        Ok(())
    }
}
