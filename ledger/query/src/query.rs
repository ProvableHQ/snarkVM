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
use snarkvm_ledger_store::{BlockStorage, BlockStore};
use snarkvm_synthesizer_program::Program;

// ureq re-exports the `http` crate.
use ureq::http;

#[derive(Clone)]
pub enum Query<N: Network, B: BlockStorage<N>> {
    /// The block store from the VM.
    VM(BlockStore<N, B>),
    /// The base URL of the node.
    REST(String),
    STATIC(StaticQuery<N>),
}

impl<N: Network, B: BlockStorage<N>> From<BlockStore<N, B>> for Query<N, B> {
    fn from(block_store: BlockStore<N, B>) -> Self {
        Self::VM(block_store)
    }
}

impl<N: Network, B: BlockStorage<N>> From<&BlockStore<N, B>> for Query<N, B> {
    fn from(block_store: &BlockStore<N, B>) -> Self {
        Self::VM(block_store.clone())
    }
}

impl<N: Network, B: BlockStorage<N>> From<String> for Query<N, B> {
    fn from(string_representation: String) -> Self {
        match string_representation.parse::<StaticQuery<N>>() {
            Ok(query) => Self::STATIC(query),
            Err(_) => Self::REST(string_representation),
        }
    }
}

impl<N: Network, B: BlockStorage<N>> From<&String> for Query<N, B> {
    fn from(string_representation_ref: &String) -> Self {
        let string_representation = string_representation_ref.to_string();
        match string_representation.parse::<StaticQuery<N>>() {
            Ok(query) => Self::STATIC(query),
            Err(_) => Self::REST(string_representation),
        }
    }
}

impl<N: Network, B: BlockStorage<N>> From<&str> for Query<N, B> {
    fn from(str_representation_ref: &str) -> Self {
        let string_representation = str_representation_ref.to_string();
        match string_representation.parse::<StaticQuery<N>>() {
            Ok(query) => Self::STATIC(query),
            Err(_) => Self::REST(string_representation),
        }
    }
}

#[cfg_attr(feature = "async", async_trait(?Send))]
impl<N: Network, B: BlockStorage<N>> QueryTrait<N> for Query<N, B> {
    /// Returns the current state root.
    fn current_state_root(&self) -> Result<N::StateRoot> {
        match self {
            Self::VM(block_store) => Ok(block_store.current_state_root()),
            Self::REST(url) => match N::ID {
                console::network::MainnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/mainnet/stateRoot/latest"))?.body_mut().read_json()?)
                }
                console::network::TestnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/testnet/stateRoot/latest"))?.body_mut().read_json()?)
                }
                console::network::CanaryV0::ID => {
                    Ok(Self::get_request(&format!("{url}/canary/stateRoot/latest"))?.body_mut().read_json()?)
                }
                _ => bail!("Unsupported network ID in inclusion query"),
            },
            Self::STATIC(query) => query.current_state_root(),
        }
    }

    /// Returns the current state root.
    #[cfg(feature = "async")]
    async fn current_state_root_async(&self) -> Result<N::StateRoot> {
        match self {
            Self::VM(block_store) => Ok(block_store.current_state_root()),
            Self::REST(url) => match N::ID {
                console::network::MainnetV0::ID => {
                    Ok(Self::get_request_async(&format!("{url}/mainnet/stateRoot/latest")).await?.json().await?)
                }
                console::network::TestnetV0::ID => {
                    Ok(Self::get_request_async(&format!("{url}/testnet/stateRoot/latest")).await?.json().await?)
                }
                console::network::CanaryV0::ID => {
                    Ok(Self::get_request_async(&format!("{url}/canary/stateRoot/latest")).await?.json().await?)
                }
                _ => bail!("Unsupported network ID in inclusion query"),
            },
            Self::STATIC(_query) => bail!("Async calls are not supported by StaticQuery"),
        }
    }

    /// Returns a state path for the given `commitment`.
    fn get_state_path_for_commitment(&self, commitment: &Field<N>) -> Result<StatePath<N>> {
        match self {
            Self::VM(block_store) => block_store.get_state_path_for_commitment(commitment),
            Self::REST(url) => match N::ID {
                console::network::MainnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/mainnet/statePath/{commitment}"))?.body_mut().read_json()?)
                }
                console::network::TestnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/testnet/statePath/{commitment}"))?.body_mut().read_json()?)
                }
                console::network::CanaryV0::ID => {
                    Ok(Self::get_request(&format!("{url}/canary/statePath/{commitment}"))?.body_mut().read_json()?)
                }
                _ => bail!("Unsupported network ID in inclusion query"),
            },
            Self::STATIC(query) => query.get_state_path_for_commitment(commitment),
        }
    }

    /// Returns a state path for the given `commitment`.
    #[cfg(feature = "async")]
    async fn get_state_path_for_commitment_async(&self, commitment: &Field<N>) -> Result<StatePath<N>> {
        match self {
            Self::VM(block_store) => block_store.get_state_path_for_commitment(commitment),
            Self::REST(url) => match N::ID {
                console::network::MainnetV0::ID => {
                    Ok(Self::get_request_async(&format!("{url}/mainnet/statePath/{commitment}")).await?.json().await?)
                }
                console::network::TestnetV0::ID => {
                    Ok(Self::get_request_async(&format!("{url}/testnet/statePath/{commitment}")).await?.json().await?)
                }
                console::network::CanaryV0::ID => {
                    Ok(Self::get_request_async(&format!("{url}/canary/statePath/{commitment}")).await?.json().await?)
                }
                _ => bail!("Unsupported network ID in inclusion query"),
            },
            Self::STATIC(_query) => bail!("Async calls are not supported by StaticQuery"),
        }
    }

    /// Returns a state path for the given `commitment`.
    fn current_block_height(&self) -> Result<u32> {
        match self {
            Self::VM(block_store) => Ok(block_store.max_height().unwrap_or_default()),
            Self::REST(url) => match N::ID {
                console::network::MainnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/mainnet/block/height/latest"))?.body_mut().read_json()?)
                }
                console::network::TestnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/testnet/block/height/latest"))?.body_mut().read_json()?)
                }
                console::network::CanaryV0::ID => {
                    Ok(Self::get_request(&format!("{url}/canary/block/height/latest"))?.body_mut().read_json()?)
                }
                _ => bail!("Unsupported network ID in inclusion query"),
            },
            Self::STATIC(query) => query.current_block_height(),
        }
    }

    /// Returns a state path for the given `commitment`.
    #[cfg(feature = "async")]
    async fn current_block_height_async(&self) -> Result<u32> {
        match self {
            Self::VM(block_store) => Ok(block_store.max_height().unwrap_or_default()),
            Self::REST(url) => match N::ID {
                console::network::MainnetV0::ID => {
                    Ok(Self::get_request_async(&format!("{url}/mainnet/block/height/latest")).await?.json().await?)
                }
                console::network::TestnetV0::ID => {
                    Ok(Self::get_request_async(&format!("{url}/testnet/block/height/latest")).await?.json().await?)
                }
                console::network::CanaryV0::ID => {
                    Ok(Self::get_request_async(&format!("{url}/canary/block/height/latest")).await?.json().await?)
                }
                _ => bail!("Unsupported network ID in inclusion query"),
            },
            Self::STATIC(_query) => bail!("Async calls are not supported by StaticQuery"),
        }
    }
}

impl<N: Network, B: BlockStorage<N>> Query<N, B> {
    /// Returns the program for the given program ID.
    pub fn get_program(&self, program_id: &ProgramID<N>) -> Result<Program<N>> {
        match self {
            Self::VM(block_store) => block_store
                .get_latest_program(program_id)?
                .ok_or_else(|| anyhow!("Program {program_id} not found in storage")),
            Self::REST(url) => match N::ID {
                console::network::MainnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/mainnet/program/{program_id}"))?.body_mut().read_json()?)
                }
                console::network::TestnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/testnet/program/{program_id}"))?.body_mut().read_json()?)
                }
                console::network::CanaryV0::ID => {
                    Ok(Self::get_request(&format!("{url}/canary/program/{program_id}"))?.body_mut().read_json()?)
                }
                _ => bail!("Unsupported network ID in inclusion query"),
            },
            Self::STATIC(_query) => unimplemented!("get_program is not supported by StaticQuery"),
        }
    }

    /// Returns the program for the given program ID.
    #[cfg(feature = "async")]
    pub async fn get_program_async(&self, program_id: &ProgramID<N>) -> Result<Program<N>> {
        match self {
            Self::VM(block_store) => block_store
                .get_latest_program(program_id)?
                .ok_or_else(|| anyhow!("Program {program_id} not found in storage")),
            Self::REST(url) => match N::ID {
                console::network::MainnetV0::ID => {
                    Ok(Self::get_request_async(&format!("{url}/mainnet/program/{program_id}")).await?.json().await?)
                }
                console::network::TestnetV0::ID => {
                    Ok(Self::get_request_async(&format!("{url}/testnet/program/{program_id}")).await?.json().await?)
                }
                console::network::CanaryV0::ID => {
                    Ok(Self::get_request_async(&format!("{url}/canary/program/{program_id}")).await?.json().await?)
                }
                _ => bail!("Unsupported network ID in inclusion query"),
            },
            Self::STATIC(_query) => unimplemented!("get_program_async is not supported by StaticQuery"),
        }
    }

    /// Performs a GET request to the given URL.
    fn get_request(url: &str) -> Result<http::Response<ureq::Body>> {
        let response = ureq::get(url).call()?;
        if response.status() == http::StatusCode::OK { Ok(response) } else { bail!("Failed to fetch from {url}") }
    }

    /// Performs a GET request to the given URL.
    #[cfg(feature = "async")]
    async fn get_request_async(url: &str) -> Result<reqwest::Response> {
        let response = reqwest::get(url).await?;
        if response.status() == http::StatusCode::OK { Ok(response) } else { bail!("Failed to fetch from {url}") }
    }
}
