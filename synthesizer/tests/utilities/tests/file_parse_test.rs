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
use std::path::{Path, PathBuf};

/// A test for a parser, where the entire file is the test input.
pub struct FileParseTest {
    test_string: String,
    expectation: String,
    expectation_path: PathBuf,
    rewrite: bool,
}

impl FileParseTest {
    /// Returns the test string.
    pub fn test_string(&self) -> &String {
        &self.test_string
    }
}

impl ExpectedTest for FileParseTest {
    type Output = String;

    /// Loads the test from a given path.
    fn load<P: AsRef<Path>>(test_path: P, expectation_dir: P) -> Self {
        // Read the contents of the test file.
        let test_string = std::fs::read_to_string(&test_path).expect("Failed to read test file.");
        // Check if the expectation file should be rewritten.
        let rewrite = std::env::var("REWRITE_EXPECTATIONS").is_ok();
        // Construct the path the expectation file.
        let expectation_path = get_expectation_path(&test_path, &expectation_dir);
        // If the expectation file should be rewritten, then there is no need to read the expectation file.
        let expectation = match rewrite {
            true => String::new(),
            false => std::fs::read_to_string(&expectation_path).expect("Failed to read expectation file."),
        };
        Self { test_string, expectation, expectation_path, rewrite }
    }

    fn check(&self, output: &Self::Output) -> Result<()> {
        match self.rewrite {
            false if self.expectation != *output => {
                bail!("{}", print_difference(&self.test_string, &self.expectation, output))
            }
            _ => Ok(()),
        }
    }

    fn save(&self, output: &Self::Output) -> Result<()> {
        if self.rewrite {
            std::fs::write(&self.expectation_path, output)?;
        }
        Ok(())
    }
}
