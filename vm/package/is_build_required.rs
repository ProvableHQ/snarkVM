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
    /// Returns `true` if the package is stale or has not been built.
    pub fn is_build_required<A: crate::circuit::Aleo<Network = N, BaseField = N::Field>>(&self) -> bool {
        // Prepare the build directory.
        let build_directory = self.build_directory();
        // If the build directory does not exist, then a build is required.
        if !build_directory.exists() {
            return true;
        }

        // If the main AVM file does not exists, then a build is required.
        if !AVMFile::<N>::main_exists_at(&build_directory) {
            return true;
        }

        // Open the main AVM file.
        let avm_file = match AVMFile::open(&build_directory, &self.program_id, true) {
            // Retrieve the main AVM file.
            Ok(file) => file,
            // If the main AVM file fails to open, then a build is required.
            Err(_) => return true,
        };

        // Check if the main program matches.
        let program = self.program();
        if avm_file.program() != program {
            return true;
        }

        // Next, check if the prover and verifier exist for each function.
        for function_name in program.functions().keys() {
            // Check if the prover file exists.
            if !ProverFile::exists_at(&build_directory, function_name) {
                // If not, we need to build the circuit.
                return true;
            }
            // Check if the verifier file exists.
            if !VerifierFile::exists_at(&build_directory, function_name) {
                // If not, we need to build the circuit.
                return true;
            }
        }

        // Skip building the package, as it has not changed.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console::network::MainnetV0;
    use std::{fs::File, io::Write};

    type CurrentNetwork = MainnetV0;
    type Aleo = crate::circuit::AleoV0;

    fn temp_dir() -> PathBuf {
        tempfile::tempdir().expect("Failed to open temporary directory").into_path()
    }

    fn initialize_unbuilt_package(valid: bool) -> Result<Package<MainnetV0>> {
        // Initialize a temporary directory.
        let directory = temp_dir();

        let program_id = ProgramID::<CurrentNetwork>::from_str("token.aleo").unwrap();

        let program_string = match valid {
            true => program_with_id("token.aleo"),
            false => program_with_id("invalid_id.aleo"),
        };

        // Write the program string to a file in the temporary directory.
        let path = directory.join("main.aleo");
        let mut file = File::create(path).unwrap();
        file.write_all(program_string.as_bytes()).unwrap();

        // Create the manifest file.
        let _manifest_file = Manifest::create(&directory, &program_id).unwrap();

        // Create the build directory.
        let build_directory = directory.join("build");
        std::fs::create_dir_all(build_directory).unwrap();

        // Open the package at the temporary directory.
        Package::<MainnetV0>::open(&directory)
    }

    fn program_with_id(id: &str) -> String {
        format!(
            r"program {id};

record token:
    owner as address.private;
    token_amount as u64.private;

function compute:
    input r0 as token.record;
    add.w r0.token_amount r0.token_amount into r1;
    output r1 as u64.private;"
        )
    }

    #[test]
    fn test_for_new_package() {
        let package = initialize_unbuilt_package(true).unwrap();
        assert!(package.is_build_required::<Aleo>());
    }

    #[test]
    fn test_when_avm_file_does_not_exist() {
        let package = initialize_unbuilt_package(true).unwrap();
        assert!(package.build_directory().exists());
        assert!(!AVMFile::<CurrentNetwork>::main_exists_at(&package.build_directory()));
        assert!(package.is_build_required::<Aleo>());
    }

    #[test]
    fn test_fails_when_avm_and_package_program_ids_do_not_match() {
        let package = initialize_unbuilt_package(false);
        assert!(package.is_err());
    }

    #[test]
    fn test_when_prover_and_verifier_files_do_not_exist() {
        let package = initialize_unbuilt_package(true).unwrap();
        assert!(package.build_directory().exists());

        assert!(!AVMFile::<CurrentNetwork>::main_exists_at(&package.build_directory()));
        assert!(AVMFile::<CurrentNetwork>::create(&package.build_directory(), package.program().clone(), true).is_ok());
        assert!(AVMFile::<CurrentNetwork>::main_exists_at(&package.build_directory()));

        let avm_file = AVMFile::open(&package.build_directory(), &package.program_id, true).unwrap();
        assert_eq!(avm_file.program().id(), &package.program_id);
        assert_eq!(avm_file.program(), package.program());

        assert!(
            package
                .program()
                .functions()
                .keys()
                .filter(|k| {
                    ProverFile::exists_at(&package.build_directory(), k)
                        || VerifierFile::exists_at(&package.build_directory(), k)
                })
                .peekable()
                .peek()
                .is_none()
        );

        assert!(package.is_build_required::<Aleo>());
    }

    #[test]
    fn test_prebuilt_package_does_not_rebuild() {
        let package = initialize_unbuilt_package(true).unwrap();
        assert!(package.is_build_required::<Aleo>());

        package.build::<Aleo>(None).unwrap();
        assert!(!package.is_build_required::<Aleo>());
    }
}
