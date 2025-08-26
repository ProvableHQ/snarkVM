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

use crate::{QueryTrait, StaticQuery};
use console::{
    network::prelude::*,
    program::{ProgramID, StatePath},
    types::Field,
};
use snarkvm_ledger_block::Transaction;
use snarkvm_ledger_store::{BlockStorage, BlockStore};
use snarkvm_synthesizer_program::Program;

use anyhow::{Context, Result};
// ureq re-exports the `http` crate.
use ureq::http::{self, uri};

/// Allows inspecting the state of the blockstain using either local state or a remote endpoint.
#[derive(Clone)]
pub enum Query<N: Network, B: BlockStorage<N>> {
    /// The block store from the VM.
    VM(BlockStore<N, B>),
    /// The base URL of the node.
    REST(http::Uri),
    /// The local state to query.
    STATIC(StaticQuery<N>),
}

/// Initialize the `Query` object from a local `BlockStore`.
impl<N: Network, B: BlockStorage<N>> From<BlockStore<N, B>> for Query<N, B> {
    fn from(block_store: BlockStore<N, B>) -> Self {
        Self::VM(block_store)
    }
}

/// Initialize the `Query` object from a local `BlockStore`.
impl<N: Network, B: BlockStorage<N>> From<&BlockStore<N, B>> for Query<N, B> {
    fn from(block_store: &BlockStore<N, B>) -> Self {
        Self::VM(block_store.clone())
    }
}

/// Initialize the `Query` object from an endpoint URL. The URI should point to a snarkOS node's REST API.
impl<N: Network, B: BlockStorage<N>> From<http::Uri> for Query<N, B> {
    fn from(uri: http::Uri) -> Self {
        Self::REST(uri)
    }
}

/// Initialize the `Query` object from an endpoint URL (passed as a string). The URI should point to a snarkOS node's REST API.
impl<N: Network, B: BlockStorage<N>> TryFrom<String> for Query<N, B> {
    type Error = anyhow::Error;

    fn try_from(string_representation: String) -> Result<Self> {
        Self::try_from(string_representation.as_str())
    }
}

/// Initialize the `Query` object from an endpoint URL (passed as a string). The URI should point to a snarkOS node's REST API.
impl<N: Network, B: BlockStorage<N>> TryFrom<&String> for Query<N, B> {
    type Error = anyhow::Error;

    fn try_from(string_representation: &String) -> Result<Self> {
        Self::try_from(string_representation.as_str())
    }
}

/// Initialize the `Query` object from an endpoint URL (passed as a string). The URI should point to a snarkOS node's REST API.
impl<N: Network, B: BlockStorage<N>> TryFrom<&str> for Query<N, B> {
    type Error = anyhow::Error;

    fn try_from(str_representation: &str) -> Result<Self> {
        str_representation.parse::<Self>()
    }
}

/// Initialize the `Query` object from an endpoint URL (passed as a string). The URI should point to a snarkOS node's REST API.
impl<N: Network, B: BlockStorage<N>> FromStr for Query<N, B> {
    type Err = anyhow::Error;

    fn from_str(str_representation: &str) -> Result<Self> {
        // A static query is represented as JSON and a valid URI does not start with `}`.
        if str_representation.trim().starts_with('{') {
            let static_query =
                str_representation.parse::<StaticQuery<N>>().with_context(|| "Failed to parse static query")?;
            Ok(Self::STATIC(static_query))
        } else {
            let uri = str_representation.parse::<http::Uri>().with_context(|| "Failed to parse URL")?;

            if let Some(scheme) = uri.scheme()
                && *scheme != uri::Scheme::HTTP
                && *scheme != uri::Scheme::HTTPS
            {
                bail!("Invalid scheme in URL: {scheme}");
            }

            if let Some(s) = uri.host()
                && s.is_empty()
            {
                bail!("Invalid URL. Empty hostname given.");
            } else if uri.host().is_none() {
                bail!("Invalid URL. No hostname given.");
            }

            if uri.query().is_some() {
                bail!("Query URL cannot contain a query");
            }

            Ok(Self::REST(uri))
        }
    }
}

#[cfg_attr(feature = "async", async_trait(?Send))]
impl<N: Network, B: BlockStorage<N>> QueryTrait<N> for Query<N, B> {
    /// Returns the current state root.
    fn current_state_root(&self) -> Result<N::StateRoot> {
        match self {
            Self::VM(block_store) => Ok(block_store.current_state_root()),
            Self::REST(base_url) => Self::get_request(base_url, "stateRoot/latest"),
            Self::STATIC(query) => query.current_state_root(),
        }
    }

    /// Returns the current state root.
    #[cfg(feature = "async")]
    async fn current_state_root_async(&self) -> Result<N::StateRoot> {
        match self {
            Self::VM(block_store) => Ok(block_store.current_state_root()),
            Self::REST(base_url) => Self::get_request_async(base_url, "stateRoot/latest").await,
            Self::STATIC(_query) => bail!("Async calls are not supported by StaticQuery"),
        }
    }

    /// Returns a state path for the given `commitment`.
    fn get_state_path_for_commitment(&self, commitment: &Field<N>) -> Result<StatePath<N>> {
        match self {
            Self::VM(block_store) => block_store.get_state_path_for_commitment(commitment),
            Self::REST(base_url) => Self::get_request(base_url, &format!("statePath/{commitment}")),
            Self::STATIC(query) => query.get_state_path_for_commitment(commitment),
        }
    }

    /// Returns a state path for the given `commitment`.
    #[cfg(feature = "async")]
    async fn get_state_path_for_commitment_async(&self, commitment: &Field<N>) -> Result<StatePath<N>> {
        match self {
            Self::VM(block_store) => block_store.get_state_path_for_commitment(commitment),
            Self::REST(base_url) => Self::get_request_async(base_url, &format!("statePath/{commitment}")).await,
            Self::STATIC(_query) => bail!("Async calls are not supported by StaticQuery"),
        }
    }

    /// Returns a list of state paths for the given list of `commitment`s.
    fn get_state_paths_for_commitments(&self, commitments: &[Field<N>]) -> Result<Vec<StatePath<N>>> {
        // Return an empty vector if there are no commitments.
        if commitments.is_empty() {
            return Ok(vec![]);
        }

        match self {
            Self::VM(block_store) => block_store.get_state_paths_for_commitments(commitments),
            Self::REST(base_url) => {
                // Construct the comma separated string of commitments.
                let commitments_string = commitments.iter().map(|cm| cm.to_string()).collect::<Vec<_>>().join(",");
                Self::get_request(base_url, &format!("statePaths?commitments={commitments_string}"))
            }
            Self::STATIC(query) => query.get_state_paths_for_commitments(commitments),
        }
    }

    /// Returns a list of state paths for the given list of `commitment`s.
    #[cfg(feature = "async")]
    async fn get_state_paths_for_commitments_async(&self, commitments: &[Field<N>]) -> Result<Vec<StatePath<N>>> {
        match self {
            Self::VM(block_store) => block_store.get_state_paths_for_commitments(commitments),
            Self::REST(base_url) => {
                // Construct the comma separated string of commitments.
                let commitments_string = commitments.iter().map(|cm| cm.to_string()).collect::<Vec<_>>().join(",");
                Self::get_request_async(base_url, &format!("statePaths?commitments={commitments_string}")).await
            }
            Self::STATIC(query) => query.get_state_paths_for_commitments(commitments),
        }
    }

    /// Returns a state path for the given `commitment`.
    fn current_block_height(&self) -> Result<u32> {
        match self {
            Self::VM(block_store) => Ok(block_store.max_height().unwrap_or_default()),
            Self::REST(base_url) => Self::get_request(base_url, "block/height/latest"),
            Self::STATIC(query) => query.current_block_height(),
        }
    }

    /// Returns a state path for the given `commitment`.
    #[cfg(feature = "async")]
    async fn current_block_height_async(&self) -> Result<u32> {
        match self {
            Self::VM(block_store) => Ok(block_store.max_height().unwrap_or_default()),
            Self::REST(base_url) => Self::get_request_async(base_url, "block/height/latest").await,
            Self::STATIC(_query) => bail!("Async calls are not supported by StaticQuery"),
        }
    }
}

impl<N: Network, B: BlockStorage<N>> Query<N, B> {
    /// Returns the transaction for the given transaction ID.
    pub fn get_transaction(&self, transaction_id: &N::TransactionID) -> Result<Transaction<N>> {
        match self {
            Self::VM(block_store) => {
                let txn = block_store.get_transaction(transaction_id)?;
                txn.ok_or_else(|| anyhow!("Transaction {transaction_id} not in local storage"))
            }
            Self::REST(base_url) => Self::get_request(base_url, &format!("transaction/{transaction_id}")),
            Self::STATIC(_query) => bail!("get_transaction is not supported by StaticQuery"),
        }
    }

    /// Returns the transaction for the given transaction ID.
    #[cfg(feature = "async")]
    pub async fn get_transaction_async(&self, transaction_id: &N::TransactionID) -> Result<Transaction<N>> {
        match self {
            Self::VM(block_store) => {
                let txn = block_store.get_transaction(transaction_id)?;
                txn.ok_or_else(|| anyhow!("Transaction {transaction_id} not in local storage"))
            }
            Self::REST(base_url) => Self::get_request_async(base_url, &format!("transaction/{transaction_id}")).await,
            Self::STATIC(_query) => bail!("get_transaction is not supported by StaticQuery"),
        }
    }

    /// Returns the program for the given program ID.
    pub fn get_program(&self, program_id: &ProgramID<N>) -> Result<Program<N>> {
        match self {
            Self::VM(block_store) => block_store
                .get_latest_program(program_id)?
                .ok_or_else(|| anyhow!("Program {program_id} not found in storage")),
            Self::REST(base_url) => Self::get_request(base_url, &format!("program/{program_id}")),
            Self::STATIC(_query) => bail!("get_program is not supported by StaticQuery"),
        }
    }

    /// Returns the program for the given program ID.
    #[cfg(feature = "async")]
    pub async fn get_program_async(&self, program_id: &ProgramID<N>) -> Result<Program<N>> {
        match self {
            Self::VM(block_store) => block_store
                .get_latest_program(program_id)?
                .with_context(|| format!("Program {program_id} not found in storage")),
            Self::REST(base_url) => Self::get_request_async(base_url, &format!("program/{program_id}")).await,
            Self::STATIC(_query) => bail!("get_program_async is not supported by StaticQuery"),
        }
    }

    /// Builds the full endpoint Uri from the base and path. Used internally
    /// for all REST API calls.
    ///
    /// # Arguments
    ///  - `base_url`: the hostname (and path prefix) of the node to query. this must exclude the network name.
    ///  - `route`: the route to the endpoint (e.g., `stateRoot/latest`). This cannot start with a slash.
    fn build_endpoint(base_url: &http::Uri, route: &str) -> Result<String> {
        // This function is only called internally but check for additional sanity.
        ensure!(!route.starts_with('/'), "path cannot start with a slash");

        // Work around a bug in the `http` crate where empty paths will be set to '/' but other paths are not appended with a slash.
        // See [this issue](https://github.com/hyperium/http/issues/507).
        let path = if base_url.path().ends_with('/') {
            format!("{base_url}{network}/{route}", network = N::SHORT_NAME)
        } else {
            format!("{base_url}/{network}/{route}", network = N::SHORT_NAME)
        };

        Ok(path)
    }

    /// Performs a GET request to the given URL and deserializes returned JSON.
    fn get_request<T: DeserializeOwned>(base_url: &http::Uri, path: &str) -> Result<T> {
        let endpoint = Self::build_endpoint(base_url, path)?;
        let mut response = ureq::get(&endpoint).call().with_context(|| format!("Failed to fetch from {endpoint}"))?;
        if response.status() != http::StatusCode::OK {
            // NOTE: ureq will return an error in this case, but we are keeping the check just in case.
            bail!("Failed to fetch from {endpoint}: Server returned status {}", response.status());
        }

        response.body_mut().read_json().with_context(|| "Failed to parse JSON response")
    }

    /// Performs a GET request to the given URL and deserializes returned JSON (async version).
    #[cfg(feature = "async")]
    async fn get_request_async<T: DeserializeOwned>(base_url: &http::Uri, path: &str) -> Result<T> {
        let endpoint = Self::build_endpoint(base_url, path)?;
        let response = reqwest::get(&endpoint).await.with_context(|| format!("Failed to fetch from {endpoint}"))?;
        if response.status() != http::StatusCode::OK {
            bail!("Failed to fetch from {endpoint}: Server returned status {}", response.status());
        }

        response.json().await.with_context(|| "Failed to parse JSON response")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::TestnetV0;

    use snarkvm_ledger_store::helpers::memory::BlockMemory;

    type CurrentNetwork = TestnetV0;
    type CurrentQuery = Query<CurrentNetwork, BlockMemory<CurrentNetwork>>;

    #[test]
    fn test_static_query_parse() {
        let json = r#"{"state_root": "sr1dz06ur5spdgzkguh4pr42mvft6u3nwsg5drh9rdja9v8jpcz3czsls9geg", "height": 14}"#
            .to_string();
        let query = CurrentQuery::try_from(json).unwrap();

        assert!(matches!(query, Query::STATIC(_)));
    }

    #[test]
    fn test_static_query_parse_invalid() {
        let json = r#"{"invalid_key": "sr1dz06ur5spdgzkguh4pr42mvft6u3nwsg5drh9rdja9v8jpcz3czsls9geg", "height": 14}"#
            .to_string();
        let result = json.parse::<CurrentQuery>();

        assert!(result.is_err());
    }

    /// Tests HTTP's behavior of printing an empty path `/`
    ///
    /// `generate_endpoint` can handle base_urls with and without a trailing slash.
    /// However, this test is still useful to see if the behavior changes in the future and a second slash is not
    /// appended to a URL with an existing trailing slash.
    #[test]
    fn test_rest_url_parse() {
        let noslash = "http://localhost:3030";
        let withslash = format!("{noslash}/");

        let query = noslash.parse::<CurrentQuery>().unwrap();
        let Query::REST(base_uri) = query else { panic!() };
        assert_eq!(base_uri.path_and_query().unwrap().to_string(), "/");
        assert_eq!(base_uri.to_string(), withslash);

        let query = withslash.parse::<CurrentQuery>().unwrap();
        let Query::REST(base_uri) = query else { panic!() };
        assert_eq!(base_uri.path_and_query().unwrap().to_string(), "/");
        assert_eq!(base_uri.to_string(), withslash);
    }

    #[test]
    fn test_rest_url_with_colon_parse() {
        let str = "http://myendpoint.addr/:var/foo/bar";
        let query = str.parse::<CurrentQuery>().unwrap();

        let Query::REST(base_uri) = query else { panic!() };

        assert_eq!(base_uri.to_string(), format!("{str}"));
        assert_eq!(base_uri.path_and_query().unwrap().to_string(), "/:var/foo/bar");
    }

    #[test]
    fn test_rest_url_parse_with_suffix() -> Result<()> {
        let base = "http://localhost:3030/a/prefix";
        let route = "a/route";
        let query = base.parse::<CurrentQuery>().unwrap();

        // Test without trailing slash.
        let Query::REST(base_uri) = query else { panic!() };
        assert_eq!(CurrentQuery::build_endpoint(&base_uri, route)?, format!("{base}/testnet/{route}"));

        // Set again with trailing slash.
        let query = format!("{base}/").parse::<CurrentQuery>().unwrap();
        let Query::REST(base_uri) = query else { panic!() };
        assert_eq!(CurrentQuery::build_endpoint(&base_uri, route)?, format!("{base}/testnet/{route}"));

        Ok(())
    }

    #[test]
    fn test_rest_url_parse_invalid_scheme() {
        let str = "ftp://localhost:3030";
        let result = CurrentQuery::try_from(str);

        assert!(result.is_err());
    }

    #[test]
    fn test_rest_url_parse_invalid_host() {
        let str = "http://:3030";
        let result = CurrentQuery::try_from(str);

        assert!(result.is_err());
    }
}
