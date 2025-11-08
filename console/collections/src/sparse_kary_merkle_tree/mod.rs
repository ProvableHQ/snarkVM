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

#[derive(Clone)]
pub struct SparseKaryMerkleTree<E: Environment, PH: PathHash, KH: KeyHash<Hash = PH::Hash>, LH: LeafHash<Hash = PH::Hash>, const DEPTH: u8, const ARITY: u8> {
    /// The path hasher for the Sparse K-ary Merkle tree.
    path_hasher: PH,
    /// The key hasher for the Sparse K-ary Merkle tree.
    key_hasher: KH,
    /// The leaf hasher for the Sparse K-ary Merkle tree.
    leaf_hasher: LH,
    /// The computed root of the full Sparse K-ary Merkle tree.
    root: PH::Hash,
    /// The internal hashes, stored as a map from node index to hash.
    /// For a sparse tree, we only store non-empty nodes.
    tree: BTreeMap<usize, PH::Hash>,
    /// The canonical empty hash.
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
        // Ensure the Sparse K-ary Merkle tree depth is less than or equal to 64.
        ensure!(DEPTH <= 64u8, "Sparse K-ary Merkle tree depth must be less than or equal to 64");
        // Ensure the Sparse K-ary Merkle tree arity is greater than 1.
        ensure!(ARITY > 1, "Sparse K-ary Merkle tree arity must be greater than 1");
        // Ensure the Sparse K-ary Merkle tree does not overflow a u128.
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
            tree: BTreeMap::new(),
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

        // Insert all entries using batch insert.
        let entries_map: BTreeMap<_, _> = entries.iter().cloned().collect();
        tree.insert_many(&entries_map)?;

        finish!(timer);

        Ok(tree)
    }

    #[inline]
    /// Inserts or updates a key-value pair in the Sparse K-ary Merkle tree.
    pub fn insert(&mut self, key: KH::Key, value: LH::Leaf) -> Result<()> {
        let timer = timer!("SparseKaryMerkleTree::insert");

        // Compute the key hash.
        let key_hash = self.key_hasher.hash_key(&key)?;

        // Compute the leaf index from the key hash.
        let leaf_index = self.compute_leaf_index(&key_hash)?;

        // Compute the leaf hash.
        let leaf_hash = self.leaf_hasher.hash_leaf(&value)?;

        // Update the entries map.
        self.entries.insert(key, value);

        // Update the tree with the new leaf hash.
        self.tree.insert(leaf_index, leaf_hash);

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

        // Hash all keys and compute leaf indices.
        let hash_key = |(key, value): (&KH::Key, &LH::Leaf)| -> Result<(usize, PH::Hash, KH::Key, LH::Leaf)> {
            let key_hash = self.key_hasher.hash_key(key)?;
            let leaf_index = self.compute_leaf_index(&key_hash)?;
            let leaf_hash = self.leaf_hasher.hash_leaf(value)?;
            Ok((leaf_index, leaf_hash, key.clone(), value.clone()))
        };

        // Process entries in parallel if there are many.
        let updates: Vec<(usize, PH::Hash, KH::Key, LH::Leaf)> = match entries.len() {
            0..=100 => entries.iter().map(|entry| hash_key(entry)).collect::<Result<Vec<_>>>()?,
            _ => cfg_iter!(entries).map(|entry| hash_key(entry)).collect::<Result<Vec<_>>>()?,
        };
        lap!(timer, "Hashed {} keys and values", updates.len());

        // Update entries map.
        for (_, _, key, value) in &updates {
            self.entries.insert(key.clone(), value.clone());
        }

        // Update tree with new leaf hashes.
        for (leaf_index, leaf_hash, _, _) in &updates {
            self.tree.insert(*leaf_index, *leaf_hash);
        }

        // Recompute the root hash efficiently for batch updates.
        self.recompute_root_batch(&updates.iter().map(|(idx, _, _, _)| *idx).collect::<Vec<_>>())?;

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

        // Compute the leaf index from the key hash.
        let leaf_index = self.compute_leaf_index(&key_hash)?;

        // Compute the leaf hash.
        let leaf_hash = self.leaf_hasher.hash_leaf(&value)?;

        // Update the entries map.
        self.entries.insert(key.clone(), value);

        // Update the tree with the new leaf hash.
        self.tree.insert(leaf_index, leaf_hash);

        // Recompute the root hash.
        self.recompute_root()?;

        finish!(timer);

        Ok(())
    }

    #[inline]
    /// Updates multiple key-value pairs in the Sparse K-ary Merkle tree.
    pub fn update_many(&mut self, entries: &BTreeMap<KH::Key, LH::Leaf>) -> Result<()> {
        let timer = timer!("SparseKaryMerkleTree::update_many");

        // Check that there are entries to update.
        ensure!(!entries.is_empty(), "There must be at least one entry to update in the Sparse K-ary Merkle tree");

        // Ensure all keys exist.
        for key in entries.keys() {
            ensure!(
                self.entries.contains_key(key),
                "Key does not exist in the Sparse K-ary Merkle tree"
            );
        }

        // Hash all keys and compute leaf indices.
        let hash_key = |(key, value): (&KH::Key, &LH::Leaf)| -> Result<(usize, PH::Hash, KH::Key, LH::Leaf)> {
            let key_hash = self.key_hasher.hash_key(key)?;
            let leaf_index = self.compute_leaf_index(&key_hash)?;
            let leaf_hash = self.leaf_hasher.hash_leaf(value)?;
            Ok((leaf_index, leaf_hash, key.clone(), value.clone()))
        };

        // Process entries in parallel if there are many.
        let updates: Vec<(usize, PH::Hash, KH::Key, LH::Leaf)> = match entries.len() {
            0..=100 => entries.iter().map(|entry| hash_key(entry)).collect::<Result<Vec<_>>>()?,
            _ => cfg_iter!(entries).map(|entry| hash_key(entry)).collect::<Result<Vec<_>>>()?,
        };
        lap!(timer, "Hashed {} keys and values", updates.len());

        // Update entries map.
        for (_, _, key, value) in &updates {
            self.entries.insert(key.clone(), value.clone());
        }

        // Update tree with new leaf hashes.
        for (leaf_index, leaf_hash, _, _) in &updates {
            self.tree.insert(*leaf_index, *leaf_hash);
        }

        // Recompute the root hash efficiently for batch updates.
        self.recompute_root_batch(&updates.iter().map(|(idx, _, _, _)| *idx).collect::<Vec<_>>())?;

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

        // Compute the leaf index from the key hash.
        let leaf_index = self.compute_leaf_index(&key_hash)?;

        // Remove from entries map.
        self.entries.remove(key);

        // Remove from tree.
        self.tree.remove(&leaf_index);

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

        // Compute the leaf index from the key hash.
        let leaf_index = self.compute_leaf_index(&key_hash)?;

        // Get the value.
        let value = self.entries.get(key).unwrap();

        // Compute the leaf hash.
        let leaf_hash = self.leaf_hasher.hash_leaf(value)?;

        // Ensure the leaf hash matches the one in the tree.
        ensure!(
            self.tree.get(&leaf_index).copied() == Some(leaf_hash),
            "The given Sparse K-ary Merkle leaf does not match the one in the tree"
        );

        // Initialize a vector for the Merkle path.
        let mut path = Vec::with_capacity(DEPTH as usize);

        // Iterate from the leaf to the root, storing the sibling hashes along the path.
        let mut current_index = leaf_index;
        for _level in 0..DEPTH {
            // Compute the sibling indices.
            if let Some(siblings) = self.compute_siblings::<ARITY>(current_index) {
                // Get the sibling hashes (or empty hash if not present).
                let sibling_hashes: Vec<PH::Hash> = siblings
                    .map(|idx| self.tree.get(&idx).copied().unwrap_or(self.empty_hash))
                    .collect();

                // Append the sibling hashes to the path.
                path.push(sibling_hashes);

                // Update the current index to the parent.
                if let Some(parent_index) = self.compute_parent::<ARITY>(current_index) {
                    current_index = parent_index;
                } else {
                    break;
                }
            } else {
                // If we're at the root, pad with empty hashes.
                let empty_hashes = vec![self.empty_hash; (ARITY - 1) as usize];
                path.push(empty_hashes);
            }
        }

        // Ensure the path has exactly DEPTH levels.
        if path.len() < DEPTH as usize {
            let empty_hashes = vec![self.empty_hash; (ARITY - 1) as usize];
            path.resize(DEPTH as usize, empty_hashes);
        }

        // Return the Merkle path.
        SparseKaryMerklePath::try_from((key_hash, path))
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

    /// Recomputes the root hash from the current tree state.
    #[inline]
    fn recompute_root(&mut self) -> Result<()> {
        let timer = timer!("SparseKaryMerkleTree::recompute_root");

        // If the tree is empty, compute the empty root.
        if self.tree.is_empty() {
            let mut root_hash = self.empty_hash;
            for _ in 0..DEPTH {
                let children = vec![root_hash; ARITY as usize];
                root_hash = self.path_hasher.hash_children(&children)?;
            }
            self.root = root_hash;
            finish!(timer);
            return Ok(());
        }

        // Compute the root by hashing from leaves to root.
        // Start with all leaf nodes (tree only contains leaves).
        let mut current_level: BTreeMap<usize, PH::Hash> = self.tree.clone();

        // Process each level from leaves to root.
        for _level in 0..DEPTH {
            let mut next_level = BTreeMap::new();

            // Group nodes by their parent index.
            let mut parents: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for &index in current_level.keys() {
                if let Some(parent_idx) = self.compute_parent::<ARITY>(index) {
                    parents.entry(parent_idx).or_default().push(index);
                }
            }

            // Compute hash for each parent.
            for (parent_idx, _child_indices) in parents {
                // Get all ARITY children of this parent.
                let all_children: Vec<usize> = self.compute_child_indexes::<ARITY>(parent_idx).collect();
                let mut child_hashes = Vec::with_capacity(ARITY as usize);
                
                for child_idx in all_children {
                    let hash = current_level.get(&child_idx).copied().unwrap_or(self.empty_hash);
                    child_hashes.push(hash);
                }
                
                let parent_hash = self.path_hasher.hash_children(&child_hashes)?;
                next_level.insert(parent_idx, parent_hash);
            }

            current_level = next_level;
            if current_level.is_empty() {
                break;
            }
        }

        // The root should be at index 0.
        self.root = current_level.get(&0).copied().unwrap_or(self.empty_hash);

        finish!(timer);

        Ok(())
    }

    /// Recomputes the root hash efficiently for batch updates.
    /// Only recomputes paths affected by the updated leaf indices.
    #[inline]
    fn recompute_root_batch(&mut self, updated_indices: &[usize]) -> Result<()> {
        let timer = timer!("SparseKaryMerkleTree::recompute_root_batch");

        // If the tree is empty, compute the empty root.
        if self.tree.is_empty() {
            let mut root_hash = self.empty_hash;
            for _ in 0..DEPTH {
                let children = vec![root_hash; ARITY as usize];
                root_hash = self.path_hasher.hash_children(&children)?;
            }
            self.root = root_hash;
            finish!(timer);
            return Ok(());
        }

        // For batch updates, just call the regular recompute_root.
        // Since tree only contains leaves, we don't need special optimization.
        self.recompute_root()?;

        finish!(timer);

        Ok(())
    }

    /// Computes the leaf index from the key hash using base-ARITY digits.
    #[inline]
    fn compute_leaf_index(&self, key_hash: &PH::Hash) -> Result<usize> {
        // Extract base-ARITY digits from the key hash.
        // We convert the key hash to a number and extract digits.
        let path_digits = self.compute_path_digits(key_hash)?;
        
        // Compute the leaf index using the path digits.
        // For a k-ary tree, the leaf index is computed as:
        // index = d_0 * ARITY^0 + d_1 * ARITY^1 + ... + d_{DEPTH-1} * ARITY^{DEPTH-1}
        let mut index = 0usize;
        let mut arity_power = 1usize;
        
        for &digit in &path_digits {
            index += (digit as usize) * arity_power;
            arity_power = arity_power.checked_mul(ARITY as usize)
                .ok_or_else(|| anyhow!("Integer overflow when computing leaf index"))?;
        }

        // For a sparse k-ary tree, we need to map this to the actual tree structure.
        // The tree uses a different indexing scheme where:
        // - Root is at index 0
        // - Children of node i are at indices: i * ARITY + 1, i * ARITY + 2, ..., i * ARITY + ARITY
        // - Leaves start at a specific offset
        
        // Compute the maximum number of leaves.
        let max_leaves = (ARITY as u128).checked_pow(DEPTH as u32)
            .ok_or_else(|| anyhow!("Integer overflow when computing max leaves"))?;
        
        // Compute the start index for leaves.
        let start = ((max_leaves - 1) / (ARITY as u128 - 1)) as usize;
        
        // The leaf position is at start + index.
        Ok(start + index)
    }

    /// Computes the path digits (base-ARITY) from the key hash.
    #[inline]
    fn compute_path_digits(&self, key_hash: &PH::Hash) -> Result<Vec<u8>>
    {
        // Convert the key hash to a number.
        // We'll use the field value modulo ARITY^DEPTH to get a number in the right range.
        // Note: This assumes PH::Hash is Field<E> which has to_bits_le() method.
        // For now, we'll use a workaround by converting to bytes first.
        let bytes = key_hash.to_bytes_le()?;
        let mut bits = Vec::new();
        for byte in bytes {
            for i in 0..8 {
                bits.push((byte >> i) & 1 == 1);
            }
        }
        
        // Extract enough bits to represent numbers up to ARITY^DEPTH.
        // We need at least log2(ARITY^DEPTH) = DEPTH * log2(ARITY) bits.
        let bits_needed = (DEPTH as f64 * (ARITY as f64).log2()).ceil() as usize;
        let bits_len = bits.len();
        let bits_to_use = bits.into_iter().take(bits_needed.min(bits_len)).collect::<Vec<_>>();
        
        // Convert bits to a number.
        let mut number = 0u128;
        for (i, &bit) in bits_to_use.iter().enumerate() {
            if bit {
                number += 1u128 << i;
            }
        }
        
        // Extract base-ARITY digits.
        let mut digits = Vec::with_capacity(DEPTH as usize);
        let mut remaining = number;
        let arity_u128 = ARITY as u128;
        
        for _ in 0..DEPTH {
            digits.push((remaining % arity_u128) as u8);
            remaining /= arity_u128;
        }
        
        Ok(digits)
    }

    /// Computes the sibling indices for a given node index.
    #[inline]
    fn compute_siblings<const A: u8>(&self, index: usize) -> Option<impl Iterator<Item = usize>> {
        if index == 0 {
            // Root has no siblings.
            None
        } else {
            // Find the left-most sibling.
            let left_most_sibling = ((index - 1) / A as usize) * A as usize + 1;
            
            // Return all siblings except for the given index.
            Some((left_most_sibling..left_most_sibling + A as usize).filter(move |&i| index != i))
        }
    }

    /// Computes the parent index for a given node index.
    #[inline]
    fn compute_parent<const A: u8>(&self, index: usize) -> Option<usize> {
        if index > 0 {
            Some((index - 1) / A as usize)
        } else {
            None
        }
    }

    /// Computes the child index range for a given parent index.
    #[inline]
    fn compute_child_indexes<const A: u8>(&self, parent_index: usize) -> impl Iterator<Item = usize> {
        let start = parent_index * A as usize + 1;
        start..start + A as usize
    }
}
