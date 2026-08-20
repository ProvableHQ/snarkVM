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

//! Protocol for committing to rows of a matrix of some type <code>T:
//! [Encodable]</code>.

use std::fmt;

use derive_where::derive_where;
use serde::{Deserialize, Serialize};
use snarkvm_fields::{Field, PrimeField};
use static_assertions::assert_obj_safe;
#[cfg(any())]
use tracing::instrument;
use zerocopy::{Immutable, IntoBytes};

use crate::snark::provekit::whir::{
    engines::EngineId,
    hash::{self, Hash},
    protocols::merkle_tree,
    transcript::{
        DuplexSpongeInterface,
        ProverMessage,
        ProverState,
        VerificationError,
        VerificationResult,
        VerifierState,
    },
    type_info::{Type, TypeInfo},
    utils::{workload_size, zip_strict},
    verify,
};

/// Trait for types that can be encoded into a byte slice.
pub trait Encodable {
    /// Exact size in bytes this type will encode in.
    fn encoded_size() -> usize;

    /// Encoder for this type.
    ///
    /// During leaf hashing, an encoder will be created per thread and re-used
    /// as much as possible.
    fn encoder() -> Box<dyn Encoder<Self>>;
}

/// Object-safe encoder for types that implement [`Encodable`].
pub trait Encoder<T> {
    /// Returns true if the encoder uses an internal buffer.
    ///
    /// When true the input sizes will be chunked to limit the
    /// required buffer size.
    fn is_buffered(&self) -> bool;

    /// Encodes a slice of values into a byte slice.
    ///
    /// The byte slice must be exactly `values.len()` times `T::encoded_size()`
    /// bytes long.
    ///
    /// It can be a reference to an internal buffer in the encoder, or a
    /// reference to the values.
    fn encode<'e, 'd>(&'e mut self, values: &'d [T]) -> &'e [u8]
    where
        'd: 'e;
}

assert_obj_safe!(Encoder<()>);

/// Encoder for snarkVM field elements.
///
/// It encodes reduced values in little-endian byte order using the minimum
/// number of bytes that fit all field elements. For extensions,  the
/// coefficients are encoded in order of increasing degree.
pub struct ArkFieldEncoder(Vec<u8>);

/// Encoder for types that implement [`zerocopy`]'s [`Immutable`] and
/// [`IntoBytes`].
///
/// This is the fastest encoder available for types that support it. To use it
/// you need to implement [`Encodable`] as follows:
///
/// ```ignore
/// # use zerocopy::{Immutable, IntoBytes};
/// # use crate::snark::provekit::whir::protocols::matrix_commit::{Encoder, Encodable, ZeroCopyEncoder};
/// #[derive(Immutable, IntoBytes)]
/// pub struct MyType(u64);
///
/// impl Encodable for MyType {
///   fn encoded_size() -> usize {
///       std::mem::size_of::<Self>()
///   }
///
///   fn encoder() -> Box<dyn Encoder<Self>> {
///       Box::new(ZeroCopyEncoder)
///   }
/// }
/// ```
pub struct ZeroCopyEncoder;

/// Configuration for the matrix commit protocol.
///
/// Commits row-wise to a matrix of field elements using a merkle tree.
#[derive_where(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[derive(Serialize, Deserialize)]
#[serde(bound = "T: TypeInfo")]
pub struct Config<T>
where
    T: TypeInfo + Encodable + Send + Sync,
{
    /// Field used for the matrix elements.
    pub element_type: Type<T>,

    /// The number of columns in the matrix.
    pub num_cols: usize,

    /// The hash function used for rows of the matrix.
    pub leaf_hash_id: EngineId,

    /// Merkle tree configuration for the matrix.
    pub merkle_tree: merkle_tree::Config,
}

pub type Witness = merkle_tree::Witness;

pub type Commitment = merkle_tree::Commitment;

/// Encode snarkVM field elements using [`ArkFieldEncoder`].
impl<F: Field> Encodable for F {
    fn encoded_size() -> usize {
        F::BasePrimeField::size_in_bits().div_ceil(8)
    }

    fn encoder() -> Box<dyn Encoder<Self>> {
        Box::new(ArkFieldEncoder(Vec::new()))
    }
}

impl<F: Field> Encoder<F> for ArkFieldEncoder {
    fn is_buffered(&self) -> bool {
        true
    }

    fn encode<'e, 'd>(&'e mut self, values: &'d [F]) -> &'e [u8]
    where
        'd: 'e,
    {
        let width = F::BasePrimeField::size_in_bits().div_ceil(8);
        self.0.clear();
        self.0.reserve(values.len() * width);
        for value in values {
            let start = self.0.len();
            self.0.resize(start + width, 0);
            value.write_le(&mut self.0[start..]).expect("field encoding fits canonical width");
        }
        self.0.as_ref()
    }
}

impl<T: Immutable + IntoBytes> Encoder<T> for ZeroCopyEncoder {
    fn is_buffered(&self) -> bool {
        false
    }

    fn encode<'e, 'd>(&'e mut self, values: &'d [T]) -> &'e [u8]
    where
        'd: 'e,
    {
        values.as_bytes()
    }
}

impl<T: TypeInfo + Encodable + Send + Sync> Config<T> {
    /// Create a new matrix commit configuration with the recommended hash
    /// function.
    pub fn new(num_rows: usize, num_cols: usize) -> Self {
        // Select a leaf hash function.
        let leaf_size = T::encoded_size() * num_cols;
        let leaf_hash_id = if leaf_size <= 32 {
            hash::COPY
        } else if hash::Blake3::supports_size(leaf_size) {
            hash::BLAKE3
        } else {
            hash::SHA2
        };

        Self { element_type: Type::new(), num_cols, leaf_hash_id, merkle_tree: merkle_tree::Config::new(num_rows) }
    }

    pub fn with_hash(hash_id: EngineId, num_rows: usize, num_cols: usize) -> Self {
        Self {
            element_type: Type::new(),
            num_cols,
            leaf_hash_id: hash_id,
            merkle_tree: merkle_tree::Config::with_hash(hash_id, num_rows),
        }
    }

    pub const fn num_rows(&self) -> usize {
        self.merkle_tree.num_leaves
    }

    pub const fn size(&self) -> usize {
        self.num_rows() * self.num_cols
    }

    /// Commit the matrix (in row-major order).
    #[cfg_attr(any(), instrument(skip_all, fields(self = %self, size = matrix.len(), engine)))]
    pub fn commit<H>(&self, prover_state: &mut ProverState<H>, matrix: &[T]) -> Witness
    where
        H: DuplexSpongeInterface,
        Hash: ProverMessage<[H::U]>,
    {
        assert_eq!(matrix.len(), self.num_rows() * self.num_cols);

        let engine = hash::ENGINES.retrieve(self.leaf_hash_id).expect("Failed to retrieve hash engine");
        #[cfg(any())]
        tracing::Span::current().record("engine", engine.name().as_ref());

        // Compute leaf hashes
        let mut leaves = Vec::with_capacity(self.merkle_tree.num_nodes());
        leaves.resize(self.merkle_tree.num_leaves, Hash::default());
        hash_rows(&*engine, matrix, &mut leaves[..self.num_rows()]);

        // Commit the leaf hashes
        self.merkle_tree.commit(prover_state, leaves)
    }

    #[cfg_attr(any(), instrument(skip_all, fields(self = %self)))]
    pub fn receive_commitment<H>(&self, verifier_state: &mut VerifierState<H>) -> VerificationResult<Commitment>
    where
        H: DuplexSpongeInterface,
        Hash: ProverMessage<[H::U]>,
    {
        self.merkle_tree.receive_commitment(verifier_state)
    }

    /// Opens the commitment at the provided row indices.
    ///
    /// Indices can be in any order and may contain duplicates. The row values
    /// are not provided by this protocol, it is up to the caller to provide
    /// them to the verifier.
    #[cfg_attr(any(), instrument(skip(prover_state, witness, indices), fields(self = %self, num_indices = indices.len())))]
    pub fn open<H>(&self, prover_state: &mut ProverState<H>, witness: &Witness, indices: &[usize])
    where
        H: DuplexSpongeInterface,
        Hash: ProverMessage<[H::U]>,
    {
        self.merkle_tree.open(prover_state, witness, indices);
    }

    /// Verifies the commitment at the provided row indices.
    ///
    /// Indices can be in any order and may contain duplicates. The row values
    /// are not provided by this protocol, it is up to the caller to provide
    /// them to the verifier.
    #[cfg_attr(any(), instrument(skip_all, fields(self = %self, engine, num_indices = indices.len())))]
    pub fn verify<H>(
        &self,
        verifier_state: &mut VerifierState<H>,
        commitment: &Commitment,
        indices: &[usize],
        matrix: &[T],
    ) -> VerificationResult<()>
    where
        H: DuplexSpongeInterface,
        Hash: ProverMessage<[H::U]>,
    {
        verify!(matrix.len() == self.num_cols * indices.len());

        let engine = hash::ENGINES.retrieve(self.leaf_hash_id).ok_or(VerificationError)?;
        #[cfg(any())]
        tracing::Span::current().record("engine", engine.name().as_ref());

        let mut leaf_hashes = vec![Hash::default(); indices.len()];
        hash_rows(&*engine, matrix, &mut leaf_hashes);
        self.merkle_tree.verify(verifier_state, commitment, indices, &leaf_hashes)
    }
}

impl<T: TypeInfo + Encodable + Send + Sync> fmt::Display for Config<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MatrixCommit({} x {})", self.num_rows(), self.num_cols)
    }
}

#[cfg(feature = "serial")]
fn hash_rows<T: Encodable + Send + Sync>(engine: &dyn hash::HashEngine, matrix: &[T], out: &mut [Hash]) {
    hash_rows_serial(engine, matrix, out);
}

#[cfg(not(feature = "serial"))]
fn hash_rows<T: Encodable + Send + Sync>(engine: &dyn hash::HashEngine, matrix: &[T], out: &mut [Hash]) {
    let Some(cols) = matrix.len().checked_div(out.len()) else {
        // Empty input and output
        return;
    };
    if matrix.len() > workload_size::<T>() && out.len() > engine.preferred_batch_size() {
        // Large task, split into parallel threads.
        let split = (out.len() / 2).next_multiple_of(engine.preferred_batch_size());
        let (mat_a, mat_b) = matrix.split_at(split * cols);
        let (out_a, out_b) = out.split_at_mut(split);
        rayon::join(|| hash_rows(engine, mat_a, out_a), || hash_rows(engine, mat_b, out_b));
    } else {
        hash_rows_serial(engine, matrix, out);
    }
}

fn hash_rows_serial<T: Encodable + Send + Sync>(engine: &dyn hash::HashEngine, matrix: &[T], out: &mut [Hash]) {
    assert!(matrix.len().is_multiple_of(out.len()));
    let Some(cols) = matrix.len().checked_div(out.len()) else {
        // Empty input and output
        return;
    };
    let message_size = T::encoded_size() * cols;
    if message_size == 0 {
        // Empty message
        engine.hash_many(0, &[], out);
        return;
    }
    let mut encoder = T::encoder();
    if encoder.is_buffered() {
        // Buffered encoder, find some optimal size.
        let target = workload_size::<u8>() / 8;
        let batch_size = (target / message_size).max(1).next_multiple_of(engine.preferred_batch_size());
        for (matrix, out) in zip_strict(matrix.chunks(batch_size * cols), out.chunks_mut(batch_size)) {
            let bytes = encoder.encode(matrix);
            engine.hash_many(message_size, bytes, out);
        }
    } else {
        // Unbuffered, encode everything in one go
        let bytes = encoder.encode(matrix);
        engine.hash_many(message_size, bytes, out);
    }
}

#[cfg(any())]
pub(crate) mod tests {
    use proptest::{prop_assume, proptest, strategy::Strategy};
    use rand::{
        Rng,
        SeedableRng,
        distributions::{Distribution, Standard},
        rngs::StdRng,
    };

    use super::*;
    use crate::snark::provekit::whir::{
        algebra::fields,
        hash::{self, tests::hash_for_size},
        transcript::{DomainSeparator, codecs::Empty},
    };

    pub fn config<T>(num_rows: usize, num_cols: usize) -> impl Strategy<Value = Config<T>>
    where
        T: TypeInfo + Encodable + Send + Sync,
    {
        let leaf_hash = hash_for_size(T::encoded_size() * num_cols);
        let merkle_tree = merkle_tree::tests::config(num_rows);
        (leaf_hash, merkle_tree).prop_map(move |(leaf_hash_id, merkle_tree)| Config {
            element_type: Type::new(),
            num_cols,
            leaf_hash_id,
            merkle_tree,
        })
    }

    fn test<T>(
        mut rng: impl RngCore,
        leaf_hash: EngineId,
        node_hash: EngineId,
        layers: usize,
        num_rows: usize,
        num_cols: usize,
        indices: &[usize],
    ) where
        T: Clone + TypeInfo + Encodable + Send + Sync,
        T: snarkvm_utilities::Uniform,
    {
        crate::snark::provekit::whir::tests::init();
        assert!(layers >= merkle_tree::layers_for_size(num_rows));
        assert!(indices.iter().all(|&index| index < num_rows));

        // Config
        let config = Config {
            element_type: Type::<T>::new(),
            num_cols,
            leaf_hash_id: leaf_hash,
            merkle_tree: merkle_tree::Config {
                num_leaves: num_rows,
                layers: vec![merkle_tree::LayerConfig { hash_id: node_hash }; layers],
            },
        };
        let ds =
            DomainSeparator::protocol(&config).session(&format!("Test at {}:{}", file!(), line!())).instance(&Empty);

        // Instance
        let matrix: Vec<T> = (0..config.size()).map(|_| F::rand(&mut rng)).collect();
        let submatrix: Vec<T> = if num_cols > 0 {
            indices
                .iter()
                .flat_map(|&index| matrix.chunks_exact(num_cols).nth(index).unwrap().iter().cloned())
                .collect::<Vec<T>>()
        } else {
            Vec::new()
        };

        // Prover
        let mut prover_state = ProverState::new_std(&ds);
        let tree = config.commit(&mut prover_state, &matrix);
        config.open(&mut prover_state, &tree, indices);
        let proof = prover_state.proof();

        // Verifier
        let mut verifier_state = VerifierState::new_std(&ds, &proof);
        let commitment = config.receive_commitment(&mut verifier_state).unwrap();
        config.verify(&mut verifier_state, &commitment, indices, &submatrix).unwrap();
        verifier_state.check_eof().unwrap();
    }

    fn proptest<T>()
    where
        T: Clone + TypeInfo + Encodable + Send + Sync,
        T: snarkvm_utilities::Uniform,
    {
        let hashes = [hash::COPY, hash::SHA2, hash::SHA3, hash::BLAKE3];
        proptest!(|(
            seed: u64,
            leaf_hash in 0_usize..hashes.len(),
            node_hash in 1_usize..hashes.len(),
            layers in 0_usize..10,
            num_rows in 0_usize..100,
            num_cols in 0_usize..100,
            num_indices in 0_usize..100,
        )| {
            // There are no valid indices without rows.
            let num_indices = if num_rows == 0 { 0 } else { num_indices };

            // We need at least enough layers to cover the number of rows.
            let layers = layers + merkle_tree::layers_for_size(num_rows);

            let leaf_hash = hashes[leaf_hash];
            let node_hash = hashes[node_hash];
            prop_assume!(hash::ENGINES.retrieve(leaf_hash).unwrap().supports_size(T::encoded_size() * num_cols));
            prop_assume!(hash::ENGINES.retrieve(node_hash).unwrap().supports_size(64));

            let mut rng = StdRng::seed_from_u64(seed);
            let indices = (0..num_indices).map(|_| rng.gen_range(0..num_rows)).collect::<Vec<_>>();

            test::<T>(rng, leaf_hash, node_hash, layers, num_rows, num_cols, &indices);
        });
    }

    #[test]
    fn test_field64() {
        proptest::<fields::Field64>();
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field128() {
        proptest::<fields::Field128>();
    }

    #[test]
    fn test_field64_3() {
        // A non-power of two sized type.
        proptest::<fields::Field64_3>();
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field256() {
        proptest::<fields::Field256>();
    }
}
