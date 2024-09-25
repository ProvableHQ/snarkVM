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

pub mod file_parse_test;
pub use file_parse_test::*;

pub mod line_parse_test;
pub use line_parse_test::*;

pub mod program_test;
pub use program_test::*;

use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

/// A general interface for a test, with an expected result.
/// The output of the test can be checked against the expected result.
/// The expected result can be saved.
pub trait ExpectedTest: Sized {
    type Output;
    /// Loads the test and expectation from the given path.
    fn load<P: AsRef<Path>>(test_path: P, expectation_dir: P) -> Self;
    /// Checks the expectation against the given output.
    /// Prints an error message if the test case fails.
    fn check(&self, output: &Self::Output) -> Result<()>;
    /// Saves the test output.
    fn save(&self, output: &Self::Output) -> Result<()>;
}

/// Recursively reads all files in the `dir` directory and loads them as tests.
/// Filters the test file names by the `TEST_FILTER` environment variable.
/// Note that `dir` must be a relative path from `[...]/snarkVM/synthesizer/tests`.
pub fn load_tests<P: AsRef<Path>, T: ExpectedTest>(test_dir: P, expectation_dir: P) -> Vec<T> {
    let test_dir =
        std::env::current_dir().expect("Failed to retrieve the current directory.").join("tests").join(test_dir);
    // Read the `TEST_FILTER` environment variable.
    let test_filter = std::env::var("TEST_FILTER").ok();
    // Recursively read all files in the `root` directory, filtering out directories and files without sufficient permissions.
    let paths =
        WalkDir::new(test_dir).into_iter().filter_map(|e| e.ok()).map(|e| e.into_path()).filter(|path| path.is_file());
    // Filter the test file names by the `TEST_FILTER` environment variable.
    let filtered_paths = match test_filter {
        Some(ref filter) => paths
            .filter(|path| {
                path.file_name()
                    .expect("Failed to get filename.")
                    .to_str()
                    .expect("Filename is not valid Unicode.")
                    .contains(filter)
            })
            .collect::<Vec<_>>(),
        None => paths.collect::<Vec<_>>(),
    };
    // Initialize the test files.
    filtered_paths
        .into_iter()
        .map(|test_path| T::load(test_path, expectation_dir.as_ref().to_path_buf()))
        .collect::<Vec<_>>()
}
