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

use crate::{get_expectation_path, print_difference, ExpectedTest};

use anyhow::{bail, Result};
use itertools::Itertools;
use std::path::{Path, PathBuf};

/// A set of tests for a parser, where each line is the test input.
pub struct LineParseTest {
    test_strings: Vec<String>,
    expectations: Vec<String>,
    expectation_path: PathBuf,
    rewrite: bool,
}

impl LineParseTest {
    /// Returns the set of test strings to be run.
    pub fn test_strings(&self) -> &Vec<String> {
        &self.test_strings
    }
}

impl ExpectedTest for LineParseTest {
    type Output = Vec<String>;

    /// Loads the tests from a given path.
    fn load<P: AsRef<Path>>(test_path: P, expectation_dir: P) -> Self {
        // Read the contents of the test file.
        let test_strings = std::fs::read_to_string(&test_path)
            .expect("Failed to read test file.")
            .lines()
            .map(|l| l.to_string())
            .collect();
        // Check if the expectation file should be rewritten.
        let rewrite = std::env::var("REWRITE_EXPECTATIONS").is_ok();
        // Construct the path the expectation file.
        let expectation_path = get_expectation_path(&test_path, &expectation_dir);
        // If the expectation file should be rewritten, then there is no need to read the expectation file.
        let expectations = match rewrite {
            true => Vec::new(),
            false => serde_yaml::from_str(
                &std::fs::read_to_string(&expectation_path).expect("Failed to read expectation file."),
            )
            .expect("Failed to parse expectation file."),
        };
        Self { test_strings, expectations, expectation_path, rewrite }
    }

    fn check(&self, output: &Self::Output) -> Result<()> {
        // Initialize space to accumulate errors.
        let mut errors = Vec::new();
        // If the expectation file should be rewritten, then there is no need to check the output.
        if !self.rewrite {
            self.test_strings.iter().zip_eq(self.expectations.iter().zip_eq(output.iter())).for_each(
                |(test, (expected, actual))| {
                    if expected != actual {
                        errors.push(print_difference(test, expected, actual));
                    }
                },
            );
        };
        // Write the errors, if any.
        match errors.is_empty() {
            true => Ok(()),
            false => bail!("{}", errors.iter().join("\n\n")),
        }
    }

    fn save(&self, output: &Self::Output) -> Result<()> {
        if self.rewrite {
            let content = serde_yaml::to_string(&output)?;
            std::fs::write(&self.expectation_path, content)?;
        }
        Ok(())
    }
}
