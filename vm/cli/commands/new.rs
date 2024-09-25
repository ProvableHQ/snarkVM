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

/// Create a new Aleo package.
#[derive(Debug, Parser)]
pub struct New {
    /// The program name.
    name: String,
}

impl New {
    /// Creates an Aleo package with the specified name.
    pub fn parse(self) -> Result<String> {
        // Derive the program directory path.
        let mut path = std::env::current_dir()?;
        path.push(&self.name);

        // Create the program ID from the name.
        let id = ProgramID::<CurrentNetwork>::from_str(&format!("{}.aleo", self.name))?;

        // Create the package.
        Package::create(&path, &id)?;

        // Prepare the path string.
        let path_string = format!("(in \"{}\")", path.display());

        Ok(format!("✅ Created an Aleo program '{}' {}", self.name.bold(), path_string.dimmed()))
    }
}
