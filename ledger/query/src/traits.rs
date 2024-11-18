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

use console::{network::Network, prelude::Result, program::StatePath, types::Field};

#[cfg_attr(feature = "async", async_trait(?Send))]
pub trait QueryTrait<N: Network> {
    /// Returns the current state root.
    fn current_state_root(&self) -> Result<N::StateRoot>;

    /// Returns the current state root.
    #[cfg(feature = "async")]
    async fn current_state_root_async(&self) -> Result<N::StateRoot>;

    /// Returns a state path for the given `commitment`.
    fn get_state_path_for_commitment(&self, commitment: &Field<N>) -> Result<StatePath<N>>;

    /// Returns a state path for the given `commitment`.
    #[cfg(feature = "async")]
    async fn get_state_path_for_commitment_async(&self, commitment: &Field<N>) -> Result<StatePath<N>>;

    /// Returns the current block height
    fn current_block_height(&self) -> Result<u32>;

    /// Returns the current block height
    #[cfg(feature = "async")]
    async fn current_block_height_async(&self) -> Result<u32>;
}
