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

impl<N: Network> Stack<N> {
    /// Synthesizes the proving key and verifying key for the given function name.
    #[inline]
    pub fn synthesize_key<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(
        &self,
        function_name: &Identifier<N>,
        rng: &mut R,
    ) -> Result<()> {
        // If the proving and verifying key already exist, skip the synthesis for this function.
        if self.contains_proving_key(function_name) && self.contains_verifying_key(function_name) {
            return Ok(());
        }

        // Retrieve the program ID.
        let program_id = self.program_id();
        // Retrieve the function input types.
        let input_types = self.get_function(function_name)?.input_types();

        // Initialize a burner private key.
        let burner_private_key = PrivateKey::new(rng)?;
        // Compute the burner address.
        let burner_address = Address::try_from(&burner_private_key)?;
        // Sample the inputs.
        let inputs = input_types
            .iter()
            .map(|input_type| match input_type {
                ValueType::ExternalRecord(locator) => {
                    // Retrieve the external stack.
                    let stack = self.get_external_stack(locator.program_id())?;
                    // Sample the input.
                    stack.sample_value(&burner_address, &ValueType::Record(*locator.resource()), rng)
                }
                _ => self.sample_value(&burner_address, input_type, rng),
            })
            .collect::<Result<Vec<_>>>()?;
        // Sample 'is_root'.
        let is_root = true;

        // The `root_tvk` is `None` when deploying an individual circuit.
        let root_tvk = None;

        // The caller is `None` when deploying an individual circuit.
        let caller = None;

        // Compute the request, with a burner private key.
        let request = Request::sign(
            &burner_private_key,
            *program_id,
            *function_name,
            inputs.into_iter(),
            &input_types,
            root_tvk,
            is_root,
            rng,
        )?;
        // Initialize the authorization.
        let authorization = Authorization::new(request.clone());
        // Initialize the call stack.
        let call_stack = CallStack::Synthesize(vec![request], burner_private_key, authorization);
        // Synthesize the circuit.
        let _response = self.execute_function::<A, R>(call_stack, caller, root_tvk, rng)?;

        // Ensure the proving key exists.
        ensure!(self.contains_proving_key(function_name), "Function '{function_name}' is missing a proving key.");
        // Ensure the verifying key exists.
        ensure!(self.contains_verifying_key(function_name), "Function '{function_name}' is missing a verifying key.");
        Ok(())
    }

    /// Synthesizes and stores the `(proving_key, verifying_key)` for the given function name and assignment.
    #[inline]
    pub fn synthesize_from_assignment(
        &self,
        function_name: &Identifier<N>,
        assignment: &circuit::Assignment<N::Field>,
    ) -> Result<()> {
        // If the proving and verifying key already exist, skip the synthesis for this function.
        if self.contains_proving_key(function_name) && self.contains_verifying_key(function_name) {
            return Ok(());
        }

        // Synthesize the proving and verifying key.
        let (proving_key, verifying_key) = self.universal_srs.to_circuit_key(&function_name.to_string(), assignment)?;
        // Insert the proving key.
        self.insert_proving_key(function_name, proving_key)?;
        // Insert the verifying key.
        self.insert_verifying_key(function_name, verifying_key)
    }
}
