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

mod helpers;
pub use helpers::*;

mod path;
pub use path::*;

#[cfg(test)]
mod tests;

use snarkvm_console_types::prelude::*;

use aleo_std::prelude::*;

use std::collections::BTreeMap;
use std::marker::PhantomData;

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

/// A path in the sparse k-ary Merkle tree, represented as a sequence of DEPTH digits (0..ARITY-1).
type TreePath = Vec<u8>;

#[derive(Clone)]
pub struct SparseKaryMerkleTree<E: Environment, PH: PathHash, KH: KeyHash<Hash = PH::Hash>, LH: LeafHash<Hash = PH::Hash>, const DEPTH: u8, const ARITY: u8> {
    /// The path hasher for the Sparse K-ary Merkle tree.
    path_hasher: PH,
    /// The key hasher for the Sparse K-ary Merkle tree.
    key_hasher: KH,
    /// The leaf hasher for the Sparse K-ary Merkle tree.
    leaf_hasher: LH,
    /// The computed root of the Sparse K-ary Merkle tree.
    root: PH::Hash,
    /// The leaf hashes, indexed by their path in the tree.
    /// In a sparse tree, we only store non-empty leaves.
    leaves: BTreeMap<TreePath, PH::Hash>,
    /// The canonical empty hash (used for empty positions).
    empty_hash: PH::Hash,
    /// The key-value pairs stored in the tree.
    entries: BTreeMap<KH::Key, LH::Leaf>,
    /// Whether the tree is sorted by key.
    sorted: bool,
    /// Phantom data to mark E as used.
    _phantom: PhantomData<E>,
}

impl<E: Environment, PH: PathHash, KH: KeyHash<Hash = PH::Hash>, LH: LeafHash<Hash = PH::Hash>, const DEPTH: u8, const ARITY: u8>
    SparseKaryMerkleTree<E, PH, KH, LH, DEPTH, ARITY>
{
    #[inline]
    /// Initializes a new Sparse K-ary Merkle tree.
    pub fn new(path_hasher: &PH, key_hasher: &KH, leaf_hasher: &LH, sorted: bool) -> Result<Self> {
        let timer = timer!("SparseKaryMerkleTree::new");

        // Ensure the Sparse K-ary Merkle tree depth is greater than 0.
        ensure!(DEPTH > 0, "Sparse K-ary Merkle tree depth must be greater than 0");
        // Ensure the Sparse K-ary Merkle tree arity is greater than 1.
        ensure!(ARITY > 1, "Sparse K-ary Merkle tree arity must be greater than 1");
        // Ensure the Sparse K-ary Merkle tree does not overflow.
        ensure!((ARITY as u128).checked_pow(DEPTH as u32).is_some(), "Sparse K-ary Merkle tree size overflowed");

        // Compute the empty hash (hash of ARITY default hashes).
        let empty_hash = path_hasher.hash_empty::<ARITY>()?;

        // Compute the root hash for an empty tree by building from bottom to top.
        // Each level hashes ARITY copies of the previous level's hash.
        let mut root_hash = empty_hash;
        for _ in 0..DEPTH {
            let children = vec![root_hash; ARITY as usize];
            root_hash = path_hasher.hash_children(&children)?;
        }

        finish!(timer);

        Ok(Self {
            path_hasher: path_hasher.clone(),
            key_hasher: key_hasher.clone(),
            leaf_hasher: leaf_hasher.clone(),
            root: root_hash,
            leaves: BTreeMap::new(),
            empty_hash,
            entries: BTreeMap::new(),
            sorted,
            _phantom: PhantomData,
        })
    }

    #[inline]
    /// Initializes a new Sparse K-ary Merkle tree with the given entries.
    pub fn new_with_entries(
        path_hasher: &PH,
        key_hasher: &KH,
        leaf_hasher: &LH,
        entries: &[(KH::Key, LH::Leaf)],
        sorted: bool,
    ) -> Result<Self> {
        let timer = timer!("SparseKaryMerkleTree::new_with_entries");

        // Initialize an empty tree.
        let mut tree = Self::new(path_hasher, key_hasher, leaf_hasher, sorted)?;

        // Insert all entries using the batch insert method.
        if !entries.is_empty() {
            let entries_map: BTreeMap<_, _> = entries.iter().cloned().collect();
            tree.insert_many(&entries_map)?;
        }

        finish!(timer);

        Ok(tree)
    }

    #[inline]
    /// Inserts or updates a key-value pair in the Sparse K-ary Merkle tree.
    pub fn insert(&mut self, key: KH::Key, value: LH::Leaf) -> Result<()> {
        let timer = timer!("SparseKaryMerkleTree::insert");

        // Compute the key hash.
        let key_hash = self.key_hasher.hash_key(&key)?;

        // Compute the path from the key hash.
        let path = self.compute_path(&key_hash)?;

        // Compute the leaf hash.
        let leaf_hash = self.leaf_hasher.hash_leaf(&value)?;

        // Update the entries map.
        self.entries.insert(key, value);

        // Update the leaves with the new leaf hash.
        self.leaves.insert(path, leaf_hash);

        // Recompute the root hash.
        self.recompute_root()?;

        finish!(timer);

        Ok(())
    }

    #[inline]
    /// Inserts or updates multiple key-value pairs in the Sparse K-ary Merkle tree.
    pub fn insert_many(&mut self, entries: &BTreeMap<KH::Key, LH::Leaf>) -> Result<()> {
        let timer = timer!("SparseKaryMerkleTree::insert_many");

        // Check that there are entries to insert.
        ensure!(!entries.is_empty(), "There must be at least one entry to insert in the Sparse K-ary Merkle tree");

        // Process all entries and compute their paths and hashes.
        for (key, value) in entries {
            let key_hash = self.key_hasher.hash_key(key)?;
            let path = self.compute_path(&key_hash)?;
            let leaf_hash = self.leaf_hasher.hash_leaf(value)?;
            
            self.entries.insert(key.clone(), value.clone());
            self.leaves.insert(path, leaf_hash);
        }

        // Recompute the root hash.
        self.recompute_root()?;

        finish!(timer);

        Ok(())
    }

    #[inline]
    /// Updates a key-value pair in the Sparse K-ary Merkle tree.
    pub fn update(&mut self, key: &KH::Key, value: LH::Leaf) -> Result<()> {
        let timer = timer!("SparseKaryMerkleTree::update");

        // Ensure the key exists.
        ensure!(
            self.entries.contains_key(key),
            "Key does not exist in the Sparse K-ary Merkle tree"
        );

        // Compute the key hash.
        let key_hash = self.key_hasher.hash_key(key)?;

        // Compute the path from the key hash.
        let path = self.compute_path(&key_hash)?;

        // Compute the new leaf hash.
        let leaf_hash = self.leaf_hasher.hash_leaf(&value)?;

        // Update the entries map.
        self.entries.insert(key.clone(), value);

        // Update the leaves with the new leaf hash.
        self.leaves.insert(path, leaf_hash);

        // Recompute the root hash.
        self.recompute_root()?;

        finish!(timer);

        Ok(())
    }

    #[inline]
    /// Updates multiple key-value pairs in the Sparse K-ary Merkle tree.
    pub fn update_many(&mut self, updates: &BTreeMap<KH::Key, LH::Leaf>) -> Result<()> {
        let timer = timer!("SparseKaryMerkleTree::update_many");

        // Check that there are updates to apply.
        ensure!(!updates.is_empty(), "There must be at least one update to apply in the Sparse K-ary Merkle tree");

        // Ensure all keys exist.
        for key in updates.keys() {
            ensure!(
                self.entries.contains_key(key),
                "Key does not exist in the Sparse K-ary Merkle tree"
            );
        }

        // Process all updates.
        for (key, value) in updates {
            let key_hash = self.key_hasher.hash_key(key)?;
            let path = self.compute_path(&key_hash)?;
            let leaf_hash = self.leaf_hasher.hash_leaf(value)?;
            
            self.entries.insert(key.clone(), value.clone());
            self.leaves.insert(path, leaf_hash);
        }

        // Recompute the root hash.
        self.recompute_root()?;

        finish!(timer);

        Ok(())
    }

    #[inline]
    /// Removes a key-value pair from the Sparse K-ary Merkle tree.
    pub fn remove(&mut self, key: &KH::Key) -> Result<()> {
        let timer = timer!("SparseKaryMerkleTree::remove");

        // Ensure the key exists.
        ensure!(
            self.entries.contains_key(key),
            "Key does not exist in the Sparse K-ary Merkle tree"
        );

        // Compute the key hash.
        let key_hash = self.key_hasher.hash_key(key)?;

        // Compute the path from the key hash.
        let path = self.compute_path(&key_hash)?;

        // Remove from entries map.
        self.entries.remove(key);

        // Remove from leaves.
        self.leaves.remove(&path);

        // Recompute the root hash.
        self.recompute_root()?;

        finish!(timer);

        Ok(())
    }

    #[inline]
    /// Returns the Merkle path for the given key.
    pub fn prove(&self, key: &KH::Key) -> Result<SparseKaryMerklePath<E, PH, DEPTH, ARITY>> {
        // Ensure the key exists.
        ensure!(
            self.entries.contains_key(key),
            "Key does not exist in the Sparse K-ary Merkle tree"
        );

        // Compute the key hash.
        let key_hash = self.key_hasher.hash_key(key)?;

        // Compute the path from the key hash.
        let path = self.compute_path(&key_hash)?;

        // Initialize a cache for computed subtree hashes to avoid recomputation.
        let mut cache: BTreeMap<Vec<u8>, PH::Hash> = BTreeMap::new();

        // Initialize a vector for the sibling hashes.
        let mut siblings = Vec::with_capacity(DEPTH as usize);

        // For each level, collect the sibling hashes.
        for level in 0..DEPTH as usize {
            let mut sibling_hashes = Vec::with_capacity((ARITY - 1) as usize);
            
            // The digit at this level tells us which child position we're at.
            let my_position = path[level];
            
            // Collect all sibling hashes (all positions except my_position).
            for position in 0..ARITY {
                if position != my_position {
                    // Construct the sibling's path.
                    let mut sibling_path = path[0..=level].to_vec();
                    sibling_path[level] = position;
                    
                    // Get the sibling's subtree hash (with caching).
                    let sibling_hash = self.get_subtree_hash_cached(&sibling_path, &mut cache)?;
                    
                    sibling_hashes.push(sibling_hash);
                }
            }
            
            siblings.push(sibling_hashes);
        }

        // Return the Merkle path.
        SparseKaryMerklePath::try_from((key_hash, siblings))
    }

    /// Returns `true` if the given Merkle path is valid for the given root and key-value pair.
    pub fn verify(&self, path: &SparseKaryMerklePath<E, PH, DEPTH, ARITY>, root: &PH::Hash, key: &KH::Key, value: &LH::Leaf) -> bool {
        path.verify(&self.key_hasher, &self.leaf_hasher, &self.path_hasher, root, key, value)
    }

    /// Returns the Merkle root of the tree.
    pub const fn root(&self) -> &PH::Hash {
        &self.root
    }

    /// Returns the empty hash.
    pub const fn empty_hash(&self) -> &PH::Hash {
        &self.empty_hash
    }

    /// Returns the number of entries in the Sparse K-ary Merkle tree.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the Sparse K-ary Merkle tree is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns `true` if the tree is sorted.
    pub const fn is_sorted(&self) -> bool {
        self.sorted
    }

    /// Returns a reference to the entries map.
    pub fn entries(&self) -> &BTreeMap<KH::Key, LH::Leaf> {
        &self.entries
    }

    /// Recomputes the root hash from the current leaves using a bottom-up approach.
    /// This is much more efficient than recursion for sparse trees.
    fn recompute_root(&mut self) -> Result<()> {
        let timer = timer!("SparseKaryMerkleTree::recompute_root");

        if self.leaves.is_empty() {
            // Empty tree - compute empty root.
            let mut root_hash = self.empty_hash;
            for _ in 0..DEPTH {
                let children = vec![root_hash; ARITY as usize];
                root_hash = self.path_hasher.hash_children(&children)?;
            }
            self.root = root_hash;
            finish!(timer);
            return Ok(());
        }

        // Build up from leaves to root level by level.
        // Current level stores path -> hash mappings.
        let mut current_level: BTreeMap<Vec<u8>, PH::Hash> = self.leaves.clone();

        // Process each level from leaves to root.
        for level in (0..DEPTH as usize).rev() {
            let mut next_level: BTreeMap<Vec<u8>, PH::Hash> = BTreeMap::new();

            // Group current level nodes by their parent path.
            let mut parents: BTreeMap<Vec<u8>, Vec<(u8, PH::Hash)>> = BTreeMap::new();
            
            for (path, hash) in current_level.iter() {
                if path.len() == level + 1 {
                    // This node is at the current level.
                    let parent_path = path[0..level].to_vec();
                    let position = path[level];
                    parents.entry(parent_path).or_default().push((position, *hash));
                }
            }

            // Compute hash for each parent.
            for (parent_path, children) in parents {
                let mut child_hashes = Vec::with_capacity(ARITY as usize);
                
                // Get hash for each child position.
                for position in 0..ARITY {
                    let hash = children
                        .iter()
                        .find(|(pos, _)| *pos == position)
                        .map(|(_, h)| *h)
                        .unwrap_or(self.empty_hash);
                    child_hashes.push(hash);
                }
                
                let parent_hash = self.path_hasher.hash_children(&child_hashes)?;
                next_level.insert(parent_path, parent_hash);
            }

            current_level = next_level;
        }

        // The root is the hash at the empty path.
        self.root = current_level.get(&vec![]).copied().unwrap_or(self.empty_hash);

        finish!(timer);

        Ok(())
    }

    /// Computes the hash of a subtree rooted at the given path prefix with memoization.
    fn get_subtree_hash_cached(&self, path: &[u8], cache: &mut BTreeMap<Vec<u8>, PH::Hash>) -> Result<PH::Hash> {
        // Check cache first.
        if let Some(&cached_hash) = cache.get(path) {
            return Ok(cached_hash);
        }

        let hash = if path.len() == DEPTH as usize {
            // Leaf level.
            self.leaves.get(path).copied().unwrap_or(self.empty_hash)
        } else {
            // Internal node - check if all children are empty first (optimization).
            let all_children_empty = (0..ARITY).all(|pos| {
                let mut child_path = path.to_vec();
                child_path.push(pos);
                !self.has_any_descendant(&child_path)
            });

            if all_children_empty {
                // All children are empty, use cached empty hash for this level.
                let depth_from_here = DEPTH as usize - path.len();
                let mut hash = self.empty_hash;
                for _ in 0..depth_from_here {
                    let children = vec![hash; ARITY as usize];
                    hash = self.path_hasher.hash_children(&children)?;
                }
                hash
            } else {
                // Compute from children.
                let mut child_hashes = Vec::with_capacity(ARITY as usize);
                
                for position in 0..ARITY {
                    let mut child_path = path.to_vec();
                    child_path.push(position);
                    let child_hash = self.get_subtree_hash_cached(&child_path, cache)?;
                    child_hashes.push(child_hash);
                }
                
                self.path_hasher.hash_children(&child_hashes)?
            }
        };

        // Cache the result.
        cache.insert(path.to_vec(), hash);
        Ok(hash)
    }

    /// Checks if there are any leaves that are descendants of the given path.
    fn has_any_descendant(&self, path_prefix: &[u8]) -> bool {
        if path_prefix.len() >= DEPTH as usize {
            return self.leaves.contains_key(path_prefix);
        }

        // Check if any leaf has this path as a prefix.
        self.leaves.keys().any(|leaf_path| {
            leaf_path.len() >= path_prefix.len() && leaf_path[0..path_prefix.len()] == path_prefix[..]
        })
    }

    /// Computes the path (sequence of DEPTH digits) from the key hash.
    fn compute_path(&self, key_hash: &PH::Hash) -> Result<TreePath> {
        // Convert the key hash to bytes and then to bits.
        let bytes = key_hash.to_bytes_le()?;
        let mut bits = Vec::new();
        for byte in bytes {
            for i in 0..8 {
                bits.push((byte >> i) & 1 == 1);
            }
        }
        
        // Extract enough bits to represent DEPTH base-ARITY digits.
        let bits_needed = (DEPTH as f64 * (ARITY as f64).log2()).ceil() as usize;
        let bits_to_use: Vec<_> = bits.into_iter().take(bits_needed).collect();
        
        // Convert bits to a number.
        let mut number = 0u128;
        for (i, &bit) in bits_to_use.iter().enumerate() {
            if bit {
                number += 1u128 << i;
            }
        }
        
        // Extract base-ARITY digits to form the path.
        let mut path = Vec::with_capacity(DEPTH as usize);
        let mut remaining = number;
        
        for _ in 0..DEPTH {
            path.push((remaining % ARITY as u128) as u8);
            remaining /= ARITY as u128;
        }
        
        Ok(path)
    }
}
