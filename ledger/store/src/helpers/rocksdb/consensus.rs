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
    BlockStore,
    ConsensusStorage,
    FinalizeStore,
    helpers::rocksdb::{BlockDB, FinalizeDB, TransactionDB, TransitionDB},
};
use console::prelude::*;

use aleo_std_storage::StorageMode;
#[cfg(any(test, feature = "test"))]
use std::sync::Arc;

/// An RocksDB consensus storage.
#[derive(Clone)]
pub struct ConsensusDB<N: Network> {
    /// The finalize store.
    finalize_store: FinalizeStore<N, FinalizeDB<N>>,
    /// The block store.
    block_store: BlockStore<N, BlockDB<N>>,
    /// A test-only instance of TempDir which is cleaned up afterwards.
    #[cfg(any(test, feature = "test"))]
    _temp_dir: Arc<tempfile::TempDir>,
}

#[rustfmt::skip]
impl<N: Network> ConsensusStorage<N> for ConsensusDB<N> {
    type FinalizeStorage = FinalizeDB<N>;
    type BlockStorage = BlockDB<N>;
    type TransactionStorage = TransactionDB<N>;
    type TransitionStorage = TransitionDB<N>;

    /// Initializes the consensus storage.
    #[cfg(not(any(test, feature = "test")))]
    fn open<S: Clone + Into<StorageMode>>(storage: S) -> Result<Self> {
        // Initialize the finalize store.
        let finalize_store = FinalizeStore::<N, FinalizeDB<N>>::open(storage.clone())?;
        // Initialize the block store.
        let block_store = BlockStore::<N, BlockDB<N>>::open(storage)?;
        // Return the consensus storage.
        Ok(Self {
            finalize_store,
            block_store,
        })
    }

    /// Initializes a test-only consensus storage.
    #[cfg(any(test, feature = "test"))]
    fn open<S: Clone + Into<StorageMode>>(_storage: S) -> Result<Self> {
        // Overwrite any given storage mode with a path to a temporary directory.
        let temp_dir = Arc::new(tempfile::TempDir::with_prefix("snarkos_test_")?);
        let storage = StorageMode::Custom(temp_dir.path().to_owned());

        // Initialize the finalize store.
        let finalize_store = FinalizeStore::<N, FinalizeDB<N>>::open(storage.clone())?;
        // Initialize the block store.
        let block_store = BlockStore::<N, BlockDB<N>>::open(storage)?;
        // Return the consensus storage.
        Ok(Self {
            finalize_store,
            block_store,
            _temp_dir: temp_dir,
        })
    }

    /// Returns the finalize store.
    fn finalize_store(&self) -> &FinalizeStore<N, Self::FinalizeStorage> {
        &self.finalize_store
    }

    /// Returns the block store.
    fn block_store(&self) -> &BlockStore<N, Self::BlockStorage> {
        &self.block_store
    }
}
