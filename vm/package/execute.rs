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

impl<N: Network> Package<N> {
    /// Executes a program function with the given inputs.
    #[allow(clippy::type_complexity)]
    pub fn execute<A: crate::circuit::Aleo<Network = N, BaseField = N::Field>, R: Rng + CryptoRng>(
        &self,
        endpoint: String,
        private_key: &PrivateKey<N>,
        function_name: Identifier<N>,
        inputs: &[Value<N>],
        rng: &mut R,
    ) -> Result<(Response<N>, Execution<N>, Vec<CallMetrics<N>>)> {
        // Retrieve the main program.
        let program = self.program();
        // Retrieve the program ID.
        let program_id = program.id();
        // Ensure that the function exists.
        if !program.contains_function(&function_name) {
            bail!("Function '{function_name}' does not exist.")
        }

        // Build the package, if the package requires building.
        // TODO (howardwu): We currently choose only to support local synthesis of keys due to performance.
        // self.build::<A>(Some(endpoint.clone()))?;
        self.build::<A>(None)?;

        // Prepare the locator (even if logging is disabled, to sanity check the locator is well-formed).
        let locator = Locator::<N>::from_str(&format!("{program_id}/{function_name}"))?;

        #[cfg(feature = "aleo-cli")]
        println!("🚀 Executing '{}'...\n", locator.to_string().bold());

        // Construct the process.
        let process = self.get_process()?;
        // Authorize the function call.
        let authorization = process.authorize::<A, R>(private_key, program_id, function_name, inputs.iter(), rng)?;

        // Retrieve the program.
        let program = process.get_program(program_id)?;
        // Retrieve the function from the program.
        let function = program.get_function(&function_name)?;
        // Save all the prover and verifier files for any function calls that are made.
        for instruction in function.instructions() {
            if let Instruction::Call(call) = instruction {
                // Retrieve the program and resource.
                let (program, resource) = match call.operator() {
                    CallOperator::Locator(locator) => (process.get_program(locator.program_id())?, locator.resource()),
                    CallOperator::Resource(resource) => (program, resource),
                };
                // If this is a function call, save its corresponding prover and verifier files.
                if program.contains_function(resource) {
                    // Set the function name to the resource, in this scope.
                    let function_name = resource;
                    // Prepare the build directory for the imported program.
                    let import_build_directory =
                        self.build_directory().join(format!("{}-{}", program.id().name(), program.id().network()));

                    // Create the prover.
                    let prover = ProverFile::open(&import_build_directory, function_name)?;
                    // Adds the proving key to the process.
                    process.insert_proving_key(program.id(), function_name, prover.proving_key().clone())?;

                    // Create the verifier.
                    let verifier = VerifierFile::open(&import_build_directory, function_name)?;
                    // Adds the verifying key to the process.
                    process.insert_verifying_key(program.id(), function_name, verifier.verifying_key().clone())?;
                }
            }
        }

        // Prepare the build directory.
        let build_directory = self.build_directory();
        // Load the prover.
        let prover = ProverFile::open(&build_directory, &function_name)?;
        // Load the verifier.
        let verifier = VerifierFile::open(&build_directory, &function_name)?;

        // Adds the proving key to the process.
        process.insert_proving_key(program_id, &function_name, prover.proving_key().clone())?;
        // Adds the verifying key to the process.
        process.insert_verifying_key(program_id, &function_name, verifier.verifying_key().clone())?;

        // Execute the circuit.
        let (response, mut trace) = process.execute::<A, R>(authorization, rng)?;

        // Retrieve the call metrics.
        let call_metrics = trace.call_metrics().to_vec();

        // Prepare the trace.
        trace.prepare(Query::<_, BlockMemory<_>>::from(endpoint))?;
        // Prove the execution.
        let execution = trace.prove_execution::<A, R>(&locator.to_string(), rng)?;
        // Return the response, execution, and call metrics.
        Ok((response, execution, call_metrics))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_utilities::TestRng;

    type CurrentAleo = snarkvm_circuit::network::AleoV0;

    // TODO: Re-enable this test after `mainnet`.
    #[test]
    #[ignore]
    fn test_execute() {
        // Samples a new package at a temporary directory.
        let (directory, package) = crate::package::test_helpers::sample_token_package();

        // Ensure the build directory does *not* exist.
        assert!(!package.build_directory().exists());
        // Build the package.
        package.build::<CurrentAleo>(None).unwrap();
        // Ensure the build directory exists.
        assert!(package.build_directory().exists());

        // Initialize an RNG.
        let rng = &mut TestRng::default();
        // Sample the function inputs.
        let (private_key, function_name, inputs) =
            crate::package::test_helpers::sample_package_run(package.program_id());
        // Construct the endpoint.
        let endpoint = "https://api.explorer.aleo.org/v1".to_string();
        // Run the program function.
        let (_response, _execution, _metrics) =
            package.execute::<CurrentAleo, _>(endpoint, &private_key, function_name, &inputs, rng).unwrap();

        // Proactively remove the temporary directory (to conserve space).
        std::fs::remove_dir_all(directory).unwrap();
    }

    // TODO: Re-enable this test using a mock API endpoint for the `Query` struct.
    #[test]
    #[ignore]
    fn test_execute_with_import() {
        // Samples a new package at a temporary directory.
        let (directory, package) = crate::package::test_helpers::sample_wallet_package();

        // Ensure the build directory does *not* exist.
        assert!(!package.build_directory().exists());
        // Build the package.
        package.build::<CurrentAleo>(None).unwrap();
        // Ensure the build directory exists.
        assert!(package.build_directory().exists());

        // Initialize an RNG.
        let rng = &mut TestRng::default();
        // Sample the function inputs.
        let (private_key, function_name, inputs) =
            crate::package::test_helpers::sample_package_run(package.program_id());
        // Construct the endpoint.
        let endpoint = "https://api.explorer.aleo.org/v1".to_string();
        // Run the program function.
        let (_response, _execution, _metrics) =
            package.execute::<CurrentAleo, _>(endpoint, &private_key, function_name, &inputs, rng).unwrap();

        // Proactively remove the temporary directory (to conserve space).
        std::fs::remove_dir_all(directory).unwrap();
    }

    /// Use `cargo test profiler --features timer` to run this test.
    #[ignore]
    #[test]
    fn test_profiler() -> Result<()> {
        // Samples a new package at a temporary directory.
        let (directory, package) = crate::package::test_helpers::sample_token_package();

        // Ensure the build directory does *not* exist.
        assert!(!package.build_directory().exists());
        // Build the package.
        package.build::<CurrentAleo>(None).unwrap();
        // Ensure the build directory exists.
        assert!(package.build_directory().exists());

        // Initialize an RNG.
        let rng = &mut TestRng::default();
        // Sample the function inputs.
        let (private_key, function_name, inputs) =
            crate::package::test_helpers::sample_package_run(package.program_id());
        // Construct the endpoint.
        let endpoint = "https://api.explorer.aleo.org/v1".to_string();
        // Run the program function.
        let (_response, _execution, _metrics) =
            package.execute::<CurrentAleo, _>(endpoint, &private_key, function_name, &inputs, rng).unwrap();

        // Proactively remove the temporary directory (to conserve space).
        std::fs::remove_dir_all(directory).unwrap();

        bail!("\n\nRemember to #[ignore] this test!\n\n")
    }
}
