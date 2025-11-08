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

use super::*;
use std::marker::PhantomData;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SparseKaryMerklePath<E: Environment, PH: PathHash, const DEPTH: u8, const ARITY: u8> {
    /// The key hash for the path.
    key_hash: PH::Hash,
    /// The `siblings` contains a list of sibling hashes from the leaf to the root.
    /// Each level has ARITY-1 siblings.
    siblings: Vec<Vec<PH::Hash>>,
    /// Phantom data to mark E as used.
    _phantom: PhantomData<E>,
}

impl<E: Environment, PH: PathHash, const DEPTH: u8, const ARITY: u8> TryFrom<(PH::Hash, Vec<Vec<PH::Hash>>)> for SparseKaryMerklePath<E, PH, DEPTH, ARITY> {
    type Error = Error;

    /// Returns a new instance of a Sparse K-ary Merkle path.
    fn try_from((key_hash, siblings): (PH::Hash, Vec<Vec<PH::Hash>>)) -> Result<Self> {
        // Ensure the Sparse K-ary Merkle tree depth is greater than 0.
        ensure!(DEPTH > 0, "Sparse K-ary Merkle tree depth must be greater than 0");
        // Ensure the Sparse K-ary Merkle tree depth is less than or equal to 64.
        ensure!(DEPTH <= 64u8, "Sparse K-ary Merkle tree depth must be less than or equal to 64");
        // Ensure the Sparse K-ary Merkle tree arity is greater than 1.
        ensure!(ARITY > 1, "Sparse K-ary Merkle tree arity must be greater than 1");
        // Ensure the Merkle path is the correct length.
        ensure!(siblings.len() == DEPTH as usize, "Found an incorrect Sparse K-ary Merkle path length");
        // Ensure each level has the correct number of siblings.
        for sibling in &siblings {
            ensure!(sibling.len() == (ARITY - 1) as usize, "Found an incorrect Sparse K-ary Merkle path arity");
        }
        // Return the Merkle path.
        Ok(Self { key_hash, siblings, _phantom: PhantomData })
    }
}

impl<E: Environment, PH: PathHash, const DEPTH: u8, const ARITY: u8> SparseKaryMerklePath<E, PH, DEPTH, ARITY> {
    /// Returns the key hash for the path.
    pub fn key_hash(&self) -> PH::Hash {
        self.key_hash
    }

    /// Returns the siblings for the path.
    pub fn siblings(&self) -> &[Vec<PH::Hash>] {
        &self.siblings
    }

    /// Returns `true` if the Merkle path is valid for the given root and key-value pair.
    pub fn verify<KH: KeyHash<Hash = PH::Hash>, LH: LeafHash<Hash = PH::Hash>>(
        &self,
        key_hasher: &KH,
        leaf_hasher: &LH,
        path_hasher: &PH,
        root: &PH::Hash,
        key: &KH::Key,
        value: &LH::Leaf,
    ) -> bool {
        // Compute the key hash.
        let computed_key_hash = match key_hasher.hash_key(key) {
            Ok(hash) => hash,
            Err(_) => return false,
        };

        // Ensure the key hash matches.
        if computed_key_hash != self.key_hash {
            return false;
        }

        // Compute the leaf hash.
        let leaf_hash = match leaf_hasher.hash_leaf(value) {
            Ok(hash) => hash,
            Err(_) => return false,
        };

        // Extract base-ARITY digits from the key hash.
        // Note: This assumes PH::Hash is Field<E> which has to_bits_le() method.
        // For now, we'll use a workaround by converting to bytes first.
        let bits = match self.key_hash.to_bytes_le() {
            Ok(bytes) => {
                let mut bits = Vec::new();
                for byte in bytes {
                    for i in 0..8 {
                        bits.push((byte >> i) & 1 == 1);
                    }
                }
                bits
            }
            Err(_) => return false,
        };
        let bits_needed = (DEPTH as f64 * (ARITY as f64).log2()).ceil() as usize;
        let bits_len = bits.len();
        let bits_to_use: Vec<bool> = bits.into_iter().take(bits_needed.min(bits_len)).collect();
        
        // Convert bits to a number.
        let mut number = 0u128;
        for (i, &bit) in bits_to_use.iter().enumerate() {
            if bit {
                number += 1u128 << i;
            }
        }
        
        // Extract base-ARITY digits.
        let mut path_digits = Vec::with_capacity(DEPTH as usize);
        let mut remaining = number;
        let arity_u128 = ARITY as u128;
        
        for _ in 0..DEPTH {
            path_digits.push((remaining % arity_u128) as u8);
            remaining /= arity_u128;
        }

        // Initialize a tracker for the current hash, starting with the leaf hash.
        let mut current_hash = leaf_hash;

        // Check levels between leaf level and root.
        for (level, sibling_hashes) in self.siblings.iter().enumerate() {
            // Get the path digit for this level (which child position we are).
            let indicator_index = path_digits[level] as usize;

            // Construct the ordering of sibling hashes for this level.
            let mut children = sibling_hashes.clone();

            // Insert the current hash into the list at the correct position.
            children.insert(indicator_index, current_hash);

            // Ensure we have exactly ARITY children.
            while children.len() < ARITY as usize {
                children.push(PH::Hash::default());
            }

            // Update the current hash for the next level.
            current_hash = match path_hasher.hash_children(&children) {
                Ok(hash) => hash,
                Err(_) => return false,
            };
        }

        // Ensure the final hash matches the given root.
        current_hash == *root
    }
}

impl<E: Environment, PH: PathHash, const DEPTH: u8, const ARITY: u8> FromBytes for SparseKaryMerklePath<E, PH, DEPTH, ARITY> {
    /// Reads in a Sparse K-ary Merkle path from a buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the key hash.
        let key_hash = PH::Hash::read_le(&mut reader)?;
        // Read the Merkle path siblings.
        let siblings = (0..DEPTH)
            .map(|_| {
                (0..ARITY.saturating_sub(1))
                    .map(|_| PH::Hash::read_le(&mut reader))
                    .collect::<IoResult<Vec<_>>>()
            })
            .collect::<IoResult<Vec<_>>>()?;
        // Return the Merkle path.
        Self::try_from((key_hash, siblings)).map_err(into_io_error)
    }
}

impl<E: Environment, PH: PathHash, const DEPTH: u8, const ARITY: u8> ToBytes for SparseKaryMerklePath<E, PH, DEPTH, ARITY> {
    /// Writes the Sparse K-ary Merkle path to a buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the key hash.
        self.key_hash.write_le(&mut writer)?;
        // Write the Merkle path siblings.
        self.siblings
            .iter()
            .try_for_each(|siblings| siblings.iter().try_for_each(|sibling| sibling.write_le(&mut writer)))
    }
}

impl<E: Environment, PH: PathHash, const DEPTH: u8, const ARITY: u8> Serialize for SparseKaryMerklePath<E, PH, DEPTH, ARITY> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ToBytesSerializer::serialize_with_size_encoding(self, serializer)
    }
}

impl<'de, E: Environment, PH: PathHash, const DEPTH: u8, const ARITY: u8> Deserialize<'de> for SparseKaryMerklePath<E, PH, DEPTH, ARITY> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "Sparse K-ary Merkle path")
    }
}
