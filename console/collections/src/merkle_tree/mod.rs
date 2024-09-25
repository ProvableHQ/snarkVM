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

mod helpers;
pub use helpers::*;

mod path;
pub use path::*;

#[cfg(test)]
mod tests;

use snarkvm_console_types::prelude::*;

use aleo_std::prelude::*;

use std::collections::BTreeMap;

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

#[derive(Clone)]
pub struct MerkleTree<E: Environment, LH: LeafHash<Hash = PH::Hash>, PH: PathHash<Hash = Field<E>>, const DEPTH: u8> {
    /// The leaf hasher for the Merkle tree.
    leaf_hasher: LH,
    /// The path hasher for the Merkle tree.
    path_hasher: PH,
    /// The computed root of the full Merkle tree.
    root: PH::Hash,
    /// The internal hashes, from root to hashed leaves, of the full Merkle tree.
    tree: Vec<PH::Hash>,
    /// The canonical empty hash.
    empty_hash: Field<E>,
    /// The number of hashed leaves in the tree.
    number_of_leaves: usize,
}

impl<E: Environment, LH: LeafHash<Hash = PH::Hash>, PH: PathHash<Hash = Field<E>>, const DEPTH: u8>
    MerkleTree<E, LH, PH, DEPTH>
{
    #[inline]
    /// Initializes a new Merkle tree with the given leaves.
    pub fn new(leaf_hasher: &LH, path_hasher: &PH, leaves: &[LH::Leaf]) -> Result<Self> {
        let timer = timer!("MerkleTree::new");

        // Ensure the Merkle tree depth is greater than 0.
        ensure!(DEPTH > 0, "Merkle tree depth must be greater than 0");
        // Ensure the Merkle tree depth is less than or equal to 64.
        ensure!(DEPTH <= 64u8, "Merkle tree depth must be less than or equal to 64");

        // Compute the maximum number of leaves.
        let max_leaves = match leaves.len().checked_next_power_of_two() {
            Some(num_leaves) => num_leaves,
            None => bail!("Integer overflow when computing the maximum number of leaves in the Merkle tree"),
        };

        // Compute the number of nodes.
        let num_nodes = max_leaves - 1;
        // Compute the tree size as the maximum number of leaves plus the number of nodes.
        let tree_size = max_leaves + num_nodes;
        // Compute the number of levels in the Merkle tree (i.e. log2(tree_size)).
        let tree_depth = tree_depth::<DEPTH>(tree_size)?;
        // Compute the number of padded levels.
        let padding_depth = DEPTH - tree_depth;

        // Compute the empty hash.
        let empty_hash = path_hasher.hash_empty()?;

        // Calculate the size of the tree which excludes leafless nodes.
        // The minimum tree size is either a single root node or the calculated number of nodes plus
        // the supplied leaves; if the number of leaves is odd, an empty hash is added for padding.
        let minimum_tree_size =
            std::cmp::max(1, num_nodes + leaves.len() + if leaves.len() > 1 { leaves.len() % 2 } else { 0 });

        // Initialize the Merkle tree.
        let mut tree = vec![empty_hash; minimum_tree_size];

        // Compute and store each leaf hash.
        tree[num_nodes..num_nodes + leaves.len()].copy_from_slice(&leaf_hasher.hash_leaves(leaves)?);
        lap!(timer, "Hashed {} leaves", leaves.len());

        // Compute and store the hashes for each level, iterating from the penultimate level to the root level.
        let mut start_index = num_nodes;
        // Compute the start index of the current level.
        while let Some(start) = parent(start_index) {
            // Compute the end index of the current level.
            let end = left_child(start);
            // Construct the children for each node in the current level; the leaves are padded, which means
            // that there either are 2 children, or there are none, at which point we may stop iterating.
            let tuples = (start..end)
                .take_while(|&i| tree.get(left_child(i)).is_some())
                .map(|i| (tree[left_child(i)], tree[right_child(i)]))
                .collect::<Vec<_>>();
            // Compute and store the hashes for each node in the current level.
            let num_full_nodes = tuples.len();
            tree[start..][..num_full_nodes].copy_from_slice(&path_hasher.hash_all_children(&tuples)?);
            // Use the precomputed empty node hash for every empty node, if there are any.
            if start + num_full_nodes < end {
                let empty_node_hash = path_hasher.hash_children(&empty_hash, &empty_hash)?;
                for node in tree.iter_mut().take(end).skip(start + num_full_nodes) {
                    *node = empty_node_hash;
                }
            }
            // Update the start index for the next level.
            start_index = start;
        }
        lap!(timer, "Hashed {} levels", tree_depth);

        // Compute the root hash, by iterating from the root level up to `DEPTH`.
        let mut root_hash = tree[0];
        for _ in 0..padding_depth {
            // Update the root hash, by hashing the current root hash with the empty hash.
            root_hash = path_hasher.hash_children(&root_hash, &empty_hash)?;
        }
        lap!(timer, "Hashed {} padding levels", padding_depth);

        finish!(timer);

        Ok(Self {
            leaf_hasher: leaf_hasher.clone(),
            path_hasher: path_hasher.clone(),
            root: root_hash,
            tree,
            empty_hash,
            number_of_leaves: leaves.len(),
        })
    }

    #[inline]
    /// Returns a new Merkle tree with the given new leaves appended to it.
    pub fn prepare_append(&self, new_leaves: &[LH::Leaf]) -> Result<Self> {
        let timer = timer!("MerkleTree::prepare_append");

        // Compute the maximum number of leaves.
        let max_leaves = match (self.number_of_leaves + new_leaves.len()).checked_next_power_of_two() {
            Some(num_leaves) => num_leaves,
            None => bail!("Integer overflow when computing the maximum number of leaves in the Merkle tree"),
        };
        // Compute the number of nodes.
        let num_nodes = max_leaves - 1;
        // Compute the tree size as the maximum number of leaves plus the number of nodes.
        let tree_size = num_nodes + max_leaves;
        // Compute the number of levels in the Merkle tree (i.e. log2(tree_size)).
        let tree_depth = tree_depth::<DEPTH>(tree_size)?;
        // Compute the number of padded levels.
        let padding_depth = DEPTH - tree_depth;

        // Initialize the Merkle tree.
        let mut tree = vec![self.empty_hash; num_nodes];
        // Extend the new Merkle tree with the existing leaf hashes.
        tree.extend(self.leaf_hashes()?);
        // Extend the new Merkle tree with the new leaf hashes.
        tree.extend(&self.leaf_hasher.hash_leaves(new_leaves)?);

        // Calculate the size of the tree which excludes leafless nodes.
        let new_number_of_leaves = self.number_of_leaves + new_leaves.len();
        let minimum_tree_size = std::cmp::max(
            1,
            num_nodes + new_number_of_leaves + if new_number_of_leaves > 1 { new_number_of_leaves % 2 } else { 0 },
        );

        // Resize the new Merkle tree with empty hashes to pad up to `tree_size`.
        tree.resize(minimum_tree_size, self.empty_hash);
        lap!(timer, "Hashed {} new leaves", new_leaves.len());

        // Initialize a start index to track the starting index of the current level.
        let start_index = num_nodes;
        // Initialize a middle index to separate the precomputed indices from the new indices that need to be computed.
        let middle_index = num_nodes + self.number_of_leaves;
        // Initialize a precompute index to track the starting index of each precomputed level.
        let start_precompute_index = match self.number_of_leaves.checked_next_power_of_two() {
            Some(num_leaves) => num_leaves - 1,
            None => bail!("Integer overflow when computing the Merkle tree precompute index"),
        };
        // Initialize a precompute index to track the middle index of each precomputed level.
        let middle_precompute_index = match num_nodes == start_precompute_index {
            // If the old tree and new tree are of the same size, then we can copy over the right half of the old tree.
            true => Some(start_precompute_index + self.number_of_leaves + new_leaves.len() + 1),
            // Otherwise, we need to compute the right half of the new tree.
            false => None,
        };

        // Compute and store the hashes for each level, iterating from the penultimate level to the root level.
        self.compute_updated_tree(
            &mut tree,
            start_index,
            middle_index,
            start_precompute_index,
            middle_precompute_index,
        )?;

        // Compute the root hash, by iterating from the root level up to `DEPTH`.
        let mut root_hash = tree[0];
        for _ in 0..padding_depth {
            // Update the root hash, by hashing the current root hash with the empty hash.
            root_hash = self.path_hasher.hash_children(&root_hash, &self.empty_hash)?;
        }
        lap!(timer, "Hashed {} padding levels", padding_depth);

        finish!(timer);

        Ok(Self {
            leaf_hasher: self.leaf_hasher.clone(),
            path_hasher: self.path_hasher.clone(),
            root: root_hash,
            tree,
            empty_hash: self.empty_hash,
            number_of_leaves: self.number_of_leaves + new_leaves.len(),
        })
    }

    #[inline]
    /// Updates the Merkle tree with the given new leaves appended to it.
    pub fn append(&mut self, new_leaves: &[LH::Leaf]) -> Result<()> {
        let timer = timer!("MerkleTree::append");

        // Compute the updated Merkle tree with the new leaves.
        let updated_tree = self.prepare_append(new_leaves)?;
        // Update the tree at the very end, so the original tree is not altered in case of failure.
        *self = updated_tree;

        finish!(timer);
        Ok(())
    }

    #[inline]
    /// Updates the Merkle tree at the location of the given leaf index with the new leaf.
    pub fn update(&mut self, leaf_index: usize, new_leaf: &LH::Leaf) -> Result<()> {
        let timer = timer!("MerkleTree::update");

        // Compute the updated Merkle tree with the new leaves.
        let updated_tree = self.prepare_update(leaf_index, new_leaf)?;
        // Update the tree at the very end, so the original tree is not altered in case of failure.
        *self = updated_tree;

        finish!(timer);
        Ok(())
    }

    #[inline]
    /// Returns a new Merkle tree with updates at the location of the given leaf index with the new leaf.
    pub fn prepare_update(&self, leaf_index: usize, new_leaf: &LH::Leaf) -> Result<Self> {
        let timer = timer!("MerkleTree::prepare_update");

        // Check that the leaf index is within the bounds of the Merkle tree.
        ensure!(
            leaf_index < self.number_of_leaves,
            "Leaf index must be less than the number of leaves in the Merkle tree {leaf_index} , {}",
            self.number_of_leaves
        );

        // Allocate a vector to store the path hashes.
        let mut path_hashes = Vec::with_capacity(DEPTH as usize);

        // Compute and add the new leaf hash to the path hashes.
        path_hashes.push(self.leaf_hasher.hash_leaf(new_leaf)?);
        lap!(timer, "Hashed 1 new leaf");

        // Compute the start index (on the left) for the leaf hashes level in the Merkle tree.
        let start = match self.number_of_leaves.checked_next_power_of_two() {
            Some(num_leaves) => num_leaves - 1,
            None => bail!("Integer overflow when computing the Merkle tree start index"),
        };

        // Compute the new hashes for the path from the leaf to the root.
        let mut index = start + leaf_index;
        while let Some(parent) = parent(index) {
            // Get the left and right child hashes of the parent.
            let (left, right) = match is_left_child(index) {
                true => (path_hashes.last().unwrap(), &self.tree[right_child(parent)]),
                false => (&self.tree[left_child(parent)], path_hashes.last().unwrap()),
            };
            // Compute and add the new parent hash to the path hashes.
            path_hashes.push(self.path_hasher.hash_children(left, right)?);
            // Update the index to the parent.
            index = parent;
        }

        // Compute the number of levels in the Merkle tree (i.e. log2(tree_size)).
        let tree_depth = tree_depth::<DEPTH>(self.tree.len())?;
        // Compute the padding depth.
        let padding_depth = DEPTH - tree_depth;

        // Update the root hash.
        // This unwrap is safe, as the path hashes vector is guaranteed to have at least one element.
        let mut root_hash = *path_hashes.last().unwrap();
        for _ in 0..padding_depth {
            // Update the root hash, by hashing the current root hash with the empty hash.
            root_hash = self.path_hasher.hash_children(&root_hash, &self.empty_hash)?;
        }
        lap!(timer, "Hashed {} padding levels", padding_depth);

        // Initialize the Merkle tree.
        let mut tree = Vec::with_capacity(self.tree.len());
        // Extend the new Merkle tree with the existing leaf hashes.
        tree.extend(&self.tree);

        // Update the rest of the tree with the new path hashes.
        let mut index = Some(start + leaf_index);
        for path_hash in path_hashes {
            tree[index.unwrap()] = path_hash;
            index = parent(index.unwrap());
        }

        finish!(timer);

        Ok(Self {
            leaf_hasher: self.leaf_hasher.clone(),
            path_hasher: self.path_hasher.clone(),
            root: root_hash,
            tree,
            empty_hash: self.empty_hash,
            number_of_leaves: self.number_of_leaves,
        })
    }

    #[inline]
    /// Updates the Merkle tree at the location of the given leaf indices with the new leaves.
    pub fn update_many(&mut self, updates: &BTreeMap<usize, LH::Leaf>) -> Result<()> {
        let timer = timer!("MerkleTree::update_many");

        // Check that there are updates to perform.
        ensure!(!updates.is_empty(), "There must be at least one leaf to update in the Merkle tree");

        // Check that the latest leaf index is less than number of leaves in the Merkle tree.
        // Note: This unwrap is safe since updates is guaranteed to be non-empty.
        ensure!(
            *updates.last_key_value().unwrap().0 < self.number_of_leaves,
            "Leaf index must be less than the number of leaves in the Merkle tree"
        );

        // Compute the start index (on the left) for the leaf hashes level in the Merkle tree.
        let start = match self.number_of_leaves.checked_next_power_of_two() {
            Some(num_leaves) => num_leaves - 1,
            None => bail!("Integer overflow when computing the Merkle tree start index"),
        };

        // A helper to compute the leaf hash.
        let hash_update = |(leaf_index, leaf): &(&usize, &LH::Leaf)| {
            self.leaf_hasher.hash_leaf(leaf).map(|hash| (start + **leaf_index, hash))
        };

        // Hash the leaves and add them to the updated hashes.
        let leaf_hashes: Vec<(usize, LH::Hash)> = match updates.len() {
            0..=100 => updates.iter().map(|update| hash_update(&update)).collect::<Result<Vec<_>>>()?,
            _ => cfg_iter!(updates).map(|update| hash_update(&update)).collect::<Result<Vec<_>>>()?,
        };
        lap!(timer, "Hashed {} new leaves", leaf_hashes.len());

        // Store the updated hashes by level.
        let mut updated_hashes = Vec::new();
        updated_hashes.push(leaf_hashes);

        // A helper function to compute the path hashes for a given level.
        type Update<PH> = (usize, (<PH as PathHash>::Hash, <PH as PathHash>::Hash));
        let compute_path_hashes = |inputs: &[Update<PH>]| match inputs.len() {
            0..=100 => inputs
                .iter()
                .map(|(index, (left, right))| self.path_hasher.hash_children(left, right).map(|hash| (*index, hash)))
                .collect::<Result<Vec<_>>>(),
            _ => cfg_iter!(inputs)
                .map(|(index, (left, right))| self.path_hasher.hash_children(left, right).map(|hash| (*index, hash)))
                .collect::<Result<Vec<_>>>(),
        };

        // Compute the depth of the tree. This corresponds to the number of levels of hashes in the tree.
        let tree_depth = tree_depth::<DEPTH>(self.tree.len())?;
        // Allocate a vector to store the inputs to the path hasher.
        let mut inputs = Vec::with_capacity(updated_hashes[0].len());
        // For each level in the tree, compute the path hashes.
        // In the first iteration, we compute the path hashes for the updated leaf hashes.
        // In the subsequent iterations, we compute the path hashes for the updated path hashes, until we reach the root.
        for level in 0..tree_depth as usize {
            let mut current = 0;
            while current < updated_hashes[level].len() {
                let (current_leaf_index, current_leaf_hash) = updated_hashes[level][current];
                // Get the sibling of the current leaf.
                let sibling_leaf_index = match sibling(current_leaf_index) {
                    Some(sibling_index) => sibling_index,
                    // If there is no sibling, then we have reached the root.
                    None => break,
                };
                // Check if the sibling hash is the next hash in the vector.
                let sibling_is_next_hash = match current + 1 < updated_hashes[level].len() {
                    true => updated_hashes[level][current + 1].0 == sibling_leaf_index,
                    false => false,
                };
                // Get the sibling hash.
                // Note: This algorithm assumes that the sibling hash is either the next hash in the vector,
                // or in the original Merkle tree. Consequently, updates need to be provided in sequential order.
                // This is enforced by the type of `updates: `BTreeMap<usize, LH::Leaf>`.
                // If this assumption is violated, then the algorithm will compute incorrect path hashes in the Merkle tree.
                let sibling_leaf_hash = match sibling_is_next_hash {
                    true => updated_hashes[level][current + 1].1,
                    false => self.tree[sibling_leaf_index],
                };
                // Order the current and sibling hashes.
                let (left, right) = match is_left_child(current_leaf_index) {
                    true => (current_leaf_hash, sibling_leaf_hash),
                    false => (sibling_leaf_hash, current_leaf_hash),
                };
                // Compute the parent index.
                // Note that this unwrap is safe, since we check that the `current_leaf_index` is not the root.
                let parent_index = parent(current_leaf_index).unwrap();
                // Add the parent hash to the updated hashes.
                inputs.push((parent_index, (left, right)));
                // Update the current index.
                match sibling_is_next_hash {
                    true => current += 2,
                    false => current += 1,
                }
            }
            // Compute the path hashes for the current level.
            let path_hashes = compute_path_hashes(&inputs)?;
            // Add the path hashes to the updated hashes.
            updated_hashes.push(path_hashes);
            // Clear the inputs.
            inputs.clear();
        }

        // Compute the padding depth.
        let padding_depth = DEPTH - tree_depth;

        // Update the root hash.
        // This unwrap is safe, as the updated hashes is guaranteed to have at least one element.
        let mut root_hash = updated_hashes.last().unwrap()[0].1;
        for _ in 0..padding_depth {
            // Update the root hash, by hashing the current root hash with the empty hash.
            root_hash = self.path_hasher.hash_children(&root_hash, &self.empty_hash)?;
        }
        lap!(timer, "Hashed {} padding levels", padding_depth);

        // Update the root hash.
        self.root = root_hash;

        // Update the rest of the tree with the updated hashes.
        for (index, hash) in updated_hashes.into_iter().flatten() {
            self.tree[index] = hash;
        }

        finish!(timer);
        Ok(())
    }

    #[inline]
    /// Returns a new Merkle tree with the last 'n' leaves removed from it.
    pub fn prepare_remove_last_n(&self, n: usize) -> Result<Self> {
        let timer = timer!("MerkleTree::prepare_remove_last_n");

        ensure!(n > 0, "Cannot remove zero leaves from the Merkle tree");

        // Determine the updated number of leaves, after removing the last 'n' leaves.
        let updated_number_of_leaves = self.number_of_leaves.checked_sub(n).ok_or_else(|| {
            anyhow!("Failed to remove '{n}' leaves from the Merkle tree, as it only contains {}", self.number_of_leaves)
        })?;

        // Compute the maximum number of leaves.
        let max_leaves = match (updated_number_of_leaves).checked_next_power_of_two() {
            Some(num_leaves) => num_leaves,
            None => bail!("Integer overflow when computing the maximum number of leaves in the Merkle tree"),
        };
        // Compute the number of nodes.
        let num_nodes = max_leaves - 1;
        // Compute the tree size as the maximum number of leaves plus the number of nodes.
        let tree_size = num_nodes + max_leaves;
        // Compute the number of levels in the Merkle tree (i.e. log2(tree_size)).
        let tree_depth = tree_depth::<DEPTH>(tree_size)?;
        // Compute the number of padded levels.
        let padding_depth = DEPTH - tree_depth;

        // Calculate the size of the tree which excludes leafless nodes.
        let minimum_tree_size = std::cmp::max(
            1,
            num_nodes
                + updated_number_of_leaves
                + if updated_number_of_leaves > 1 { updated_number_of_leaves % 2 } else { 0 },
        );

        // Initialize the Merkle tree.
        let mut tree = vec![self.empty_hash; num_nodes];
        // Extend the new Merkle tree with the existing leaf hashes, excluding the last 'n' leaves.
        tree.extend(&self.leaf_hashes()?[..updated_number_of_leaves]);
        // Resize the new Merkle tree with empty hashes to pad up to `tree_size`.
        tree.resize(minimum_tree_size, self.empty_hash);
        lap!(timer, "Resizing to {} leaves", updated_number_of_leaves);

        // Initialize a start index to track the starting index of the current level.
        let start_index = num_nodes;
        // Initialize a middle index to separate the precomputed indices from the new indices that need to be computed.
        let middle_index = num_nodes + updated_number_of_leaves;
        // Initialize a precompute index to track the starting index of each precomputed level.
        let start_precompute_index = match self.number_of_leaves.checked_next_power_of_two() {
            Some(num_leaves) => num_leaves - 1,
            None => bail!("Integer overflow when computing the Merkle tree precompute index"),
        };
        // Initialize a precompute index to track the middle index of each precomputed level.
        let middle_precompute_index = match num_nodes == start_precompute_index {
            // If the old tree and new tree are of the same size, then we can copy over the right half of the old tree.
            true => Some(start_precompute_index + self.number_of_leaves + 1),
            // true => None,
            // Otherwise, do nothing, since shrinking the tree is already free.
            false => None,
        };

        // Compute and store the hashes for each level, iterating from the penultimate level to the root level.
        self.compute_updated_tree(
            &mut tree,
            start_index,
            middle_index,
            start_precompute_index,
            middle_precompute_index,
        )?;

        // Compute the root hash, by iterating from the root level up to `DEPTH`.
        let mut root_hash = tree[0];
        for _ in 0..padding_depth {
            // Update the root hash, by hashing the current root hash with the empty hash.
            root_hash = self.path_hasher.hash_children(&root_hash, &self.empty_hash)?;
        }
        lap!(timer, "Hashed {} padding levels", padding_depth);

        finish!(timer);

        Ok(Self {
            leaf_hasher: self.leaf_hasher.clone(),
            path_hasher: self.path_hasher.clone(),
            root: root_hash,
            tree,
            empty_hash: self.empty_hash,
            number_of_leaves: updated_number_of_leaves,
        })
    }

    #[inline]
    /// Updates the Merkle tree with the last 'n' leaves removed from it.
    pub fn remove_last_n(&mut self, n: usize) -> Result<()> {
        let timer = timer!("MerkleTree::remove_last_n");

        // Compute the updated Merkle tree with the last 'n' leaves removed.
        let updated_tree = self.prepare_remove_last_n(n)?;
        // Update the tree at the very end, so the original tree is not altered in case of failure.
        *self = updated_tree;

        finish!(timer);
        Ok(())
    }

    #[inline]
    /// Returns the Merkle path for the given leaf index and leaf.
    pub fn prove(&self, leaf_index: usize, leaf: &LH::Leaf) -> Result<MerklePath<E, DEPTH>> {
        // Ensure the leaf index is valid.
        ensure!(leaf_index < self.number_of_leaves, "The given Merkle leaf index is out of bounds");

        // Compute the leaf hash.
        let leaf_hash = self.leaf_hasher.hash_leaf(leaf)?;

        // Compute the start index (on the left) for the leaf hashes level in the Merkle tree.
        let start = match self.number_of_leaves.checked_next_power_of_two() {
            Some(num_leaves) => num_leaves - 1,
            None => bail!("Integer overflow when computing the Merkle tree start index"),
        };

        // Compute the absolute index of the leaf in the Merkle tree.
        let mut index = start + leaf_index;
        // Ensure the leaf index is valid.
        ensure!(index < self.tree.len(), "The given Merkle leaf index is out of bounds");
        // Ensure the leaf hash matches the one in the tree.
        ensure!(self.tree[index] == leaf_hash, "The given Merkle leaf does not match the one in the Merkle tree");

        // Initialize a vector for the Merkle path.
        let mut path = Vec::with_capacity(DEPTH as usize);

        // Iterate from the leaf hash to the root level, storing the sibling hashes along the path.
        for _ in 0..DEPTH {
            // Compute the index of the sibling hash, if it exists.
            if let Some(sibling) = sibling(index) {
                // Append the sibling hash to the path.
                path.push(self.tree[sibling]);
                // Compute the index of the parent hash, if it exists.
                match parent(index) {
                    // Update the index to the parent index.
                    Some(parent) => index = parent,
                    // If the parent does not exist, the path is complete.
                    None => break,
                }
            }
        }

        // If the Merkle path length is not equal to `DEPTH`, pad the path with the empty hash.
        path.resize(DEPTH as usize, self.empty_hash);

        // Return the Merkle path.
        MerklePath::try_from((U64::new(leaf_index as u64), path))
    }

    /// Returns `true` if the given Merkle path is valid for the given root and leaf.
    pub fn verify(&self, path: &MerklePath<E, DEPTH>, root: &PH::Hash, leaf: &LH::Leaf) -> bool {
        path.verify(&self.leaf_hasher, &self.path_hasher, root, leaf)
    }

    /// Returns the Merkle root of the tree.
    pub const fn root(&self) -> &PH::Hash {
        &self.root
    }

    /// Returns the Merkle tree (excluding the hashes of the leaves).
    pub fn tree(&self) -> &[PH::Hash] {
        &self.tree
    }

    /// Returns the empty hash.
    pub const fn empty_hash(&self) -> &PH::Hash {
        &self.empty_hash
    }

    /// Returns the leaf hashes from the Merkle tree.
    pub fn leaf_hashes(&self) -> Result<&[LH::Hash]> {
        // Compute the start index (on the left) for the leaf hashes level in the Merkle tree.
        let start = match self.number_of_leaves.checked_next_power_of_two() {
            Some(num_leaves) => num_leaves - 1,
            None => bail!("Integer overflow when computing the Merkle tree start index"),
        };
        // Compute the end index (on the right) for the leaf hashes level in the Merkle tree.
        let end = start + self.number_of_leaves;
        // Return the leaf hashes.
        Ok(&self.tree[start..end])
    }

    /// Returns the number of leaves in the Merkle tree.
    pub const fn number_of_leaves(&self) -> usize {
        self.number_of_leaves
    }

    /// Compute and store the hashes for each level, iterating from the penultimate level to the root level.
    ///
    /// ```ignore
    ///  start_index      middle_index                              end_index
    ///  start_precompute_index         middle_precompute_index     end_index
    /// ```
    #[inline]
    fn compute_updated_tree(
        &self,
        tree: &mut [Field<E>],
        mut start_index: usize,
        mut middle_index: usize,
        mut start_precompute_index: usize,
        mut middle_precompute_index: Option<usize>,
    ) -> Result<()> {
        // Initialize a timer for the while loop.
        let timer = timer!("MerkleTree::compute_updated_tree");

        // Compute and store the hashes for each level, iterating from the penultimate level to the root level.
        let empty_hash = self.path_hasher.hash_empty()?;
        while let (Some(start), Some(middle)) = (parent(start_index), parent(middle_index)) {
            // Compute the end index of the current level.
            let end = left_child(start);

            // If the current level has precomputed indices, copy them instead of recomputing them.
            if let Some(start_precompute) = parent(start_precompute_index) {
                // Compute the end index of the precomputed level.
                let end_precompute = start_precompute + (middle - start);
                // Copy the hashes for each node in the current level.
                tree[start..middle].copy_from_slice(&self.tree[start_precompute..end_precompute]);
                // Update the precompute index for the next level.
                start_precompute_index = start_precompute;
            } else {
                // Ensure the start index is equal to the middle index, as all precomputed indices have been processed.
                ensure!(start == middle, "Failed to process all left precomputed indices in the Merkle tree");
            }
            lap!(timer, "Precompute (Left): {start} -> {middle}");

            // If the current level has precomputed indices, copy them instead of recomputing them.
            // Note: This logic works because the old tree and new tree are the same power of two.
            if let Some(middle_precompute) = middle_precompute_index {
                if let Some(middle_precompute) = parent(middle_precompute) {
                    // Construct the children for the new indices in the current level.
                    let tuples = (middle..middle_precompute)
                        .map(|i| {
                            (
                                tree.get(left_child(i)).copied().unwrap_or(empty_hash),
                                tree.get(right_child(i)).copied().unwrap_or(empty_hash),
                            )
                        })
                        .collect::<Vec<_>>();
                    // Process the indices that need to be computed for the current level.
                    // If any level requires computing more than 100 nodes, borrow the tree for performance.
                    match tuples.len() >= 100 {
                        // Option 1: Borrow the tree to compute and store the hashes for the new indices in the current level.
                        true => cfg_iter_mut!(tree[middle..middle_precompute]).zip_eq(cfg_iter!(tuples)).try_for_each(
                            |(node, (left, right))| {
                                *node = self.path_hasher.hash_children(left, right)?;
                                Ok::<_, Error>(())
                            },
                        )?,
                        // Option 2: Compute and store the hashes for the new indices in the current level.
                        false => tree[middle..middle_precompute].iter_mut().zip_eq(&tuples).try_for_each(
                            |(node, (left, right))| {
                                *node = self.path_hasher.hash_children(left, right)?;
                                Ok::<_, Error>(())
                            },
                        )?,
                    }
                    lap!(timer, "Compute: {middle} -> {middle_precompute}");

                    // Copy the hashes for each node in the current level.
                    tree[middle_precompute..end].copy_from_slice(&self.tree[middle_precompute..end]);
                    // Update the precompute index for the next level.
                    middle_precompute_index = Some(middle_precompute + 1);
                    lap!(timer, "Precompute (Right): {middle_precompute} -> {end}");
                } else {
                    // Ensure the middle precompute index is equal to the end index, as all precomputed indices have been processed.
                    ensure!(
                        middle_precompute == end,
                        "Failed to process all right precomputed indices in the Merkle tree"
                    );
                }
            } else {
                // Construct the children for the new indices in the current level.
                let tuples = (middle..end)
                    .map(|i| {
                        (
                            tree.get(left_child(i)).copied().unwrap_or(empty_hash),
                            tree.get(right_child(i)).copied().unwrap_or(empty_hash),
                        )
                    })
                    .collect::<Vec<_>>();
                // Process the indices that need to be computed for the current level.
                // If any level requires computing more than 100 nodes, borrow the tree for performance.
                match tuples.len() >= 100 {
                    // Option 1: Borrow the tree to compute and store the hashes for the new indices in the current level.
                    true => cfg_iter_mut!(tree[middle..end]).zip_eq(cfg_iter!(tuples)).try_for_each(
                        |(node, (left, right))| {
                            *node = self.path_hasher.hash_children(left, right)?;
                            Ok::<_, Error>(())
                        },
                    )?,
                    // Option 2: Compute and store the hashes for the new indices in the current level.
                    false => tree[middle..end].iter_mut().zip_eq(&tuples).try_for_each(|(node, (left, right))| {
                        *node = self.path_hasher.hash_children(left, right)?;
                        Ok::<_, Error>(())
                    })?,
                }
                lap!(timer, "Compute: {middle} -> {end}");
            }

            // Update the start index for the next level.
            start_index = start;
            // Update the middle index for the next level.
            middle_index = middle;
        }

        // End the timer for the while loop.
        finish!(timer);

        Ok(())
    }
}

/// Returns the depth of the tree, given the size of the tree.
#[inline]
fn tree_depth<const DEPTH: u8>(tree_size: usize) -> Result<u8> {
    let tree_size = u64::try_from(tree_size)?;
    // Since we only allow tree sizes up to u64::MAX, the maximum possible depth is 63.
    let tree_depth = u8::try_from(tree_size.checked_ilog2().unwrap_or(0))?;
    // Ensure the tree depth is within the depth bound.
    match tree_depth <= DEPTH {
        // Return the tree depth.
        true => Ok(tree_depth),
        false => bail!("Merkle tree cannot exceed depth {DEPTH}: attempted to reach depth {tree_depth}"),
    }
}

/// Returns the index of the left child, given an index.
#[inline]
const fn left_child(index: usize) -> usize {
    2 * index + 1
}

/// Returns the index of the right child, given an index.
#[inline]
const fn right_child(index: usize) -> usize {
    2 * index + 2
}

/// Returns the index of the sibling, given an index.
#[inline]
const fn sibling(index: usize) -> Option<usize> {
    if is_root(index) {
        None
    } else if is_left_child(index) {
        Some(index + 1)
    } else {
        Some(index - 1)
    }
}

/// Returns true iff the index represents the root.
#[inline]
const fn is_root(index: usize) -> bool {
    index == 0
}

/// Returns true iff the given index represents a left child.
#[inline]
const fn is_left_child(index: usize) -> bool {
    index % 2 == 1
}

/// Returns the index of the parent, given the index of a child.
#[inline]
const fn parent(index: usize) -> Option<usize> {
    if index > 0 { Some((index - 1) >> 1) } else { None }
}
