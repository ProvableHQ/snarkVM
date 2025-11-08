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
use snarkvm_circuit_types::U64;

impl<E: Environment, PH: PathHash<E>, const DEPTH: u8, const ARITY: u8> SparseKaryMerklePath<E, PH, DEPTH, ARITY> {
    /// Returns `true` if the Sparse K-ary Merkle path is valid for the given root and key-value pair.
    /// This implementation minimizes R1CS constraints by using efficient base-ARITY digit extraction
    /// and ternary operations for child selection.
    pub fn verify<KH, LH>(
        &self,
        key_hasher: &KH,
        leaf_hasher: &LH,
        path_hasher: &PH,
        root: &PH::Hash,
        key: &KH::Key,
        value: &LH::Leaf,
    ) -> Boolean<E>
    where
        PH: PathHash<E, Hash = Field<E>>,
        KH: KeyHash<E, Hash = Field<E>>,
        LH: LeafHash<Hash = Field<E>>,
        LH::Leaf: Clone,
    {
        // Compute the key hash in the circuit.
        let computed_key_hash = key_hasher.hash_key(key);

        // Ensure the key hash matches (using a constraint-efficient equality check).
        let key_hash_matches = computed_key_hash.is_equal(self.key_hash());

        // Compute the leaf hash in the circuit.
        let leaf_hash = leaf_hasher.hash_leaf(value);

        // Extract base-ARITY digits from the key hash efficiently.
        // We'll use the key hash field value and extract digits level by level.
        // Initialize a tracker for the current hash, starting with the leaf hash.
        let mut current_hash = leaf_hash;
        let arity = U64::<E>::constant(console::U64::new(ARITY as u64));
        
        // Convert key hash to U64 for digit extraction.
        // Extract enough bits to represent numbers up to ARITY^DEPTH.
        let bits_needed = ((DEPTH as f64) * (ARITY as f64).log2()).ceil() as usize;
        let key_hash_bits = self.key_hash().to_lower_bits_le(bits_needed.min(E::BaseField::size_in_bits()));
        let key_hash_u64 = U64::from_bits_le(&key_hash_bits[..64.min(key_hash_bits.len())]);

        // Extract path digits level by level.
        let mut remaining = key_hash_u64;

        // Check levels between leaf level and root.
        for (_level, sibling_hashes) in self.siblings.iter().enumerate() {
            // Extract the base-ARITY digit for this level: remaining % ARITY
            let indicator_index = &remaining % &arity;
            
            // Update remaining for next level: remaining / ARITY
            remaining = &remaining / &arity;

            // Assemble the children based on the ternary results.
            // We need to place current_hash at position indicator_index and siblings at other positions.
            let mut children = Vec::with_capacity(ARITY as usize);

            // Build children array efficiently using ternary operations.
            // For each position i from 0 to ARITY-1:
            //   - If i == indicator_index, use current_hash
            //   - Otherwise, use the appropriate sibling hash
            
            let zero_index = U64::<E>::zero();
            
            // First child: if indicator_index == 0, use current_hash, else use sibling_hashes[0]
            let first_child = PH::Hash::ternary(
                &indicator_index.is_equal(&zero_index),
                &current_hash,
                &sibling_hashes[0],
            );
            children.push(first_child);

            // Middle children: for i from 1 to ARITY-2
            for i in 1..(ARITY as usize - 1) {
                let index = U64::<E>::constant(console::U64::new(i as u64));
                
                // Determine which hash to use at this position.
                // If indicator_index < i, use sibling_hashes[i]
                // If indicator_index == i, use current_hash
                // If indicator_index > i, use sibling_hashes[i-1]
                
                let use_current = indicator_index.is_equal(&index);
                let use_prev_sibling = indicator_index.is_less_than(&index);
                
                // Select between current_hash, sibling_hashes[i], and sibling_hashes[i-1]
                let option_a = PH::Hash::ternary(
                    &use_prev_sibling,
                    &sibling_hashes[i],
                    &sibling_hashes[i - 1],
                );
                let option_b = PH::Hash::ternary(
                    &use_current,
                    &current_hash,
                    &option_a,
                );
                children.push(option_b);
            }

            // Last child: if indicator_index == ARITY-1, use current_hash, else use last sibling
            let last_index = U64::<E>::constant(console::U64::new((ARITY - 1) as u64));
            let last_child = PH::Hash::ternary(
                &indicator_index.is_equal(&last_index),
                &current_hash,
                sibling_hashes.last().unwrap(),
            );
            children.push(last_child);

            // Ensure we have exactly ARITY children.
            while children.len() < ARITY as usize {
                children.push(PH::Hash::zero());
            }

            // Update the current hash for the next level.
            current_hash = path_hasher.hash_children(&children);
        }

        // Ensure the final hash matches the given root.
        let root_matches = root.is_equal(&current_hash);

        // Both conditions must be true: key hash matches and root matches.
        key_hash_matches & root_matches
    }

    /// Verifies multiple Sparse K-ary Merkle paths in batch.
    /// Returns `true` if all paths are valid.
    pub fn verify_many<KH, LH>(
        paths: &[Self],
        key_hasher: &KH,
        leaf_hasher: &LH,
        path_hasher: &PH,
        root: &PH::Hash,
        entries: &[(KH::Key, LH::Leaf)],
    ) -> Boolean<E>
    where
        PH: PathHash<E, Hash = Field<E>>,
        KH: KeyHash<E, Hash = Field<E>>,
        LH: LeafHash<Hash = Field<E>>,
        LH::Leaf: Clone,
    {
        // Ensure the number of paths matches the number of entries.
        if paths.len() != entries.len() {
            E::halt("Number of paths must match number of entries")
        }

        // Verify each path individually and combine results with AND.
        let mut all_valid = Boolean::constant(true);
        for (path, (key, value)) in paths.iter().zip(entries.iter()) {
            let is_valid = path.verify(key_hasher, leaf_hasher, path_hasher, root, key, value);
            all_valid = all_valid & is_valid;
        }

        all_valid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_algorithms::{BHP512, BHP1024, Poseidon2, Poseidon4};
    use snarkvm_circuit_types::environment::Circuit;
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    const ITERATIONS: u128 = 10;
    const DOMAIN: &str = "SparseKaryMerkleTreeCircuit0";

    macro_rules! check_verify {
        ($kh:ident, $lh:ident, $ph:ident, $mode:ident, $depth:expr, $arity:expr, ($num_constants:expr, $num_public:expr, $num_private:expr, $num_constraints:expr)) => {{
            // Initialize the key hasher.
            let native_key_hasher =
                snarkvm_console_algorithms::$kh::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
            let circuit_key_hasher = $kh::<Circuit>::constant(native_key_hasher.clone());

            // Initialize the leaf hasher.
            let native_leaf_hasher =
                snarkvm_console_algorithms::$lh::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
            let circuit_leaf_hasher = $lh::<Circuit>::constant(native_leaf_hasher.clone());

            let mut rng = TestRng::default();

            // Initialize the path hasher.
            let native_path_hasher =
                snarkvm_console_algorithms::$ph::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
            let circuit_path_hasher = $ph::<Circuit>::constant(native_path_hasher.clone());

            for i in 0..ITERATIONS {
                // Create a sparse k-ary merkle tree with some entries.
                let num_entries = core::cmp::min(($arity as u128).pow($depth as u32), i);
                
                // Create entries.
                let entries: Vec<_> = (0..num_entries)
                    .map(|_| {
                        // Generate random fields using Uniform
                        let key_field: console::Field<<Circuit as Environment>::Network> = Uniform::rand(&mut rng);
                        let value_field: console::Field<<Circuit as Environment>::Network> = Uniform::rand(&mut rng);
                        let key = key_field.to_bits_le();
                        let value = value_field.to_bits_le();
                        (key, value)
                    })
                    .collect();

                // Compute the sparse k-ary merkle tree.
                let sparse_kary_merkle_tree = console::sparse_kary_merkle_tree::SparseKaryMerkleTree::<_, _, _, _, $depth, $arity>::new_with_entries(
                    &native_path_hasher,
                    &native_key_hasher,
                    &native_leaf_hasher,
                    &entries,
                    false,
                )?;

                for (key, value) in &entries {
                    // Compute the Sparse K-ary Merkle path.
                    let sparse_kary_merkle_path = sparse_kary_merkle_tree.prove(key)?;

                    // Initialize the Sparse K-ary Merkle path.
                    let path = SparseKaryMerklePath::<Circuit, $ph<Circuit>, $depth, $arity>::new(Mode::$mode, sparse_kary_merkle_path.clone());
                    assert_eq!(sparse_kary_merkle_path, path.eject_value());
                    // Initialize the Sparse K-ary Merkle root.
                    let root = Field::new(Mode::$mode, *sparse_kary_merkle_tree.root());
                    // Initialize the key.
                    let key_circuit: Vec<_> = Inject::new(Mode::$mode, key.clone());
                    // Initialize the value.
                    let value_circuit: Vec<_> = Inject::new(Mode::$mode, value.clone());

                    Circuit::scope(format!("Verify {}", Mode::$mode), || {
                        let candidate = path.verify(&circuit_key_hasher, &circuit_leaf_hasher, &circuit_path_hasher, &root, &key_circuit, &value_circuit);
                        assert!(candidate.eject_value());
                        assert_scope!($num_constants, $num_public, $num_private, $num_constraints);
                    });
                    Circuit::reset();
                }
            }
            Ok(())
        }};
    }

    #[test]
    fn test_verify_bhp512_constant() -> Result<()> {
        check_verify!(BHP1024, BHP1024, BHP512, Constant, 7, 8, (0, 0, 0, 0))
    }

    #[test]
    fn test_verify_bhp512_public() -> Result<()> {
        check_verify!(BHP1024, BHP1024, BHP512, Public, 7, 8, (0, 0, 0, 0))
    }

    #[test]
    fn test_verify_bhp512_private() -> Result<()> {
        check_verify!(BHP1024, BHP1024, BHP512, Private, 7, 8, (0, 0, 0, 0))
    }
}
