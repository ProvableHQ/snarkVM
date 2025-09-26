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

use crate::block::{Block, Network};

use snarkvm_utilities::ensure_equals;

use std::collections::VecDeque;

use anyhow::Result;

/// Helper struct for caching the most recent blocks.
pub(super) struct BlockCache<N: Network> {
    /// Contains the most recent blocks ordered by height.
    /// We do not use a BTreeMap here as the cache is small and updates to a vector are more efficient
    blocks: VecDeque<Block<N>>,
}

impl<N: Network> BlockCache<N> {
    /// The maximum size of the cache in blocks.
    pub(super) const BLOCK_CACHE_SIZE: u32 = 10;

    /// Initialize the cache with the given blocks.
    pub fn new(blocks: Vec<Block<N>>) -> Result<Self> {
        Ok(Self { blocks: VecDeque::from(blocks) })
    }

    /// Insert a new block into the cache.
    /// Must be the successor of the last block inserted into the cache.
    #[inline]
    pub fn insert(&mut self, block: Block<N>) -> Result<()> {
        if let Some(prev) = self.blocks.back() {
            ensure_equals!(
                prev.height() + 1,
                block.height(),
                "Block is not the successor of the last block inserted into the cache"
            );
        }

        self.blocks.push_back(block.clone());
        if self.blocks.len() > (Self::BLOCK_CACHE_SIZE as usize) {
            self.blocks.pop_front();
        }

        Ok(())
    }

    /// Return the block at the given height if it is in the cache.i
    #[inline]
    pub fn get_block(&self, block_height: u32) -> Option<&Block<N>> {
        // Is the block height in the range of the cache?
        let Some(first_block) = self.blocks.front() else {
            // Cache is empty
            return None;
        };

        let offset = block_height.checked_sub(first_block.height())?;
        self.blocks.get(offset as usize)
    }

    /// Return the block with the given hash if it is in the cache.
    #[inline]
    pub fn get_block_by_hash(&self, block_hash: &N::BlockHash) -> Option<&Block<N>> {
        // Perform a linear search through the cache.
        // This is cheap, as the cache is very small.
        self.blocks.iter().find(|block| &block.hash() == block_hash)
    }

    /// Remove the last `n` blocks from the cache.
    #[inline]
    pub fn remove_last_n(&mut self, n: u32) -> Result<()> {
        for _ in 0..n {
            self.blocks.pop_back();
        }
        Ok(())
    }
}
