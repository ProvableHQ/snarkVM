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

use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KaryMerklePath<PH: PathHash, const DEPTH: u8, const ARITY: u8> {
    /// The leaf index for the path.
    leaf_index: u64,
    /// The `siblings` contains a list of sibling hashes from the leaf to the root.
    siblings: Vec<Vec<PH::Hash>>,
}

impl<PH: PathHash, const DEPTH: u8, const ARITY: u8> KaryMerklePath<PH, DEPTH, ARITY> {
    /// Returns a new instance of a Merkle path.
    pub fn try_from((leaf_index, siblings): (u64, Vec<Vec<PH::Hash>>)) -> Result<Self> {
        // Ensure the Merkle tree depth is greater than 0.
        ensure!(DEPTH > 0, "Merkle tree depth must be greater than 0");
        // Ensure the Merkle tree depth is less than or equal to 64.
        ensure!(DEPTH <= 64u8, "Merkle tree depth must be less than or equal to 64");
        // Ensure the Merkle tree arity is greater than 1.
        ensure!(ARITY > 1, "Merkle tree arity must be greater than 1");
        // Ensure the Merkle tree does not overflow a u128.
        ensure!((ARITY as u128).checked_pow(DEPTH as u32).is_some(), "Merkle tree size overflowed");
        // Ensure the leaf index is within the tree depth.
        ensure!((leaf_index as u128) < (ARITY as u128).saturating_pow(DEPTH as u32), "Out of bounds Merkle leaf index");
        // Ensure the Merkle path is the correct length.
        ensure!(siblings.len() == DEPTH as usize, "Found an incorrect Merkle path length");
        for sibling in &siblings {
            // Note: The ARITY is guaranteed to be greater than 1 (by the above check).
            ensure!(sibling.len() == (ARITY - 1) as usize, "Found an incorrect Merkle path arity");
        }
        // Return the Merkle path.
        Ok(Self { leaf_index, siblings })
    }
}

impl<PH: PathHash, const DEPTH: u8, const ARITY: u8> KaryMerklePath<PH, DEPTH, ARITY> {
    /// Returns the leaf index for the path.
    pub fn leaf_index(&self) -> u64 {
        self.leaf_index
    }

    /// Returns the siblings for the path.
    pub fn siblings(&self) -> &[Vec<PH::Hash>] {
        &self.siblings
    }

    /// Returns `true` if the Merkle path is valid for the given root and leaf.
    pub fn verify<LH: LeafHash<Hash = PH::Hash>>(
        &self,
        leaf_hasher: &LH,
        path_hasher: &PH,
        root: &PH::Hash,
        leaf: &LH::Leaf,
    ) -> bool {
        // Ensure the leaf index is within the tree depth.
        if (self.leaf_index as u128) >= (ARITY as u128).saturating_pow(DEPTH as u32) {
            eprintln!("Found an out of bounds Merkle leaf index");
            return false;
        }
        // Ensure the path length matches the expected depth.
        if self.siblings.len() != DEPTH as usize {
            eprintln!("Found an incorrect Merkle path length");
            return false;
        }

        // Initialize a tracker for the current hash, by computing the leaf hash to start.
        let mut current_hash = match leaf_hasher.hash_leaf(leaf) {
            Ok(candidate_leaf_hash) => candidate_leaf_hash,
            Err(error) => {
                eprintln!("Failed to hash the Merkle leaf during verification: {error}");
                return false;
            }
        };

        // Compute the ordering of the current hash and sibling hashes on each level.
        // The indicator index determines which sibling the current hash is.
        let Ok(indicator_indexes) = (0..DEPTH)
            .map(|i| {
                usize::try_from(self.leaf_index as u128 / (ARITY as u128).saturating_pow(i as u32) % (ARITY as u128))
            })
            .collect::<Result<Vec<_>, _>>()
        else {
            eprintln!("Found an incorrect Merkle leaf index");
            return false;
        };

        // Check levels between leaf level and root.
        for (indicator_index, sibling_hashes) in indicator_indexes.into_iter().zip_eq(&self.siblings) {
            // Construct the ordering of sibling hashes for this level.
            let mut sibling_hashes = sibling_hashes.clone();

            // Insert the current hash into the list of sibling hashes.
            sibling_hashes.insert(indicator_index, current_hash);

            // Update the current hash for the next level.
            match path_hasher.hash_children(&sibling_hashes) {
                Ok(hash) => current_hash = hash,
                Err(error) => {
                    eprintln!("Failed to hash the Merkle path during verification: {error}");
                    return false;
                }
            }
        }

        // Ensure the final hash matches the given root.
        current_hash == *root
    }
}

impl<PH: PathHash, const DEPTH: u8, const ARITY: u8> FromBytes for KaryMerklePath<PH, DEPTH, ARITY> {
    /// Reads in a Merkle path from a buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the leaf index.
        let leaf_index = u64::read_le(&mut reader)?;
        // Read the Merkle path siblings.
        let siblings = (0..DEPTH)
            .map(|_| (0..ARITY).map(|_| FromBytes::read_le(&mut reader)).collect::<IoResult<Vec<_>>>())
            .collect::<IoResult<Vec<_>>>()?;
        // Return the Merkle path.
        Self::try_from((leaf_index, siblings)).map_err(error)
    }
}

impl<PH: PathHash, const DEPTH: u8, const ARITY: u8> ToBytes for KaryMerklePath<PH, DEPTH, ARITY> {
    /// Writes the Merkle path to a buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the leaf index.
        self.leaf_index.write_le(&mut writer)?;
        // Write the Merkle path siblings.
        self.siblings
            .iter()
            .try_for_each(|siblings| siblings.iter().try_for_each(|sibling| sibling.write_le(&mut writer)))
    }
}

impl<PH: PathHash, const DEPTH: u8, const ARITY: u8> Serialize for KaryMerklePath<PH, DEPTH, ARITY> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ToBytesSerializer::serialize_with_size_encoding(self, serializer)
    }
}

impl<'de, PH: PathHash, const DEPTH: u8, const ARITY: u8> Deserialize<'de> for KaryMerklePath<PH, DEPTH, ARITY> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "K-ary Merkle path")
    }
}
