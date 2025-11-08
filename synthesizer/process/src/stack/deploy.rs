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

use rand::{SeedableRng, rngs::StdRng};

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
            let (verifying_key, certificate) = if cfg!(feature = "dev_skip_checks") {
                // Sample a dummy verifying key.
                let verifying_key = VerifyingKey::from_str(
                    "verifier1qygqqqqqqqqqqqyvxgqqqqqqqqq87vsqqqqqqqqqhe7sqqqqqqqqqma4qqqqqqqqqq65yqqqqqqqqqqvqqqqqqqqqqqgtlaj49fmrk2d8slmselaj9tpucgxv6awu6yu4pfcn5xa0yy0tpxpc8wemasjvvxr9248vt3509vpk3u60ejyfd9xtvjmudpp7ljq2csk4yqz70ug3x8xp3xn3ul0yrrw0mvd2g8ju7rts50u3smue03gp99j88f0ky8h6fjlpvh58rmxv53mldmgrxa3fq6spsh8gt5whvsyu2rk4a2wmeyrgvvdf29pwp02srktxnvht3k6ff094usjtllggva2ym75xc4lzuqu9xx8ylfkm3qc7lf7ktk9uu9du5raukh828dzgq26hrarq5ajjl7pz7zk924kekjrp92r6jh9dpp05mxtuffwlmvew84dvnqrkre7lw29mkdzgdxwe7q8z0vnkv2vwwdraekw2va3plu7rkxhtnkuxvce0qkgxcxn5mtg9q2c3vxdf2r7jjse2g68dgvyh85q4mzfnvn07lletrpty3vypus00gfu9m47rzay4mh5w9f03z9zgzgzhkv0mupdqsk8naljqm9tc2qqzhf6yp3mnv2ey89xk7sw9pslzzlkndfd2upzmew4e4vnrkr556kexs9qrykkuhsr260mnrgh7uv0sp2meky0keeukaxgjdsnmy77kl48g3swcvqdjm50ejzr7x04vy7hn7anhd0xeetclxunnl7pd6e52qxdlr3nmutz4zr8f2xqa57a2zkl59a28w842cj4783zpy9hxw03k6vz4a3uu7sm072uqknpxjk8fyq4vxtqd08kd93c2mt40lj9ag35nm4rwcfjayejk57m9qqu83qnkrj3sz90pw808srmf705n2yu6gvqazpvu2mwm8x6mgtlsntxfhr0qas43rqxnccft36z4ygty86390t7vrt08derz8368z8ekn3yywxgp4uq24gm6e58tpp0lcvtpsm3nkwpnmzztx4qvkaf6vk38wg787h8mfpqqqqqqqqqqt49m8x",
                )?;
                // Sample a dummy certificate.
                let certificate = Certificate::from_str(
                    "certificate1qyqsqqqqqqqqqqxvwszp09v860w62s2l4g6eqf0kzppyax5we36957ywqm2dplzwvvlqg0kwlnmhzfatnax7uaqt7yqqqw0sc4u",
                )?;

                (verifying_key, certificate)
            } else {
                self.synthesize_key::<A, R>(function_name, rng)?;
                lap!(timer, "Synthesize key for {function_name}");

                // Retrieve the proving key.
                let proving_key = self.get_proving_key(function_name)?;
                // Retrieve the verifying key.
                let verifying_key = self.get_verifying_key(function_name)?;
                lap!(timer, "Retrieve the keys for {function_name}");
                // Certify the verifying key.
                let certificate = Certificate::certify(&function_name.to_string(), &proving_key, &verifying_key)?;
                lap!(timer, "Certify the verifying key");

                (verifying_key, certificate)
            };
            // Add the verifying key and certificate to the bundle.
            verifying_keys.push((*function_name, (verifying_key, certificate)));
        }

        finish!(timer);

        // Return the deployment.
        Deployment::new(*self.program_edition, self.program.clone(), verifying_keys, None, None)
    }

    /// Checks each function in the program on the given verifying key and certificate.
    #[inline]
    pub fn verify_deployment<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(
        &self,
        _consensus_version: ConsensusVersion,
        deployment: &Deployment<N>,
        rng: &mut R,
    ) -> Result<()> {
        let timer = timer!("Stack::verify_deployment");

        // NOTE: As developer, you will likely still want to confirm that your
        // deployment is within R1CS constraint and variable limits using
        // targeted and parallelized synthesis.
        if cfg!(feature = "dev_skip_checks") {
            return Ok(());
        }

        // Sanity Checks //

        // Ensure the deployment is ordered.
        deployment.check_is_ordered()?;

        // Ensure the program in the stack and deployment matches.
        ensure!(&self.program == deployment.program(), "The stack program does not match the deployment program");
        // If the deployment contains a checksum, ensure it matches the one computed by the stack.
        if let Some(program_checksum) = deployment.program_checksum() {
            ensure!(
                program_checksum == self.program_checksum,
                "The deployment checksum does not match the stack checksum"
            );
        }

        // Check Verifying Keys //

        // Get the program ID.
        let program_id = self.program.id();

        // Check that the number of combined variables does not exceed the deployment limit.
        ensure!(deployment.num_combined_variables()? <= N::MAX_DEPLOYMENT_VARIABLES);
        // Check that the number of combined constraints does not exceed the deployment limit.
        ensure!(deployment.num_combined_constraints()? <= N::MAX_DEPLOYMENT_CONSTRAINTS);

        // Construct the call stacks and assignments used to verify the certificates.
        let mut call_stacks = Vec::with_capacity(deployment.verifying_keys().len());

        // Sample a dummy `root_tvk` for circuit synthesis.
        let root_tvk = None;
        // Sample a dummy `caller` for circuit synthesis.
        let caller = None;

        // Check that the number of functions matches the number of verifying keys.
        ensure!(
            deployment.program().functions().len() == deployment.verifying_keys().len(),
            "The number of functions in the program does not match the number of verifying keys"
        );

        #[cfg(not(any(test, feature = "test")))]
        // Skip the certificate verification if the consensus version is before ConsensusVersion::V8.
        // Circuit synthesis was changed in a backwards incompatible way in ConsensusVersion::V8.
        if (ConsensusVersion::V1..=ConsensusVersion::V7).contains(&_consensus_version) {
            finish!(timer);
            return Ok(());
        }

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
            // Retrieve the program checksum, if the program has a constructor.
            let program_checksum = match self.program().contains_constructor() {
                true => Some(self.program_checksum_as_field()?),
                false => None,
            };
            // Sample the inputs.
            let inputs = input_types
                .iter()
                .map(|input_type| match input_type {
                    ValueType::ExternalRecord(locator) => {
                        // Retrieve the external stack.
                        let stack = self.get_external_stack(locator.program_id())?;
                        // Sample the input.
                        stack.sample_value(
                            &burner_address,
                            &ValueType::Record(*locator.resource()).into(),
                            &mut seeded_rng,
                        )
                    }
                    _ => self.sample_value(&burner_address, &input_type.into(), &mut seeded_rng),
                })
                .collect::<Result<Vec<_>>>()?;
            lap!(timer, "Sample the inputs");
            // Sample a dummy 'is_root'.
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
                program_checksum,
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
        let rngs = (0..call_stacks.len()).map(|_| StdRng::from_seed(seeded_rng.r#gen())).collect::<Vec<_>>();
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
