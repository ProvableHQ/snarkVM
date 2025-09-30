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

use crate::QueryTrait;

use snarkvm_console::{
    network::Network,
    program::{ProgramID, StatePath},
    types::Field,
};
use snarkvm_ledger_block::Transaction;
use snarkvm_synthesizer_program::Program;

use anyhow::{Context, Result, anyhow, bail, ensure};
use serde::{Deserialize, de::DeserializeOwned};
use ureq::http::{self, uri};

use std::str::FromStr;

/// Queries that use a node's REST API as their source of information.
#[derive(Clone)]
pub struct RestQuery<N: Network> {
    base_url: http::Uri,
    _marker: std::marker::PhantomData<N>,
}

impl<N: Network> From<http::Uri> for RestQuery<N> {
    fn from(base_url: http::Uri) -> Self {
        Self { base_url, _marker: Default::default() }
    }
}

/// The serialized REST error sent over the network.
#[derive(Debug, Deserialize)]
pub struct RestError {
    /// The type of error (corresponding to the HTTP status code).
    error_type: String,
    /// The top-level error message.
    message: String,
    /// The chain of errors that led to the top-level error.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    chain: Vec<String>,
}

impl RestError {
    /// Converts a `RestError` into an `anyhow::Error`.
    pub fn parse(self) -> anyhow::Error {
        let mut error: Option<anyhow::Error> = None;
        for next in self.chain.into_iter() {
            if let Some(previous) = error {
                error = Some(previous.context(next));
            } else {
                error = Some(anyhow!(next));
            }
        }

        let toplevel = format!("{}: {}", self.error_type, self.message);
        if let Some(error) = error { error.context(toplevel) } else { anyhow!(toplevel) }
    }
}

/// Initialize the `Query` object from an endpoint URL (passed as a string). The URI should point to a snarkOS node's REST API.
impl<N: Network> FromStr for RestQuery<N> {
    type Err = anyhow::Error;

    fn from_str(str_representation: &str) -> Result<Self> {
        let base_url = str_representation.parse::<http::Uri>().with_context(|| "Failed to parse URL")?;

        if let Some(scheme) = base_url.scheme()
            && *scheme != uri::Scheme::HTTP
            && *scheme != uri::Scheme::HTTPS
        {
            bail!("Invalid scheme in URL: {scheme}");
        }

        if let Some(s) = base_url.host()
            && s.is_empty()
        {
            bail!("Invalid URL for REST endpoint. Empty hostname given.");
        } else if base_url.host().is_none() {
            bail!("Invalid URL for REST endpoint. No hostname given.");
        }

        if base_url.query().is_some() {
            bail!("Base URL for REST endpoints cannot contain a query");
        }

        Ok(Self { base_url, _marker: Default::default() })
    }
}

#[cfg_attr(feature = "async", async_trait::async_trait(?Send))]
impl<N: Network> QueryTrait<N> for RestQuery<N> {
    /// Returns the current state root.
    fn current_state_root(&self) -> Result<N::StateRoot> {
        self.get_request("stateRoot/latest")
    }

    /// Returns the current state root.
    #[cfg(feature = "async")]
    async fn current_state_root_async(&self) -> Result<N::StateRoot> {
        self.get_request_async("stateRoot/latest").await
    }

    /// Returns a state path for the given `commitment`.
    fn get_state_path_for_commitment(&self, commitment: &Field<N>) -> Result<StatePath<N>> {
        self.get_request(&format!("statePath/{commitment}"))
    }

    /// Returns a state path for the given `commitment`.
    #[cfg(feature = "async")]
    async fn get_state_path_for_commitment_async(&self, commitment: &Field<N>) -> Result<StatePath<N>> {
        self.get_request_async(&format!("statePath/{commitment}")).await
    }

    /// Returns a list of state paths for the given list of `commitment`s.
    fn get_state_paths_for_commitments(&self, commitments: &[Field<N>]) -> Result<Vec<StatePath<N>>> {
        // Construct the comma separated string of commitments.
        let commitments_string = commitments.iter().map(|cm| cm.to_string()).collect::<Vec<_>>().join(",");
        self.get_request(&format!("statePaths?commitments={commitments_string}"))
    }

    /// Returns a list of state paths for the given list of `commitment`s.
    #[cfg(feature = "async")]
    async fn get_state_paths_for_commitments_async(&self, commitments: &[Field<N>]) -> Result<Vec<StatePath<N>>> {
        // Construct the comma separated string of commitments.
        let commitments_string = commitments.iter().map(|cm| cm.to_string()).collect::<Vec<_>>().join(",");
        self.get_request_async(&format!("statePaths?commitments={commitments_string}")).await
    }

    /// Returns a state path for the given `commitment`.
    fn current_block_height(&self) -> Result<u32> {
        self.get_request("block/height/latest")
    }

    /// Returns a state path for the given `commitment`.
    #[cfg(feature = "async")]
    async fn current_block_height_async(&self) -> Result<u32> {
        self.get_request_async("block/height/latest").await
    }
}

impl<N: Network> RestQuery<N> {
    /// The API version to use when querying REST endpoints.
    const API_VERSION: &str = "v2";

    /// Returns the transaction for the given transaction ID.
    pub fn get_transaction(&self, transaction_id: &N::TransactionID) -> Result<Transaction<N>> {
        self.get_request(&format!("transaction/{transaction_id}"))
    }

    /// Returns the transaction for the given transaction ID.
    #[cfg(feature = "async")]
    pub async fn get_transaction_async(&self, transaction_id: &N::TransactionID) -> Result<Transaction<N>> {
        self.get_request_async(&format!("transaction/{transaction_id}")).await
    }

    /// Returns the program for the given program ID.
    pub fn get_program(&self, program_id: &ProgramID<N>) -> Result<Program<N>> {
        self.get_request(&format!("program/{program_id}"))
    }

    /// Returns the program for the given program ID.
    #[cfg(feature = "async")]
    pub async fn get_program_async(&self, program_id: &ProgramID<N>) -> Result<Program<N>> {
        self.get_request_async(&format!("program/{program_id}")).await
    }

    /// Builds the full endpoint Uri from the base and path. Used internally
    /// for all REST API calls.
    ///
    /// # Arguments
    ///  - `route`: the route to the endpoint (e.g., `stateRoot/latest`). This cannot start with a slash.
    fn build_endpoint(&self, route: &str) -> Result<String> {
        // This function is only called internally but check for additional sanity.
        ensure!(!route.starts_with('/'), "path cannot start with a slash");

        // Work around a bug in the `http` crate where empty paths will be set to '/' but other paths are not appended with a slash.
        // See [this issue](https://github.com/hyperium/http/issues/507).
        let path = if self.base_url.path().ends_with('/') {
            format!(
                "{base_url}{api_version}/{network}/{route}",
                base_url = self.base_url,
                api_version = Self::API_VERSION,
                network = N::SHORT_NAME
            )
        } else {
            format!(
                "{base_url}/{api_version}/{network}/{route}",
                base_url = self.base_url,
                api_version = Self::API_VERSION,
                network = N::SHORT_NAME
            )
        };

        Ok(path)
    }

    /// Performs a GET request to the given URL and deserializes the returned JSON.
    ///
    /// # Arguments
    ///  - `route`: the specific API route to use, e.g., `stateRoot/latest`
    fn get_request<T: DeserializeOwned>(&self, route: &str) -> Result<T> {
        let endpoint = self.build_endpoint(route)?;
        let mut response = ureq::get(&endpoint)
            .config()
            .http_status_as_error(false)
            .build()
            .call()
            // This handles I/O errors.
            .with_context(|| format!("Failed to fetch from {endpoint}"))?;

        if response.status().is_success() {
            response.body_mut().read_json().with_context(|| "Failed to parse JSON response")
        } else {
            // Convert returned error into an `anyhow::Error`.
            let error: RestError =
                response.body_mut().read_json().with_context(|| "Failed to parse JSON error response")?;
            Err(error.parse().context(format!("Failed to fetch from {endpoint}")))
        }
    }

    /// Async version of [`Self::get_request`]. Performs a GET request to the given URL and deserializes the returned JSON.
    ///
    /// # Arguments
    ///  - `route`: the specific API route to use, e.g., `stateRoot/latest`
    #[cfg(feature = "async")]
    async fn get_request_async<T: DeserializeOwned>(&self, route: &str) -> Result<T> {
        let endpoint = self.build_endpoint(route)?;
        let response = reqwest::get(&endpoint).await.with_context(|| format!("Failed to fetch from {endpoint}"))?;

        if response.status().is_success() {
            response.json().await.with_context(|| "Failed to parse JSON response")
        } else {
            // Convert returned error into an `anyhow::Error`.
            let error: RestError = response.json().await.with_context(|| "Failed to parse JSON error response")?;
            Err(error.parse().context(format!("Failed to fetch from {endpoint}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Query;

    use snarkvm_console::network::TestnetV0;
    use snarkvm_ledger_store::helpers::memory::BlockMemory;

    use anyhow::Result;

    type CurrentNetwork = TestnetV0;
    type CurrentQuery = Query<CurrentNetwork, BlockMemory<CurrentNetwork>>;

    /// Tests HTTP's behavior of printing an empty path `/`
    ///
    /// `generate_endpoint` can handle base_urls with and without a trailing slash.
    /// However, this test is still useful to see if the behavior changes in the future and a second slash is not
    /// appended to a URL with an existing trailing slash.
    #[test]
    fn test_rest_url_parse() -> Result<()> {
        let noslash = "http://localhost:3030";
        let withslash = format!("{noslash}/");
        let route = "some/route";

        let query = noslash.parse::<CurrentQuery>().unwrap();
        let Query::REST(rest) = query else { panic!() };
        assert_eq!(rest.base_url.path_and_query().unwrap().to_string(), "/");
        assert_eq!(rest.base_url.to_string(), withslash);
        assert_eq!(rest.build_endpoint(route)?, format!("{noslash}/v2/testnet/{route}"));

        let query = withslash.parse::<CurrentQuery>().unwrap();
        let Query::REST(rest) = query else { panic!() };
        assert_eq!(rest.base_url.path_and_query().unwrap().to_string(), "/");
        assert_eq!(rest.base_url.to_string(), withslash);
        assert_eq!(rest.build_endpoint(route)?, format!("{noslash}/v2/testnet/{route}"));

        Ok(())
    }

    #[test]
    fn test_rest_url_with_colon_parse() {
        let str = "http://myendpoint.addr/:var/foo/bar";
        let query = str.parse::<CurrentQuery>().unwrap();

        let Query::REST(rest) = query else { panic!() };
        assert_eq!(rest.base_url.to_string(), format!("{str}"));
        assert_eq!(rest.base_url.path_and_query().unwrap().to_string(), "/:var/foo/bar");
    }

    #[test]
    fn test_rest_url_parse_with_suffix() -> Result<()> {
        let base = "http://localhost:3030/a/prefix";
        let route = "a/route";
        let query = base.parse::<CurrentQuery>().unwrap();

        // Test without trailing slash.
        let Query::REST(rest) = query else { panic!() };
        assert_eq!(rest.build_endpoint(route)?, format!("{base}/v2/testnet/{route}"));

        // Set again with trailing slash.
        let query = format!("{base}/").parse::<CurrentQuery>().unwrap();
        let Query::REST(rest) = query else { panic!() };
        assert_eq!(rest.build_endpoint(route)?, format!("{base}/v2/testnet/{route}"));

        Ok(())
    }
}
