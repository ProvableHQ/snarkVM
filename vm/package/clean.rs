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
    /// Removes the build directory for the package.
    pub fn clean(directory: &Path) -> Result<()> {
        // Ensure the directory path exists.
        ensure!(directory.exists(), "The program directory does not exist: {}", directory.display());
        // Ensure the manifest file exists.
        ensure!(
            Manifest::<N>::exists_at(directory),
            "Missing '{}' at '{}'",
            Manifest::<N>::file_name(),
            directory.display()
        );
        // Ensure the main program file exists.
        ensure!(
            AleoFile::<N>::main_exists_at(directory),
            "Missing '{}' at '{}'",
            AleoFile::<N>::main_file_name(),
            directory.display()
        );

        // Prepare the build directory.
        let build_directory = directory.join("build");
        // Remove the build directory if it exists.
        if build_directory.exists() {
            std::fs::remove_dir_all(&build_directory)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type CurrentNetwork = snarkvm_console::network::MainnetV0;
    type CurrentAleo = snarkvm_circuit::network::AleoV0;

    #[test]
    fn test_clean() {
        // Samples a new package at a temporary directory.
        let (directory, package) = crate::package::test_helpers::sample_token_package();

        // Ensure the build directory does *not* exist.
        assert!(!package.build_directory().exists());
        // Clean the package.
        Package::<CurrentNetwork>::clean(&directory).unwrap();
        // Ensure the build directory still does *not* exist.
        assert!(!package.build_directory().exists());

        // Build the package.
        package.build::<CurrentAleo>(None).unwrap();

        // Ensure the build directory exists.
        assert!(package.build_directory().exists());
        // Clean the package.
        Package::<CurrentNetwork>::clean(&directory).unwrap();
        // Ensure the build directory does *not* exist.
        assert!(!package.build_directory().exists());

        // Proactively remove the temporary directory (to conserve space).
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn test_clean_with_import() {
        // Samples a new package at a temporary directory.
        let (directory, package) = crate::package::test_helpers::sample_wallet_package();

        // Ensure the build directory does *not* exist.
        assert!(!package.build_directory().exists());
        // Clean the package.
        Package::<CurrentNetwork>::clean(&directory).unwrap();
        // Ensure the build directory still does *not* exist.
        assert!(!package.build_directory().exists());

        // Build the package.
        package.build::<CurrentAleo>(None).unwrap();

        // Ensure the build directory exists.
        assert!(package.build_directory().exists());
        // Clean the package.
        Package::<CurrentNetwork>::clean(&directory).unwrap();
        // Ensure the build directory does *not* exist.
        assert!(!package.build_directory().exists());

        // Proactively remove the temporary directory (to conserve space).
        std::fs::remove_dir_all(directory).unwrap();
    }
}
