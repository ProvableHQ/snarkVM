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

use std::sync::{Arc, OnceLock};

use crate::QueryTrait;
use console::{
    network::prelude::*,
    program::{ProgramID, StatePath},
    types::Field,
};
use ledger_store::{BlockStorage, BlockStore};
use synthesizer_program::Program;

#[derive(Clone)]
pub struct Query<N: Network, B: BlockStorage<N>> {
    target: QueryTarget<N, B>,
    cached_state_root: Arc<OnceLock<N::StateRoot>>,
}

impl<N: Network, B: BlockStorage<N>> Query<N, B> {
    pub fn new(target: QueryTarget<N, B>) -> Self {
        Self { target, cached_state_root: Default::default() }
    }
}

#[derive(Clone)]
pub enum QueryTarget<N: Network, B: BlockStorage<N>> {
    /// The block store from the VM.
    VM(BlockStore<N, B>),
    /// The base URL of the node.
    REST(String),
}

impl<N: Network, B: BlockStorage<N>> From<BlockStore<N, B>> for Query<N, B> {
    fn from(block_store: BlockStore<N, B>) -> Self {
        Self::new(QueryTarget::VM(block_store))
    }
}

impl<N: Network, B: BlockStorage<N>> From<&BlockStore<N, B>> for Query<N, B> {
    fn from(block_store: &BlockStore<N, B>) -> Self {
        Self::new(QueryTarget::VM(block_store.clone()))
    }
}

impl<N: Network, B: BlockStorage<N>> From<String> for Query<N, B> {
    fn from(url: String) -> Self {
        Self::new(QueryTarget::REST(url))
    }
}

impl<N: Network, B: BlockStorage<N>> From<&String> for Query<N, B> {
    fn from(url: &String) -> Self {
        Self::new(QueryTarget::REST(url.to_string()))
    }
}

impl<N: Network, B: BlockStorage<N>> From<&str> for Query<N, B> {
    fn from(url: &str) -> Self {
        Self::new(QueryTarget::REST(url.to_string()))
    }
}

#[cfg_attr(feature = "async", async_trait(?Send))]
impl<N: Network, B: BlockStorage<N>> QueryTrait<N> for Query<N, B> {
    /// Returns the current state root.
    fn current_state_root(&self) -> Result<N::StateRoot> {
        if let Some(csr) = self.cached_state_root.get() {
            return Ok(csr.clone());
        }

        match &self.target {
            QueryTarget::VM(block_store) => {
                let csr = block_store.current_state_root();
                let _ = self.cached_state_root.set(csr.clone());
                Ok(csr)
            }
            QueryTarget::REST(url) => {
                let network = match N::ID {
                    console::network::MainnetV0::ID => "mainnet",
                    console::network::TestnetV0::ID => "testnet",
                    console::network::CanaryV0::ID => "canary",
                    _ => bail!("Unsupported network ID in inclusion query"),
                };
                let csr: N::StateRoot = Self::get_request(&format!("{url}/{network}/stateRoot/latest"))?.into_json()?;
                let _ = self.cached_state_root.set(csr.clone());

                Ok(csr)
            }
        }
    }

    /// Returns the current state root.
    #[cfg(feature = "async")]
    async fn current_state_root_async(&self) -> Result<N::StateRoot> {
        if let Some(csr) = self.cached_state_root.get() {
            return Ok(csr.clone());
        }

        match &self.target {
            QueryTarget::VM(block_store) => {
                let csr = block_store.current_state_root();
                let _ = self.cached_state_root.set(csr.clone());
                Ok(csr)
            }
            QueryTarget::REST(url) => {
                let network = match N::ID {
                    console::network::MainnetV0::ID => "mainnet",
                    console::network::TestnetV0::ID => "testnet",
                    console::network::CanaryV0::ID => "canary",
                    _ => bail!("Unsupported network ID in inclusion query"),
                };
                let csr: N::StateRoot =
                    Self::get_request_async(&format!("{url}/{network}/stateRoot/latest")).await?.json().await?;
                let _ = self.cached_state_root.set(csr.clone());

                Ok(csr)
            }
        }
    }

    /// Returns a state path for the given `commitment`.
    fn get_state_path_for_commitment(&self, commitment: &Field<N>) -> Result<StatePath<N>> {
        match &self.target {
            QueryTarget::VM(block_store) => block_store.get_state_path_for_commitment(commitment),
            QueryTarget::REST(url) => match N::ID {
                console::network::MainnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/mainnet/statePath/{commitment}"))?.into_json()?)
                }
                console::network::TestnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/testnet/statePath/{commitment}"))?.into_json()?)
                }
                console::network::CanaryV0::ID => {
                    Ok(Self::get_request(&format!("{url}/canary/statePath/{commitment}"))?.into_json()?)
                }
                _ => bail!("Unsupported network ID in inclusion query"),
            },
        }
    }

    /// Returns a state path for the given `commitment`.
    #[cfg(feature = "async")]
    async fn get_state_path_for_commitment_async(&self, commitment: &Field<N>) -> Result<StatePath<N>> {
        match &self.target {
            QueryTarget::VM(block_store) => block_store.get_state_path_for_commitment(commitment),
            QueryTarget::REST(url) => match N::ID {
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
        }
    }

    /// Returns a state path for the given `commitment`.
    fn current_block_height(&self) -> Result<u32> {
        match &self.target {
            QueryTarget::VM(block_store) => Ok(block_store.max_height().unwrap_or_default()),
            QueryTarget::REST(url) => match N::ID {
                console::network::MainnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/mainnet/block/height/latest"))?.into_json()?)
                }
                console::network::TestnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/testnet/block/height/latest"))?.into_json()?)
                }
                console::network::CanaryV0::ID => {
                    Ok(Self::get_request(&format!("{url}/canary/block/height/latest"))?.into_json()?)
                }
                _ => bail!("Unsupported network ID in inclusion query"),
            },
        }
    }

    /// Returns a state path for the given `commitment`.
    #[cfg(feature = "async")]
    async fn current_block_height_async(&self) -> Result<u32> {
        match &self.target {
            QueryTarget::VM(block_store) => Ok(block_store.max_height().unwrap_or_default()),
            QueryTarget::REST(url) => match N::ID {
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
        }
    }
}

impl<N: Network, B: BlockStorage<N>> Query<N, B> {
    /// Returns the program for the given program ID.
    pub fn get_program(&self, program_id: &ProgramID<N>) -> Result<Program<N>> {
        match &self.target {
            QueryTarget::VM(block_store) => {
                block_store.get_program(program_id)?.ok_or_else(|| anyhow!("Program {program_id} not found in storage"))
            }
            QueryTarget::REST(url) => match N::ID {
                console::network::MainnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/mainnet/program/{program_id}"))?.into_json()?)
                }
                console::network::TestnetV0::ID => {
                    Ok(Self::get_request(&format!("{url}/testnet/program/{program_id}"))?.into_json()?)
                }
                console::network::CanaryV0::ID => {
                    Ok(Self::get_request(&format!("{url}/canary/program/{program_id}"))?.into_json()?)
                }
                _ => bail!("Unsupported network ID in inclusion query"),
            },
        }
    }

    /// Returns the program for the given program ID.
    #[cfg(feature = "async")]
    pub async fn get_program_async(&self, program_id: &ProgramID<N>) -> Result<Program<N>> {
        match &self.target {
            QueryTarget::VM(block_store) => {
                block_store.get_program(program_id)?.ok_or_else(|| anyhow!("Program {program_id} not found in storage"))
            }
            QueryTarget::REST(url) => match N::ID {
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
        }
    }

    /// Performs a GET request to the given URL.
    fn get_request(url: &str) -> Result<ureq::Response> {
        let response = ureq::get(url).call()?;
        if response.status() == 200 { Ok(response) } else { bail!("Failed to fetch from {url}") }
    }

    /// Performs a GET request to the given URL.
    #[cfg(feature = "async")]
    async fn get_request_async(url: &str) -> Result<reqwest::Response> {
        let response = reqwest::get(url).await?;
        if response.status() == 200 { Ok(response) } else { bail!("Failed to fetch from {url}") }
    }
}
