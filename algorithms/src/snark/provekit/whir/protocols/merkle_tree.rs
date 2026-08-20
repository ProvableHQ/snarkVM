// Copyright (c) 2019-2026 Provable Inc.
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

// Originally derived from WHIR (https://github.com/WizardOfMenlo/whir),
// licensed under Apache-2.0 OR MIT.

//! Protocol for committing to a vector of [`struct@Hash`]es.
//!
//! See <https://eprint.iacr.org/2026/089> for analysis when used with truncated permutation
//! node hashes.

use std::{fmt, mem::swap};

use serde::{Deserialize, Serialize};
#[cfg(any())]
use tracing::{Level, instrument, span};
use zerocopy::IntoBytes;

use crate::snark::provekit::whir::{
    engines::EngineId,
    hash::{self, ENGINES, Hash, HashEngine},
    transcript::{
        DuplexSpongeInterface,
        ProverMessage,
        ProverState,
        VerificationError,
        VerificationResult,
        VerifierState,
    },
    utils::zip_strict,
    verify,
};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default, Serialize, Deserialize)]
pub struct Config {
    /// Number of leaves in the Merkle tree.
    pub num_leaves: usize,

    /// Layer configurations for the Merkle tree, root to bottom.
    pub layers: Vec<LayerConfig>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Copy, Debug, Hash, Default, Serialize, Deserialize)]
pub struct LayerConfig {
    /// The engine used to hash siblings.
    pub hash_id: EngineId,
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MerkleTree(num_leaves: {})", self.num_leaves)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default, Serialize, Deserialize)]
#[must_use]
pub struct Commitment {
    /// The commitment root hash.
    hash: Hash,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default, Serialize, Deserialize)]
#[must_use]
pub struct Witness {
    /// The nodes in the Merkle tree, starting with the leaf hash layer.
    nodes: Vec<Hash>,
}

impl Config {
    /// Create a new Merkle tree configuration with the recommended hash
    /// function.
    pub fn new(num_leaves: usize) -> Self {
        Self::with_hash(hash::BLAKE3, num_leaves)
    }

    pub fn with_hash(hash_id: EngineId, num_leaves: usize) -> Self {
        Self { num_leaves, layers: vec![LayerConfig { hash_id }; layers_for_size(num_leaves)] }
    }

    pub const fn num_nodes(&self) -> usize {
        (1 << (self.layers.len() + 1)) - 1
    }

    #[cfg_attr(any(), instrument(skip(prover_state, leaves), fields(self = %self)))]
    pub fn commit<H>(&self, prover_state: &mut ProverState<H>, leaves: Vec<Hash>) -> Witness
    where
        H: DuplexSpongeInterface,
        Hash: ProverMessage<[H::U]>,
    {
        assert_eq!(leaves.len(), self.num_leaves, "Expected {} leaf hashes, got {}", self.num_leaves, leaves.len());

        // Allocate nodes and fill with leaf layer. This implicitly pads the first
        // layer.
        let mut nodes = leaves;
        nodes.resize(self.num_nodes(), Hash::default());
        let (mut previous, mut remaining) = nodes.split_at_mut(1 << self.layers.len());

        // Compute merkle tree nodes.
        for layer in self.layers.iter().rev() {
            let (current, next_remaining) = remaining.split_at_mut(previous.len() / 2);
            let engine = ENGINES.retrieve(layer.hash_id).expect("Hash Engine not found");
            #[cfg(any())]
            let _span = span!(Level::DEBUG, "layer", engine = engine.name().as_ref(), count = current.len()).entered();

            // TODO: Parallelize over subtrees, not layerwise. This will
            // increase locality.
            parallel_hash(&*engine, 64, previous.as_bytes(), current);
            previous = current;
            remaining = next_remaining;
        }

        // Commit to the root hash.
        prover_state.prover_message(&previous[0]);

        Witness { nodes }
    }

    pub fn receive_commitment<H>(&self, verifier_state: &mut VerifierState<H>) -> VerificationResult<Commitment>
    where
        H: DuplexSpongeInterface,
        Hash: ProverMessage<[H::U]>,
    {
        let hash = verifier_state.prover_message()?;
        Ok(Commitment { hash })
    }

    /// Opens the commitment at the provided indices.
    ///
    /// Indices can be in any order and may contain duplicates.
    #[cfg_attr(any(), instrument(skip_all, fields(num_indices = indices.len())))]
    pub fn open<H>(&self, prover_state: &mut ProverState<H>, witness: &Witness, indices: &[usize])
    where
        H: DuplexSpongeInterface,
        Hash: ProverMessage<[H::U]>,
    {
        assert_eq!(witness.nodes.len(), self.num_nodes());
        assert!(indices.iter().all(|&i| i < self.num_leaves));

        // Abstract execution of verify algorithm writing required hashes.
        let mut indices = indices.to_vec();
        indices.sort_unstable();
        indices.dedup();
        let (mut layer, mut remaining) = witness.nodes.split_at(1 << self.layers.len());
        while layer.len() > 1 {
            let mut next_indices = Vec::with_capacity(indices.len());
            let mut iter = indices.iter().copied().peekable();
            loop {
                match (iter.next(), iter.peek()) {
                    (Some(a), Some(&b)) if b == a ^ 1 => {
                        // Neighboring indices, merging branches.
                        next_indices.push(a >> 1);
                        iter.next(); // Skip the next index.
                    }
                    (Some(a), _) => {
                        // Single index, pushing the neighbor hash.
                        prover_state.prover_hint(&layer[a ^ 1]);
                        next_indices.push(a >> 1);
                    }
                    (None, _) => break,
                }
            }
            indices = next_indices;
            let (next_layer, next_remaining) = remaining.split_at(layer.len() / 2);
            layer = next_layer;
            remaining = next_remaining;
        }
    }

    pub fn verify<H>(
        &self,
        verifier_state: &mut VerifierState<H>,
        commitment: &Commitment,
        indices: &[usize],
        leaf_hashes: &[Hash],
    ) -> VerificationResult<()>
    where
        H: DuplexSpongeInterface,
        Hash: ProverMessage<[H::U]>,
    {
        // Validate indices.
        verify!(indices.len() == leaf_hashes.len());
        verify!(indices.iter().all(|&i| i < self.num_leaves));
        if indices.is_empty() {
            return Ok(());
        }

        // Sort indices and leaf hashes.
        let mut layer =
            zip_strict(indices.iter().copied(), leaf_hashes.iter().copied()).collect::<Vec<(usize, Hash)>>();
        layer.sort_unstable_by_key(|(i, _)| *i);

        // Check duplicate leaf consistency and deduplicate.
        for i in 1..layer.len() {
            if layer[i - 1].0 == layer[i].0 {
                verify!(layer[i - 1].1 == layer[i].1);
            }
        }
        layer.dedup_by_key(|(i, _)| *i);

        // Validate the layers
        let mut indices = layer.iter().map(|(i, _)| *i).collect::<Vec<_>>();
        let mut hashes = layer.iter().map(|(_, h)| *h).collect::<Vec<_>>();
        let mut next_indices = Vec::with_capacity(layer.len());
        let mut input_hashes = Vec::with_capacity(layer.len() * 2);
        let mut next_hashes = Vec::with_capacity(layer.len());
        for layer in self.layers.iter().rev() {
            next_indices.clear();
            input_hashes.clear();
            next_hashes.clear();

            // Pair hashes with either hint or neighbor.
            let mut indices_iter = indices.iter().copied().peekable();
            let mut hashes_iter = hashes.iter().copied();
            loop {
                match (indices_iter.next(), indices_iter.peek()) {
                    (Some(a), Some(&b)) if b == a ^ 1 => {
                        // Neighboring indices, merging branches.
                        input_hashes.push(hashes_iter.next().unwrap());
                        input_hashes.push(hashes_iter.next().unwrap());
                        next_indices.push(a >> 1);
                        indices_iter.next(); // Skip the next index.
                    }
                    (Some(a), _) => {
                        // Single index, receiving the neighbor hash.
                        let hash = verifier_state.prover_hint()?;
                        if a & 1 == 0 {
                            input_hashes.push(hashes_iter.next().unwrap());
                            input_hashes.push(hash);
                        } else {
                            input_hashes.push(hash);
                            input_hashes.push(hashes_iter.next().unwrap());
                        }
                        next_indices.push(a >> 1);
                    }
                    (None, _) => break,
                }
            }

            // Compute next layer hashes
            next_hashes.resize(input_hashes.len() / 2, Hash::default());
            ENGINES.retrieve(layer.hash_id).ok_or(VerificationError)?.hash_many(
                64,
                input_hashes.as_bytes(),
                &mut next_hashes,
            );
            swap(&mut indices, &mut next_indices);
            swap(&mut hashes, &mut next_hashes);
        }

        // We should be left with a single root hash, matching the commitment.
        verify!(indices == [0]);
        verify!(hashes == [commitment.hash]);
        Ok(())
    }
}

impl Witness {
    pub const fn num_nodes(&self) -> usize {
        self.nodes.len()
    }
}

pub const fn layers_for_size(size: usize) -> usize {
    size.next_power_of_two().ilog2() as usize
}

#[cfg(feature = "serial")]
fn parallel_hash(engine: &dyn HashEngine, size: usize, input: &[u8], output: &mut [Hash]) {
    engine.hash_many(size, input, output);
}

#[cfg(not(feature = "serial"))]
fn parallel_hash(engine: &dyn HashEngine, size: usize, input: &[u8], output: &mut [Hash]) {
    use crate::snark::provekit::whir::utils::workload_size;
    assert_eq!(input.len(), size * output.len());
    if input.len() > workload_size::<u8>() && input.len() / size >= 2 {
        let (input_a, input_b) = input.split_at(input.len() / 2);
        let (output_a, output_b) = output.split_at_mut(output.len() / 2);
        rayon::join(
            || parallel_hash(engine, size, input_a, output_a),
            || parallel_hash(engine, size, input_b, output_b),
        );
    } else {
        engine.hash_many(size, input, output);
    }
}

#[cfg(any())]
pub(crate) mod tests {
    use proptest::{collection::vec, prelude::Strategy};

    use super::*;
    use crate::snark::provekit::whir::{
        hash::{BLAKE3, tests::hash_for_size},
        transcript::{DomainSeparator, codecs::Empty},
    };

    pub fn config(num_leaves: usize) -> impl Strategy<Value = Config> {
        let min_layers = layers_for_size(num_leaves);
        // Add up to three unnecessary layers
        let num_layers = min_layers..=min_layers + 3;
        // Each layer gets its own choice of hash function
        let layer = hash_for_size(64).prop_map(|hash_id| LayerConfig { hash_id });
        vec(layer, num_layers).prop_map(move |layers| Config { num_leaves, layers })
    }

    #[test]
    fn test_merkle_tree() {
        crate::snark::provekit::whir::tests::init();
        let config = Config { num_leaves: 256, layers: vec![LayerConfig { hash_id: BLAKE3 }; 8] };

        let leaves = (0..config.num_leaves).map(|i| Hash([i as u8; 32])).collect::<Vec<_>>();

        let ds =
            DomainSeparator::protocol(&config).session(&format!("Test at {}:{}", file!(), line!())).instance(&Empty);

        // Prover
        let mut prover_state = ProverState::new_std(&ds);
        let tree = config.commit(&mut prover_state, leaves);
        config.open(&mut prover_state, &tree, &[13, 42]);
        let proof = prover_state.proof();

        // Verifier
        let mut verifier_state = VerifierState::new_std(&ds, &proof);
        let root = config.receive_commitment(&mut verifier_state).unwrap();
        config.verify(&mut verifier_state, &root, &[13, 42], &[Hash([13; 32]), Hash([42; 32])]).unwrap();
    }

    #[test]
    fn test_layers_for_size() {
        assert_eq!(layers_for_size(0), 0);
        assert_eq!(layers_for_size(1), 0);
        assert_eq!(layers_for_size(2), 1);
        assert_eq!(layers_for_size(3), 2);
        assert_eq!(layers_for_size(4), 2);
        assert_eq!(layers_for_size(5), 3);
        assert_eq!(layers_for_size(6), 3);
        assert_eq!(layers_for_size(7), 3);
        assert_eq!(layers_for_size(8), 3);
    }
}
