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
pub struct MerklePath<E: Environment, const DEPTH: u8> {
    /// The leaf index for the path.
    leaf_index: U64<E>,
    /// The `siblings` contains a list of sibling hashes from the leaf to the root.
    siblings: Vec<Field<E>>,
}

impl<E: Environment, const DEPTH: u8> TryFrom<(U64<E>, Vec<Field<E>>)> for MerklePath<E, DEPTH> {
    type Error = Error;

    /// Returns a new instance of a Merkle path.
    fn try_from((leaf_index, siblings): (U64<E>, Vec<Field<E>>)) -> Result<Self> {
        // Ensure the Merkle tree depth is greater than 0.
        ensure!(DEPTH > 0, "Merkle tree depth must be greater than 0");
        // Ensure the Merkle tree depth is less than or equal to 64.
        ensure!(DEPTH <= 64u8, "Merkle tree depth must be less than or equal to 64");
        // Ensure the leaf index is within the tree depth.
        ensure!((*leaf_index as u128) < (1u128 << DEPTH), "Found an out of bounds Merkle leaf index");
        // Ensure the Merkle path is the correct length.
        ensure!(siblings.len() == DEPTH as usize, "Found an incorrect Merkle path length");
        // Return the Merkle path.
        Ok(Self { leaf_index, siblings })
    }
}

impl<E: Environment, const DEPTH: u8> MerklePath<E, DEPTH> {
    /// Returns the leaf index for the path.
    pub fn leaf_index(&self) -> U64<E> {
        self.leaf_index
    }

    /// Returns the siblings for the path.
    pub fn siblings(&self) -> &[Field<E>] {
        &self.siblings
    }

    /// Returns `true` if the Merkle path is valid for the given root and leaf.
    pub fn verify<LH: LeafHash<Hash = PH::Hash>, PH: PathHash<Hash = Field<E>>>(
        &self,
        leaf_hasher: &LH,
        path_hasher: &PH,
        root: &PH::Hash,
        leaf: &LH::Leaf,
    ) -> bool {
        // Ensure the leaf index is within the tree depth.
        if (*self.leaf_index as u128) >= (1u128 << DEPTH) {
            eprintln!("Found an out of bounds Merkle leaf index");
            return false;
        }
        // Ensure the path length matches the expected depth.
        else if self.siblings.len() != DEPTH as usize {
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

        // Compute the ordering of the current hash and sibling hash on each level.
        // If the indicator bit is `true`, then the ordering is (current_hash, sibling_hash).
        // If the indicator bit is `false`, then the ordering is (sibling_hash, current_hash).
        let indicators = (0..DEPTH).map(|i| ((*self.leaf_index >> i) & 1) == 0);

        // Check levels between leaf level and root.
        for (indicator, sibling_hash) in indicators.zip_eq(&self.siblings) {
            // Construct the ordering of the left & right child hash for this level.
            let (left, right) = match indicator {
                true => (current_hash, *sibling_hash),
                false => (*sibling_hash, current_hash),
            };
            // Update the current hash for the next level.
            match path_hasher.hash_children(&left, &right) {
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

impl<E: Environment, const DEPTH: u8> FromBytes for MerklePath<E, DEPTH> {
    /// Reads in a Merkle path from a buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the leaf index.
        let leaf_index = u64::read_le(&mut reader)?;
        // Read the Merkle path siblings.
        let siblings =
            (0..DEPTH).map(|_| Ok(Field::new(FromBytes::read_le(&mut reader)?))).collect::<IoResult<Vec<_>>>()?;
        // Return the Merkle path.
        Self::try_from((U64::new(leaf_index), siblings)).map_err(error)
    }
}

impl<E: Environment, const DEPTH: u8> ToBytes for MerklePath<E, DEPTH> {
    /// Writes the Merkle path to a buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the leaf index.
        self.leaf_index.write_le(&mut writer)?;
        // Write the Merkle path siblings.
        self.siblings.iter().try_for_each(|sibling| sibling.write_le(&mut writer))
    }
}

impl<E: Environment, const DEPTH: u8> Serialize for MerklePath<E, DEPTH> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ToBytesSerializer::serialize(self, serializer)
    }
}

impl<'de, E: Environment, const DEPTH: u8> Deserialize<'de> for MerklePath<E, DEPTH> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Compute the size for: u64 + (Field::SIZE_IN_BYTES * DEPTH).
        let size = 8 + DEPTH as usize * (Field::<E>::size_in_bits() + 7) / 8;
        FromBytesDeserializer::<Self>::deserialize(deserializer, "Merkle path", size)
    }
}
