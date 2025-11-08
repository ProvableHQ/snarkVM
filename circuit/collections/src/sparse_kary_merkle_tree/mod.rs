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

mod verify;

#[cfg(test)]
use snarkvm_circuit_types::environment::assert_scope;

use snarkvm_circuit_types::{Boolean, Field, environment::prelude::*};

pub struct SparseKaryMerklePath<E: Environment, PH: PathHash<E>, const DEPTH: u8, const ARITY: u8> {
    /// The key hash for the path.
    key_hash: PH::Hash,
    /// The `siblings` contains a list of sibling hashes from the leaf to the root.
    /// Each level has ARITY-1 siblings.
    siblings: Vec<Vec<PH::Hash>>,
}

impl<E: Environment, PH: PathHash<E>, const DEPTH: u8, const ARITY: u8> Inject for SparseKaryMerklePath<E, PH, DEPTH, ARITY> {
    type Primitive = console::sparse_kary_merkle_tree::SparseKaryMerklePath<E::Network, PH::Primitive, DEPTH, ARITY>;

    /// Initializes a Sparse K-ary Merkle path from the given mode and native Sparse K-ary Merkle path.
    fn new(mode: Mode, sparse_kary_merkle_path: Self::Primitive) -> Self {
        // Initialize the key hash.
        // PH::Hash implements Inject with Primitive = PH::Primitive::Hash
        let key_hash: PH::Hash = Inject::new(mode, sparse_kary_merkle_path.key_hash());
        // Initialize the Sparse K-ary Merkle path siblings.
        let siblings: Vec<Vec<_>> = sparse_kary_merkle_path
            .siblings()
            .iter()
            .map(|nodes| nodes.iter().map(|node| Inject::new(mode, *node)).collect())
            .collect();

        // Ensure the Sparse K-ary Merkle path has the correct arity.
        for sibling in &siblings {
            if sibling.len() != ARITY.saturating_sub(1) as usize {
                return E::halt("Sparse K-ary Merkle path is not the correct arity");
            }
        }
        // Ensure the Sparse K-ary Merkle path is the correct depth.
        match siblings.len() == DEPTH as usize {
            // Return the Sparse K-ary Merkle path.
            true => Self { key_hash, siblings },
            false => E::halt("Sparse K-ary Merkle path is not the correct depth"),
        }
    }
}

impl<E: Environment, PH: PathHash<E>, const DEPTH: u8, const ARITY: u8> Eject for SparseKaryMerklePath<E, PH, DEPTH, ARITY> {
    type Primitive = console::sparse_kary_merkle_tree::SparseKaryMerklePath<E::Network, PH::Primitive, DEPTH, ARITY>;

    /// Ejects the mode of the Sparse K-ary Merkle path.
    fn eject_mode(&self) -> Mode {
        (&self.key_hash, &self.siblings).eject_mode()
    }

    /// Ejects the Sparse K-ary Merkle path.
    fn eject_value(&self) -> Self::Primitive {
        // Convert key_hash and siblings to console types.
        let key_hash_primitive = self.key_hash.eject_value();
        let siblings_primitive: Vec<Vec<_>> = self.siblings
            .iter()
            .map(|level| level.iter().map(|hash| hash.eject_value()).collect())
            .collect();
        
        match Self::Primitive::try_from((key_hash_primitive, siblings_primitive)) {
            Ok(sparse_kary_merkle_path) => sparse_kary_merkle_path,
            Err(error) => E::halt(format!("Failed to eject the Sparse K-ary Merkle path: {error}")),
        }
    }
}

impl<E: Environment, PH: PathHash<E>, const DEPTH: u8, const ARITY: u8> SparseKaryMerklePath<E, PH, DEPTH, ARITY> {
    /// Returns the key hash for the path.
    pub fn key_hash(&self) -> &PH::Hash {
        &self.key_hash
    }

    /// Returns the siblings for the path.
    pub fn siblings(&self) -> &[Vec<PH::Hash>] {
        &self.siblings
    }
}
