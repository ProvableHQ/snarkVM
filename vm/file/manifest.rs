// Copyright (c) 2019-2025 Provable Inc.
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

use snarkvm_circuit::prelude::IndexMap;

use anyhow::{Result, anyhow, ensure};
use core::str::FromStr;
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

const MANIFEST_FILE_NAME: &str = "program.json";

pub struct Manifest<N: Network> {
    /// The file path.
    path: PathBuf,
    /// The program ID.
    program_id: ProgramID<N>,
    /// The program editions.
    editions: IndexMap<ProgramID<N>, u16>,
}

impl<N: Network> Manifest<N> {
    /// Creates a new manifest file with the given directory path and program ID.
    pub fn create(directory: &Path, id: &ProgramID<N>) -> Result<Self> {
        // Ensure the directory path exists.
        ensure!(directory.exists(), "The program directory does not exist: '{}'", directory.display());
        // Ensure the program name is valid.
        ensure!(!Program::is_reserved_keyword(id.name()), "Program name is invalid (reserved): {id}");

        // Construct the initial program manifest string.
        let manifest_string = format!(
            r#"{{
    "program": "{id}",
    "version": "0.0.0",
    "description": "",
    "license": "MIT",
    "editions": "{{}}"
}}
"#
        );

        // Construct the file path.
        let path = directory.join(MANIFEST_FILE_NAME);
        // Ensure the file path does not already exist.
        ensure!(!path.exists(), "Manifest file already exists: '{}'", path.display());

        // Write the file.
        File::create(&path)?.write_all(manifest_string.as_bytes())?;

        // Return the manifest file.
        Ok(Self { path, program_id: *id, editions: IndexMap::new() })
    }

    /// Opens the manifest file for reading.
    pub fn open(directory: &Path) -> Result<Self> {
        // Ensure the directory path exists.
        ensure!(directory.exists(), "The program directory does not exist: '{}'", directory.display());

        // Construct the file path.
        let path = directory.join(MANIFEST_FILE_NAME);
        // Ensure the file path exists.
        ensure!(path.exists(), "Manifest file is missing: '{}'", path.display());

        // Read the file to a string.
        let manifest_string = fs::read_to_string(&path)?;
        let json: serde_json::Value = serde_json::from_str(&manifest_string)?;

        // Retrieve the program ID.
        let id_string = json["program"].as_str().ok_or_else(|| anyhow!("Program ID not found."))?;
        let id = ProgramID::from_str(id_string)?;
        // Ensure the program name is valid.
        ensure!(!Program::is_reserved_keyword(id.name()), "Program name is invalid (reserved): {id}");

        // Initialize storage for the program editions.
        let mut editions = IndexMap::<ProgramID<N>, u16>::new();

        // Retrieve the editions in the manifest, if they exist.
        if let Some(editions_object) = json["editions"].as_object() {
            for (id_string, value) in editions_object {
                let id = ProgramID::from_str(id_string)?;
                let value = u16::try_from(value.as_u64().ok_or_else(|| anyhow!("The edition must be a number."))?)?;
                editions.insert(id, value);
            }
        }

        // Return the manifest file.
        Ok(Self { path, program_id: id, editions })
    }

    /// Returns `true` if the manifest file exists at the given path.
    pub fn exists_at(directory: &Path) -> bool {
        // Construct the file path.
        let path = directory.join(MANIFEST_FILE_NAME);
        // Return the result.
        path.is_file() && path.exists()
    }

    /// Returns the manifest file name.
    pub const fn file_name() -> &'static str {
        MANIFEST_FILE_NAME
    }

    /// Returns the file path.
    pub const fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Returns the program ID.
    pub const fn program_id(&self) -> &ProgramID<N> {
        &self.program_id
    }

    /// Returns the program editions.
    pub const fn editions(&self) -> &IndexMap<ProgramID<N>, u16> {
        &self.editions
    }
}
