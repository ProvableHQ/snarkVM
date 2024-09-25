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

use std::{
    env::current_dir,
    fmt::Display,
    path::{Path, PathBuf},
};

/// Constructs the path to the expectation file corresponding to the given test file.
pub fn get_expectation_path<P: AsRef<Path>>(test_path: P, expectation_dir: P) -> PathBuf {
    // Get the current directory.
    let current_dir = current_dir().expect("Failed to get current directory.");
    // Construct the path to the directory containing the expectation file.
    let expectation_dir = current_dir.join("tests").join(expectation_dir);
    // Construct the path to the expectation file.
    expectation_dir.join(Path::new(test_path.as_ref().file_name().unwrap()).with_extension("out"))
}

/// Helper function to print the difference between the expected and actual output.
pub fn print_difference(test: impl Display, expected: impl Display, actual: impl Display) -> String {
    let mut message = r"
============================================================
TEST
------------------------------------------------------------
"
    .to_string();
    message.push_str(&test.to_string());
    message.push_str(
        r"

============================================================
EXPECTED
------------------------------------------------------------
",
    );
    message.push_str(&expected.to_string());
    message.push_str(
        r"

============================================================
ACTUAL
------------------------------------------------------------
",
    );
    message.push_str(&actual.to_string());
    message.push_str(
        r"
============================================================

",
    );
    message
}
