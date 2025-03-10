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

use crate::{
    prelude::{Network, ProgramID},
    synthesizer::Program,
};

use anyhow::{Result, ensure};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

pub struct README {
    /// The file path.
    path: PathBuf,
}

impl README {
    /// Creates a new README file with the given directory path and program ID.
    pub fn create<N: Network>(directory: &Path, id: &ProgramID<N>) -> Result<Self> {
        // Ensure the directory path exists.
        ensure!(directory.exists(), "The program directory does not exist: {}", directory.display());
        // Ensure the program name is valid.
        ensure!(!Program::is_reserved_keyword(id.name()), "Program name is invalid (reserved): {id}");

        // Construct the initial README string.
        let readme_string = format!(
            r"# {id}

## Build Guide

To compile this Aleo program, run:
```bash
snarkvm build
```

To execute this Aleo program, run:
```bash
snarkvm run hello
```
"
        );

        // Construct the file name.
        let file_name = "README.md".to_string();

        // Construct the file path.
        let path = directory.join(file_name);

        // Ensure the file path does not already exist.
        ensure!(!path.exists(), "README file already exists: {}", path.display());

        // Write the file.
        File::create(&path)?.write_all(readme_string.as_bytes())?;

        // Return the README file.
        Ok(Self { path })
    }

    /// Returns the file path.
    pub const fn path(&self) -> &PathBuf {
        &self.path
    }
}
