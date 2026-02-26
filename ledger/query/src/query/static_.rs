// Copyright (c) 2019-2026 Provable Inc.
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

use crate::QueryTrait;

use snarkvm_console::{network::prelude::*, program::StatePath, types::Field};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{collections::HashMap, str::FromStr};

#[derive(Clone)]
pub struct StaticQuery<N: Network> {
    block_height: u32,
    state_root: N::StateRoot,
    state_paths: HashMap<Field<N>, StatePath<N>>,
}

impl<N: Network> StaticQuery<N> {
    pub fn new(block_height: u32, state_root: N::StateRoot, state_paths: HashMap<Field<N>, StatePath<N>>) -> Self {
        Self { block_height, state_root, state_paths }
    }
}

#[derive(Deserialize)]
struct StaticQueryInput {
    state_root: String,
    height: u32,
}

impl<N: Network> FromStr for StaticQuery<N> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        ensure!(s.trim().starts_with('{'), "Not a static query");

        let input: StaticQueryInput = serde_json::from_str(s).with_context(|| "Invalid JSON format in static query")?;
        let state_root = N::StateRoot::from_str(&input.state_root).map_err(|_| anyhow!("Invalid state root format"))?;

        Ok(Self { state_root, block_height: input.height, state_paths: HashMap::new() })
    }
}

#[cfg_attr(feature = "async", async_trait::async_trait(?Send))]
impl<N: Network> QueryTrait<N> for StaticQuery<N> {
    /// Returns the current state root.
    fn current_state_root(&self) -> Result<N::StateRoot> {
        Ok(self.state_root)
    }

    /// Returns the current state root (async version).
    #[cfg(feature = "async")]
    async fn current_state_root_async(&self) -> Result<N::StateRoot> {
        // There is no I/O in StaticQuery, so the sync version is identical.
        self.current_state_root()
    }

    /// Returns a state path for the given `commitment`.
    fn get_state_path_for_commitment(&self, commitment: &Field<N>) -> Result<StatePath<N>> {
        match self.state_paths.get(commitment) {
            Some(state_path) => Ok(state_path.clone()),
            None => bail!("Could not find state path for commitment '{commitment}'"),
        }
    }

    /// Returns a state path for the given `commitment` (async version).
    #[cfg(feature = "async")]
    async fn get_state_path_for_commitment_async(&self, commitment: &Field<N>) -> Result<StatePath<N>> {
        // There is no I/O in StaticQuery, so the sync version is identical.
        self.get_state_path_for_commitment(commitment)
    }

    /// Returns a list of state paths for the given list of `commitment`s.
    fn get_state_paths_for_commitments(&self, commitments: &[Field<N>]) -> Result<Vec<StatePath<N>>> {
        commitments
            .iter()
            .map(|commitment| self.get_state_path_for_commitment(commitment))
            .collect::<Result<Vec<StatePath<N>>>>()
    }

    /// Returns a list of state paths for the given list of `commitment`s (async version).
    #[cfg(feature = "async")]
    async fn get_state_paths_for_commitments_async(&self, commitments: &[Field<N>]) -> Result<Vec<StatePath<N>>> {
        // There is no I/O in StaticQuery, so the sync version is identical.
        self.get_state_paths_for_commitments(commitments)
    }

    /// Returns the current block height.
    fn current_block_height(&self) -> Result<u32> {
        Ok(self.block_height)
    }

    /// Returns the current block height (async version).
    #[cfg(feature = "async")]
    async fn current_block_height_async(&self) -> Result<u32> {
        // There is no I/O in StaticQuery, so the sync version is identical.
        self.current_block_height()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console::network::TestnetV0;

    #[test]
    fn test_static_query_parse() {
        let json = r#"{"state_root": "sr1dz06ur5spdgzkguh4pr42mvft6u3nwsg5drh9rdja9v8jpcz3czsls9geg", "height": 14}"#
            .to_string();
        let query: Result<StaticQuery<TestnetV0>> = json.parse();
        assert!(query.is_ok());
    }

    #[test]
    fn test_static_query_parse_invalid() {
        let json = r#"{"invalid_key": "sr1dz06ur5spdgzkguh4pr42mvft6u3nwsg5drh9rdja9v8jpcz3czsls9geg", "height": 14}"#
            .to_string();
        let query: Result<StaticQuery<TestnetV0>> = json.parse();
        assert!(query.is_err());
    }
}
