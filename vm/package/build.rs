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

use snarkvm_utilities::DeserializeExt;

pub struct BuildRequest<N: Network> {
    program: Program<N>,
    imports: Vec<Program<N>>,
    function_name: Identifier<N>,
}

impl<N: Network> BuildRequest<N> {
    /// Initializes a new build request.
    pub const fn new(program: Program<N>, imports: Vec<Program<N>>, function_name: Identifier<N>) -> Self {
        Self { program, imports, function_name }
    }

    /// Sends the request to the given endpoint.
    pub fn send(&self, endpoint: &str) -> Result<BuildResponse<N>> {
        Ok(ureq::get(endpoint).send_json(self)?.into_json()?)
    }

    /// Returns the program.
    pub const fn program(&self) -> &Program<N> {
        &self.program
    }

    /// Returns the imports.
    pub const fn imports(&self) -> &Vec<Program<N>> {
        &self.imports
    }

    /// Returns the function name.
    pub const fn function_name(&self) -> &Identifier<N> {
        &self.function_name
    }
}

impl<N: Network> Serialize for BuildRequest<N> {
    /// Serializes the build request into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut request = serializer.serialize_struct("BuildRequest", 3)?;
        request.serialize_field("program", &self.program)?;
        request.serialize_field("imports", &self.imports)?;
        request.serialize_field("function_name", &self.function_name)?;
        request.end()
    }
}

impl<'de, N: Network> Deserialize<'de> for BuildRequest<N> {
    /// Deserializes the build request from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Parse the request from a string into a value.
        let mut request = serde_json::Value::deserialize(deserializer)?;
        // Recover the leaf.
        Ok(Self::new(
            // Retrieve the program.
            DeserializeExt::take_from_value::<D>(&mut request, "program")?,
            // Retrieve the imports.
            DeserializeExt::take_from_value::<D>(&mut request, "imports")?,
            // Retrieve the function name.
            DeserializeExt::take_from_value::<D>(&mut request, "function_name")?,
        ))
    }
}

pub struct BuildResponse<N: Network> {
    program_id: ProgramID<N>,
    function_name: Identifier<N>,
    proving_key: ProvingKey<N>,
    verifying_key: VerifyingKey<N>,
}

impl<N: Network> BuildResponse<N> {
    /// Initializes a new build response.
    pub const fn new(
        program_id: ProgramID<N>,
        function_name: Identifier<N>,
        proving_key: ProvingKey<N>,
        verifying_key: VerifyingKey<N>,
    ) -> Self {
        Self { program_id, function_name, proving_key, verifying_key }
    }

    /// Returns the program ID.
    pub const fn program_id(&self) -> &ProgramID<N> {
        &self.program_id
    }

    /// Returns the function name.
    pub const fn function_name(&self) -> &Identifier<N> {
        &self.function_name
    }

    /// Returns the proving key.
    pub const fn proving_key(&self) -> &ProvingKey<N> {
        &self.proving_key
    }

    /// Returns the verifying key.
    pub const fn verifying_key(&self) -> &VerifyingKey<N> {
        &self.verifying_key
    }
}

impl<N: Network> Serialize for BuildResponse<N> {
    /// Serializes the build response into string or bytes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut response = serializer.serialize_struct("BuildResponse", 4)?;
        response.serialize_field("program_id", &self.program_id)?;
        response.serialize_field("function_name", &self.function_name)?;
        response.serialize_field("proving_key", &self.proving_key)?;
        response.serialize_field("verifying_key", &self.verifying_key)?;
        response.end()
    }
}

impl<'de, N: Network> Deserialize<'de> for BuildResponse<N> {
    /// Deserializes the build response from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Parse the response from a string into a value.
        let mut response = serde_json::Value::deserialize(deserializer)?;
        // Recover the leaf.
        Ok(Self::new(
            // Retrieve the program ID.
            DeserializeExt::take_from_value::<D>(&mut response, "program_id")?,
            // Retrieve the function name.
            DeserializeExt::take_from_value::<D>(&mut response, "function_name")?,
            // Retrieve the proving key.
            DeserializeExt::take_from_value::<D>(&mut response, "proving_key")?,
            // Retrieve the verifying key.
            DeserializeExt::take_from_value::<D>(&mut response, "verifying_key")?,
        ))
    }
}

impl<N: Network> Package<N> {
    /// Builds the package.
    pub fn build<A: crate::circuit::Aleo<Network = N, BaseField = N::Field>>(
        &self,
        endpoint: Option<String>,
    ) -> Result<()> {
        // Skip the 'build' if the program is already built.
        if !self.is_build_required::<A>() {
            return Ok(());
        }

        // Retrieve the main program.
        let program = self.program();
        // Retrieve the program ID.
        let program_id = program.id();

        #[cfg(feature = "aleo-cli")]
        println!("⏳ Compiling '{}'...\n", program_id.to_string().bold());

        // Prepare the build directory.
        let build_directory = self.build_directory();
        // Create the build directory if it does not exist.
        if !build_directory.exists() {
            std::fs::create_dir_all(&build_directory)?;
        }

        // Construct the process.
        let process = self.get_process()?;

        // Retrieve the imported programs.
        let imported_programs = program
            .imports()
            .keys()
            .map(|program_id| process.get_program(program_id).cloned())
            .collect::<Result<Vec<_>>>()?;

        // Synthesize each proving and verifying key.
        for function_name in program.functions().keys() {
            match endpoint {
                Some(ref endpoint) => {
                    // Prepare the request.
                    let request = BuildRequest::new(program.clone(), imported_programs.clone(), *function_name);
                    // Load the proving and verifying key.
                    let response = request.send(endpoint)?;
                    // Ensure the program ID matches.
                    ensure!(
                        response.program_id() == program_id,
                        "Program ID mismatch: {} != {program_id}",
                        response.program_id()
                    );
                    // Ensure the function name matches.
                    ensure!(
                        response.function_name() == function_name,
                        "Function name mismatch: {} != {function_name}",
                        response.function_name()
                    );
                    // Insert the proving key.
                    process.insert_proving_key(response.program_id(), function_name, response.proving_key().clone())?;
                    // Insert the verifying key.
                    process.insert_verifying_key(
                        response.program_id(),
                        function_name,
                        response.verifying_key().clone(),
                    )?;
                }
                None => process.synthesize_key::<A, _>(program_id, function_name, &mut rand::thread_rng())?,
            }
        }

        // Load each function circuit.
        for function_name in program.functions().keys() {
            // Retrieve the program.
            let program = process.get_program(program_id)?;
            // Retrieve the function from the program.
            let function = program.get_function(function_name)?;
            // Save all the prover and verifier files for any function calls that are made.
            for instruction in function.instructions() {
                if let Instruction::Call(call) = instruction {
                    // Retrieve the program and resource.
                    let (program, resource) = match call.operator() {
                        CallOperator::Locator(locator) => {
                            (process.get_program(locator.program_id())?, locator.resource())
                        }
                        CallOperator::Resource(resource) => (program, resource),
                    };
                    // If this is a function call, save its corresponding prover and verifier files.
                    if program.contains_function(resource) {
                        // Set the function name to the resource, in this scope.
                        let function_name = resource;
                        // Retrieve the proving key.
                        let proving_key = process.get_proving_key(program.id(), resource)?;
                        // Retrieve the verifying key.
                        let verifying_key = process.get_verifying_key(program.id(), resource)?;

                        // Prepare the build directory for the imported program.
                        let import_build_directory =
                            self.build_directory().join(format!("{}-{}", program.id().name(), program.id().network()));
                        // Create the build directory if it does not exist.
                        if !import_build_directory.exists() {
                            std::fs::create_dir_all(&import_build_directory)?;
                        }

                        // Create the prover.
                        let _prover = ProverFile::create(&import_build_directory, function_name, proving_key)?;
                        // Create the verifier.
                        let _verifier = VerifierFile::create(&import_build_directory, function_name, verifying_key)?;
                    }
                }
            }

            // Retrieve the proving key.
            let proving_key = process.get_proving_key(program_id, function_name)?;
            // Retrieve the verifying key.
            let verifying_key = process.get_verifying_key(program_id, function_name)?;

            // Create the prover.
            let _prover = ProverFile::create(&build_directory, function_name, proving_key)?;
            // Create the verifier.
            let _verifier = VerifierFile::create(&build_directory, function_name, verifying_key)?;
        }

        // Lastly, write the AVM file.
        let _avm_file = AVMFile::create(&build_directory, program.clone(), true)?;

        // Ensure the build directory exists.
        if !self.build_directory().exists() {
            bail!("Build directory does not exist: {}", self.build_directory().display());
        }

        #[cfg(feature = "aleo-cli")]
        println!();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    type CurrentAleo = snarkvm_circuit::network::AleoV0;

    #[test]
    fn test_build() {
        // Samples a new package at a temporary directory.
        let (directory, package) = crate::package::test_helpers::sample_token_package();

        // Ensure the build directory does *not* exist.
        assert!(!package.build_directory().exists());
        // Build the package.
        package.build::<CurrentAleo>(None).unwrap();
        // Ensure the build directory exists.
        assert!(package.build_directory().exists());

        // Proactively remove the temporary directory (to conserve space).
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn test_build_with_import() {
        // Samples a new package at a temporary directory.
        let (directory, package) = crate::package::test_helpers::sample_wallet_package();

        // Ensure the build directory does *not* exist.
        assert!(!package.build_directory().exists());
        // Build the package.
        package.build::<CurrentAleo>(None).unwrap();
        // Ensure the build directory exists.
        assert!(package.build_directory().exists());

        // Proactively remove the temporary directory (to conserve space).
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    #[ignore]
    fn test_build_with_import_credits() {
        // Samples a new package at a temporary directory.
        let (directory, package) = crate::package::test_helpers::sample_transfer_package();

        // Ensure the build directory does *not* exist.
        assert!(!package.build_directory().exists());
        // Build the package.
        package.build::<CurrentAleo>(None).unwrap();
        // Ensure the build directory exists.
        assert!(package.build_directory().exists());

        // Proactively remove the temporary directory (to conserve space).
        std::fs::remove_dir_all(directory).unwrap();
    }
}
