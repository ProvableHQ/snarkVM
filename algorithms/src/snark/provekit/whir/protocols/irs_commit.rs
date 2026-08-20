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

//! Interleaved Reed-Solomon Commitment Protocol
//!
//! Commits to a `num_vectors` by `vector_size` matrix over `F`.
//!
//! This will be reshaped into a `vector_size / interleaving_depth` by
//! `num_vectors * interleaving_depth` matrix. Then each row is encoded
//! using an NTT friendly Reed-Solomon code to produce a `num_vectors *
//! interleaving_depth` by `codeword_size` matrix. This matrix is committed
//! using the [`matrix_commit`] protocol.
//!
//! After committing the encoded matrix, the protocol generates a random
//! Reed-Solomon code of length `out_domain_samples` over an extension field `G`
//! of `F` and encodes the original matrix using this code to produce a
//! `num_vectors` by `out_domain_samples` matrix over `G`. Together, these two
//! encoded matrices form a commitment to the original matrix.
//!
//! On opening the commitment, the protocol randomly selects `in_domain_samples`
//! rows and opens it using the [`matrix_commit`] protocol. Sampling is done
//! with replacement, so may produce fewer than `in_domain_samples` distinct
//! rows. This produces `in_domain_samples` evaluation points in `F` and
//! `in_domain_samples` by `num_vectors * interleaving_depth`.
use std::{
    f64::{self, consts::LOG2_10},
    fmt,
    ops::Neg,
};

use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use snarkvm_fields::{Field, Zero};
#[cfg(any())]
use tracing::instrument;

use crate::snark::provekit::whir::{
    algebra::{
        dot,
        embedding::Embedding,
        fields::FieldWithSize,
        lift,
        linear_form::UnivariateEvaluation,
        mixed_univariate_evaluate,
        ntt,
    },
    engines::EngineId,
    hash::Hash,
    protocols::{challenge_indices::challenge_indices, matrix_commit},
    transcript::{
        Codec,
        Decoding,
        DuplexSpongeInterface,
        FieldElem,
        ProverMessage,
        ProverState,
        VerificationResult,
        VerifierMessage,
        VerifierState,
    },
    type_info::Typed,
    utils::{chunks_exact_or_empty, zip_strict},
    verify,
};

/// Commit to vectors over an fft-friendly field F
#[derive(Clone, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Config<M: Embedding> {
    /// Embedding into a (larger) field used for weights and drawing challenges.
    pub embedding: Typed<M>,

    /// The number of vectors to commit to in one operation.
    pub num_vectors: usize,

    /// The number of coefficients in each vector.
    pub vector_size: usize,

    /// The number of masking values to add per codeword.
    pub mask_length: usize,

    /// The number of Reed-Solomon evaluation points.
    pub codeword_length: usize,

    /// The number of independent codewords that are interleaved together.
    pub interleaving_depth: usize,

    /// The matrix commitment configuration.
    pub matrix_commit: matrix_commit::Config<M::Source>,

    /// Slack to the Jonhnson bound in list decoding.
    /// Zero indicates unique decoding.
    pub johnson_slack: OrderedFloat<f64>,

    /// The number of in-domain samples.
    pub in_domain_samples: usize,

    /// The number of out-of-domain samples.
    pub out_domain_samples: usize,

    /// Whether to sort and deduplicate the in-domain samples.
    ///
    /// Deduplication can slightly reduce proof size and prover/verifier
    /// complexity, but it makes transcript pattern and control flow
    /// non-deterministic.
    pub deduplicate_in_domain: bool,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default, Serialize, Deserialize)]
#[serde(bound = "F: Field, G: Field")]
#[must_use]
pub struct Witness<F: Field, G = F>
where
    G: Field,
{
    pub masks: Vec<F>,
    pub matrix: Vec<F>,
    pub matrix_witness: matrix_commit::Witness,
    pub out_of_domain: Evaluations<G>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default, Serialize, Deserialize)]
#[serde(bound = "G: Field")]
#[must_use]
pub struct Commitment<G: Field> {
    matrix_commitment: matrix_commit::Commitment,
    out_of_domain: Evaluations<G>,
}

/// Interleaved Reed-Solomon code.
///
/// Used for out- and in-domain samples.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize, Default)]
#[serde(bound = "F: Field")]
pub struct Evaluations<F: Field> {
    /// Evaluation points for the RS code.
    pub points: Vec<F>,

    /// Matrix of codewords for each row.
    pub matrix: Vec<F>,
}

impl<M: Embedding> Config<M> {
    pub fn new(
        security_target: f64,
        unique_decoding: bool,
        hash_id: EngineId,
        num_vectors: usize,
        vector_size: usize,
        interleaving_depth: usize,
        rate: f64,
    ) -> Self
    where
        M: Default,
    {
        assert!(vector_size.is_multiple_of(interleaving_depth));
        assert!(rate > 0. && rate <= 1.);
        let message_length = vector_size / interleaving_depth;
        #[allow(clippy::cast_sign_loss)]
        let codeword_length = (message_length as f64 / rate).ceil() as usize;
        let rate = message_length as f64 / codeword_length as f64;

        // Pick in- and out-of-domain samples.
        // η = slack to Johnson bound. We pick η = √ρ / 20.
        // TODO: Optimize picking η.
        let johnson_slack = if unique_decoding { 0.0 } else { rate.sqrt() / 20. };
        #[allow(clippy::cast_sign_loss)]
        let out_domain_samples = if unique_decoding {
            0
        } else {
            let field_size_bits = M::Target::field_size_bits();
            // Johnson list size bound 1 / (2 η √ρ)
            let list_size = 1. / (2. * johnson_slack * rate.sqrt());

            // The list size error is (L choose 2) * [(d - 1) / |𝔽|]^s
            // See [STIR] lemma 4.5.
            // We want to find s such that the error is less than security_target.
            let l_choose_2 = list_size * (list_size - 1.) / 2.;
            let log_per_sample = field_size_bits - ((vector_size - 1) as f64).log2();
            assert!(log_per_sample > 0.);
            ((security_target + l_choose_2.log2()) / log_per_sample).ceil().max(1.) as usize
        };
        #[allow(clippy::cast_sign_loss)]
        let in_domain_samples = {
            // Query error is (1 - δ)^q, so we compute 1 - δ
            let per_sample = if unique_decoding {
                // Unique decoding bound: δ = (1 - ρ) / 2
                f64::midpoint(1., rate)
            } else {
                // Johnson bound: δ = 1 - √ρ - η
                rate.sqrt() + johnson_slack
            };
            (security_target / (-per_sample.log2())).ceil() as usize
        };
        debug_assert_eq!(in_domain_samples, num_in_domain_queries(unique_decoding, security_target, rate));

        Self {
            embedding: Typed::<M>::default(),
            num_vectors,
            vector_size,
            mask_length: 0,
            codeword_length,
            interleaving_depth,
            matrix_commit: matrix_commit::Config::with_hash(hash_id, codeword_length, interleaving_depth * num_vectors),
            johnson_slack: OrderedFloat(johnson_slack),
            in_domain_samples,
            out_domain_samples,
            deduplicate_in_domain: false,
        }
    }

    pub const fn num_cols(&self) -> usize {
        self.matrix_commit.num_cols
    }

    pub const fn size(&self) -> usize {
        self.matrix_commit.size()
    }

    pub fn embedding(&self) -> &M {
        &self.embedding
    }

    pub const fn num_messages(&self) -> usize {
        self.interleaving_depth * self.num_vectors
    }

    pub fn message_length(&self) -> usize {
        assert!(self.vector_size.is_multiple_of(self.interleaving_depth));
        self.vector_size / self.interleaving_depth
    }

    /// Message length including mask coefficients.
    pub fn masked_message_length(&self) -> usize {
        self.message_length() + self.mask_length
    }

    pub fn evaluation_points(&self, indices: &[usize]) -> Vec<M::Source> {
        ntt::evaluation_points::<M::Source>(self.masked_message_length(), self.codeword_length, indices)
    }

    pub fn rate(&self) -> f64 {
        self.masked_message_length() as f64 / self.codeword_length as f64
    }

    pub fn unique_decoding(&self) -> bool {
        self.out_domain_samples == 0 && self.johnson_slack == 0.0
    }

    /// Compute a list size bound.
    pub fn list_size(&self) -> f64 {
        if self.unique_decoding() {
            1.
        } else {
            // This is the Johnson bound $1 / (2 η √ρ)$.
            1. / (2. * self.johnson_slack.into_inner() * self.rate().sqrt())
        }
    }

    /// Round-by-round soundness of the out-of-domain samples in bits.
    pub fn rbr_ood_sample(&self) -> f64 {
        let list_size = self.list_size();
        let log_field_size = M::Target::field_size_bits();
        // See [STIR] lemma 4.5.
        let l_choose_2 = list_size * (list_size - 1.) / 2.;
        let log_per_sample = ((self.vector_size - 1) as f64).log2() - log_field_size;
        -l_choose_2.log2() - self.out_domain_samples as f64 * log_per_sample
    }

    /// Round-by-round soundness of the in-domain queries in bits.
    pub fn rbr_queries(&self) -> f64 {
        let per_sample = if self.unique_decoding() {
            // 1 - δ = 1 - (1 + ρ) / 2
            f64::midpoint(1., self.rate())
        } else {
            // 1 - δ = sqrt(ρ) + η
            self.rate().sqrt() + self.johnson_slack.into_inner()
        };
        self.in_domain_samples as f64 * per_sample.log2().neg()
    }

    // Compute the proximity gaps term of the fold
    pub fn rbr_soundness_fold_prox_gaps(&self) -> f64 {
        let log_field_size = M::Target::field_size_bits();
        let log_inv_rate = self.rate().log2().neg();
        let log_k = (self.masked_message_length() as f64).log2();
        // See WHIR Theorem 4.8
        // Recall, at each round we are only folding by two at a time
        let error = if self.unique_decoding() {
            log_k + log_inv_rate
        } else {
            let log_eta = self.johnson_slack.into_inner().log2();
            // Make sure η hits the min bound.
            assert!(log_eta >= -(0.5 * log_inv_rate + LOG2_10 + 1.0) - 1e-6);
            7. * LOG2_10 + 3.5 * log_inv_rate + 2. * log_k
        };
        log_field_size - error
    }

    /// Commit to one or more vectors.
    #[cfg_attr(any(), instrument(skip_all, fields(self = %self)))]
    pub fn commit<H>(
        &self,
        prover_state: &mut ProverState<H>,
        vectors: &[&[M::Source]],
    ) -> Witness<M::Source, M::Target>
    where
        H: DuplexSpongeInterface,
        FieldElem<M::Target>: Codec<[H::U]>,
        Hash: ProverMessage<[H::U]>,
    {
        // Validate config
        assert!((self.vector_size).is_multiple_of(self.interleaving_depth));
        assert_eq!(self.matrix_commit.num_rows(), self.codeword_length);
        assert_eq!(self.matrix_commit.num_cols, self.num_messages());

        // Validate input
        assert_eq!(vectors.len(), self.num_vectors);
        assert!(vectors.iter().all(|p| p.len() == self.vector_size));

        // Generate random mask
        let masks = prover_state.random_field_vec(self.mask_length * self.num_messages());

        // Interleaved RS Encode the vectors
        let messages = vectors
            .iter()
            .flat_map(|v| chunks_exact_or_empty(v, self.message_length(), self.interleaving_depth))
            .collect::<Vec<_>>();
        let matrix = ntt::interleaved_rs_encode(&messages, &masks, self.codeword_length);

        // Commit to the matrix
        let matrix_witness = self.matrix_commit.commit(prover_state, &matrix);

        // Handle out-of-domain points and values
        let oods_points: Vec<M::Target> = prover_state.verifier_field_vec(self.out_domain_samples);
        let mut oods_matrix = Vec::with_capacity(self.out_domain_samples * self.num_vectors);
        for &point in &oods_points {
            for &vector in vectors {
                let value = mixed_univariate_evaluate(&*self.embedding, vector, point);
                prover_state.prover_field(&value);
                oods_matrix.push(value);
            }
        }

        Witness {
            masks,
            matrix,
            matrix_witness,
            out_of_domain: Evaluations { points: oods_points, matrix: oods_matrix },
        }
    }

    /// Receive a commitment to one or more vectors.
    #[cfg_attr(any(), instrument(skip_all, fields(self = %self)))]
    pub fn receive_commitment<H>(
        &self,
        verifier_state: &mut VerifierState<H>,
    ) -> VerificationResult<Commitment<M::Target>>
    where
        H: DuplexSpongeInterface,
        Hash: ProverMessage<[H::U]>,
        FieldElem<M::Target>: Codec<[H::U]>,
    {
        let matrix_commitment = self.matrix_commit.receive_commitment(verifier_state)?;
        let oods_points: Vec<M::Target> = verifier_state.verifier_field_vec(self.out_domain_samples);
        let oods_matrix = verifier_state.prover_fields_vec(self.out_domain_samples * self.num_vectors)?;
        Ok(Commitment { matrix_commitment, out_of_domain: Evaluations { points: oods_points, matrix: oods_matrix } })
    }

    /// Opens the commitment and returns the evaluations of the vectors.
    ///
    /// Constraints are returned as a pair of evaluation point and values
    /// for each row.
    ///
    /// When there are multiple openings, the evaluation matrices will
    /// be horizontally concatenated.
    #[cfg_attr(any(), instrument(skip_all, fields(self = %self)))]
    pub fn open<H>(
        &self,
        prover_state: &mut ProverState<H>,
        witnesses: &[&Witness<M::Source, M::Target>],
    ) -> Evaluations<M::Source>
    where
        H: DuplexSpongeInterface,
        u8: Decoding<[H::U]>,
        Hash: ProverMessage<[H::U]>,
    {
        for witness in witnesses {
            assert_eq!(witness.matrix.len(), self.size());
            assert_eq!(witness.out_of_domain.points.len(), self.out_domain_samples);
            assert_eq!(witness.out_of_domain.matrix.len(), self.out_domain_samples * self.num_vectors);
        }

        // Get in-domain openings
        let (indices, points) = self.in_domain_challenges(prover_state);

        // For each commitment, send the selected rows to the verifier
        // and collect them in the evaluation matrix.
        let stride = witnesses.len() * self.num_cols();
        let mut matrix = vec![M::Source::zero(); indices.len() * stride];
        let mut submatrix = Vec::with_capacity(indices.len() * self.num_cols());
        let mut matrix_col_offset = 0;
        for witness in witnesses {
            submatrix.clear();
            for (point_index, &code_index) in indices.iter().enumerate() {
                let row = &witness.matrix[code_index * self.num_cols()..(code_index + 1) * self.num_cols()];
                submatrix.extend_from_slice(row);

                let matrix_row = &mut matrix[point_index * stride..(point_index + 1) * stride];
                matrix_row[matrix_col_offset..matrix_col_offset + self.num_cols()].copy_from_slice(row);
            }
            prover_state.prover_hint_ark(&submatrix);
            self.matrix_commit.open(prover_state, &witness.matrix_witness, &indices);
            matrix_col_offset += self.num_cols();
        }

        Evaluations { points, matrix }
    }

    /// Verifies one or more openings and returns the in-domain evaluations.
    ///
    /// **Note.** This does not verify the out-of-domain evaluations.
    #[cfg_attr(any(), instrument(skip_all, fields(self = %self)))]
    pub fn verify<H>(
        &self,
        verifier_state: &mut VerifierState<H>,
        commitments: &[&Commitment<M::Target>],
    ) -> VerificationResult<Evaluations<M::Source>>
    where
        H: DuplexSpongeInterface,
        u8: Decoding<[H::U]>,
        Hash: ProverMessage<[H::U]>,
    {
        for commitment in commitments {
            verify!(commitment.out_of_domain.points.len() == self.out_domain_samples);
            verify!(commitment.out_of_domain.matrix.len() == self.num_vectors * self.out_domain_samples);
        }

        // Get in-domain openings
        let (indices, points) = self.in_domain_challenges(verifier_state);

        // Receive (as a hint) a matrix of all the columns of all the commitments
        // corresponding to the in-domain opening rows.
        let stride = commitments.len() * self.num_cols();
        let mut matrix = vec![M::Source::zero(); indices.len() * stride];
        let mut matrix_col_offset = 0;
        for commitment in commitments {
            let submatrix: Vec<M::Source> = verifier_state.prover_hint_ark()?;
            self.matrix_commit.verify(verifier_state, &commitment.matrix_commitment, &indices, &submatrix)?;
            // Horizontally concatenate matrices.
            if stride != 0 && self.num_cols() != 0 {
                for (dst, src) in zip_strict(matrix.chunks_exact_mut(stride), submatrix.chunks_exact(self.num_cols())) {
                    dst[matrix_col_offset..matrix_col_offset + self.num_cols()].copy_from_slice(src);
                }
            }
            matrix_col_offset += self.num_cols();
        }
        Ok(Evaluations { points, matrix })
    }

    fn in_domain_challenges<T>(&self, transcript: &mut T) -> (Vec<usize>, Vec<M::Source>)
    where
        T: VerifierMessage,
        u8: Decoding<[T::U]>,
    {
        // Get in-domain openings
        let indices =
            challenge_indices(transcript, self.codeword_length, self.in_domain_samples, self.deduplicate_in_domain);
        let points = self.evaluation_points(&indices);
        (indices, points)
    }
}

impl<G: Field> Commitment<G> {
    /// Returns the out-of-domain evaluations.
    pub const fn out_of_domain(&self) -> &Evaluations<G> {
        &self.out_of_domain
    }

    pub fn num_vectors(&self) -> usize {
        self.out_of_domain().num_columns()
    }
}

impl<F: Field, G: Field> Witness<F, G> {
    /// Returns the out-of-domain evaluations.
    pub const fn out_of_domain(&self) -> &Evaluations<G> {
        &self.out_of_domain
    }

    pub fn num_vectors(&self) -> usize {
        self.out_of_domain().num_columns()
    }
}

impl<F: Field> Evaluations<F> {
    pub const fn num_points(&self) -> usize {
        self.points.len()
    }

    pub fn num_columns(&self) -> usize {
        self.matrix.len().checked_div(self.num_points()).unwrap_or_default()
    }

    pub fn rows(&self) -> impl Iterator<Item = &[F]> {
        let cols = self.num_columns();
        (0..self.num_points()).map(move |i| &self.matrix[i * cols..(i + 1) * cols])
    }

    pub fn lift<M>(&self, embedding: &M) -> Evaluations<M::Target>
    where
        M: Embedding<Source = F>,
    {
        Evaluations { points: lift(embedding, &self.points), matrix: lift(embedding, &self.matrix) }
    }

    pub fn evaluators(&self, size: usize) -> impl '_ + Iterator<Item = UnivariateEvaluation<F>> {
        self.points.iter().map(move |&point| UnivariateEvaluation::new(point, size))
    }

    pub fn values<'a>(&'a self, weights: &'a [F]) -> impl 'a + Iterator<Item = F> {
        self.rows().map(|row| dot(weights, row))
    }
}

impl<M: Embedding> fmt::Display for Config<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "size {}×{}/{}", self.num_vectors, self.vector_size, self.interleaving_depth,)?;
        write!(f, " rate 2⁻{:.2}", -self.rate().log2())?;
        write!(f, " samples {} in- {} out-domain", self.in_domain_samples, self.out_domain_samples)
    }
}

/// Return the number of in-domain queries.
///
/// This is used by [`whir_zk`].
// TODO: A method with cleaner abstraction.
#[allow(clippy::cast_sign_loss)]
pub(crate) fn num_in_domain_queries(unique_decoding: bool, security_target: f64, rate: f64) -> usize {
    // Pick in- and out-of-domain samples.
    // η = slack to Johnson bound. We pick η = √ρ / 20.
    // TODO: Optimize picking η.
    let johnson_slack = if unique_decoding { 0.0 } else { rate.sqrt() / 20. };
    // Query error is (1 - δ)^q, so we compute 1 - δ
    let per_sample = if unique_decoding {
        // Unique decoding bound: δ = (1 - ρ) / 2
        f64::midpoint(1., rate)
    } else {
        // Johnson bound: δ = 1 - √ρ - η
        rate.sqrt() + johnson_slack
    };
    (security_target / (-per_sample.log2())).ceil() as usize
}

#[cfg(any())]
pub(crate) mod tests {
    use std::iter;

    use proptest::{bool, prelude::Strategy, proptest, sample::select, strategy::Just};
    use rand::{SeedableRng, distributions::Standard, prelude::Distribution, rngs::StdRng};

    use super::*;
    use crate::snark::provekit::whir::{
        algebra::{
            embedding::{Basefield, Compose, Frobenius, Identity},
            fields,
            ntt::NTT,
            random_vector,
            univariate_evaluate,
        },
        transcript::{DomainSeparator, codecs::U64},
    };

    impl<M: Embedding> Config<M> {
        pub fn arbitrary(
            embedding: M,
            num_vectors: usize,
            vector_size: usize,
            mask_length: usize,
            interleaving_depth: usize,
        ) -> impl Strategy<Value = Self> {
            assert!(interleaving_depth != 0);
            assert!(vector_size.is_multiple_of(interleaving_depth));
            let message_length = vector_size / interleaving_depth + mask_length;

            // Compute supported NTT domains for F
            let engine = NTT.get::<M::Source>().expect("Unsupported field");
            let valid_codeword_lengths = iter::successors(engine.next_order(message_length), |size| {
                    engine.next_order(*size + 1)
                })
                .filter(|n| n.is_power_of_two()) // TODO: Remove filter.
                .take(4)
                .collect::<Vec<_>>();
            let codeword_length = select(valid_codeword_lengths);

            // Combine with a matrix commitment config
            let codeword_matrix = codeword_length.prop_flat_map(move |codeword_length| {
                (
                    Just(codeword_length),
                    matrix_commit::tests::config::<M::Source>(codeword_length, interleaving_depth * num_vectors),
                )
            });

            (codeword_matrix, 0_usize..=10, 0_usize..=10, bool::ANY).prop_map(
                move |(
                    (codeword_length, matrix_commit),
                    in_domain_samples,
                    out_domain_samples,
                    deduplicate_in_domain,
                )| Self {
                    embedding: Typed::new(embedding.clone()),
                    num_vectors,
                    vector_size,
                    mask_length,
                    codeword_length,
                    interleaving_depth,
                    matrix_commit,
                    johnson_slack: OrderedFloat::default(),
                    in_domain_samples,
                    out_domain_samples,
                    deduplicate_in_domain,
                },
            )
        }
    }

    fn test<M: Embedding>(seed: u64, config: &Config<M>)
    where
        M::Source: ProverMessage,
        FieldElem<M::Target>: Codec,
        M::Source: snarkvm_utilities::Uniform,
        M::Target: snarkvm_utilities::Uniform,
    {
        crate::snark::provekit::whir::tests::init();

        // Pseudo-random Instance
        let instance = U64(seed);
        let ds =
            DomainSeparator::protocol(config).session(&format!("Test at {}:{}", file!(), line!())).instance(&instance);
        let mut rng = StdRng::seed_from_u64(seed);
        let vectors = (0..config.num_vectors).map(|_| random_vector(&mut rng, config.vector_size)).collect::<Vec<_>>();

        // TODO: Multiple commitments and openings.

        // Prover
        let mut prover_state = ProverState::new_std(&ds);
        let witness = config.commit(&mut prover_state, &vectors.iter().map(|p| p.as_slice()).collect::<Vec<_>>());
        assert_eq!(witness.out_of_domain().points.len(), config.out_domain_samples);
        assert_eq!(witness.out_of_domain().matrix.len(), config.out_domain_samples * config.num_vectors);
        if config.num_vectors > 0 {
            for (point, evals) in zip_strict(
                witness.out_of_domain().points.iter(),
                witness.out_of_domain().matrix.chunks_exact(config.num_vectors),
            ) {
                for (vector, expected) in zip_strict(vectors.iter(), evals.iter()) {
                    assert_eq!(mixed_univariate_evaluate(config.embedding(), vector, *point), *expected);
                }
            }
        }
        let in_domain_evals = config.open(&mut prover_state, &[&witness]);
        if config.deduplicate_in_domain {
            // Sorting is over index order, not points
            assert!(in_domain_evals.points.len() <= config.in_domain_samples);
            assert!({
                let mut unique = in_domain_evals.points.clone();
                unique.sort_unstable();
                unique.dedup();
                unique.len() == in_domain_evals.points.len()
            });
        } else {
            assert_eq!(in_domain_evals.points.len(), config.in_domain_samples);
        }
        assert_eq!(
            in_domain_evals.matrix.len(),
            in_domain_evals.points.len() * config.num_vectors * config.interleaving_depth
        );
        if config.num_vectors > 0 {
            let base = config.vector_size / config.interleaving_depth;
            for (point, evals) in zip_strict(
                &in_domain_evals.points,
                in_domain_evals.matrix.chunks_exact(config.num_vectors * config.interleaving_depth),
            ) {
                let expected_iter = vectors.iter().flat_map(|poly| {
                    (0..config.interleaving_depth).map(|j| {
                        // coefficients in the contiguous block for this interleaving index
                        let start = j * base;
                        let coeffs: Vec<_> = poly.iter().copied().skip(start).take(base).collect();
                        univariate_evaluate(&coeffs, *point)
                    })
                });
                for (expected, got) in zip_strict(expected_iter, evals.iter()) {
                    assert_eq!(expected, *got);
                }
            }
        }
        let proof = prover_state.proof();

        // Verifier
        let mut verifier_state = VerifierState::new_std(&ds, &proof);
        let commitment = config.receive_commitment(&mut verifier_state).unwrap();
        assert_eq!(commitment.out_of_domain(), witness.out_of_domain());
        let verifier_in_domain_evals = config.verify(&mut verifier_state, &[&commitment]).unwrap();
        assert_eq!(&verifier_in_domain_evals, &in_domain_evals);
        verifier_state.check_eof().unwrap();
    }

    fn proptest<M: Embedding>(embedding: &M)
    where
        M::Source: ProverMessage,
        FieldElem<M::Target>: Codec,
        M::Source: snarkvm_utilities::Uniform,
        M::Target: snarkvm_utilities::Uniform,
    {
        let valid_sizes = (1..=1024).filter(|&n| ntt::next_order::<M::Source>(n) == Some(n)).collect::<Vec<_>>();
        let size = select(valid_sizes);

        let config = (0_usize..=3, size, 1_usize..=10).prop_flat_map(|(num_vectors, size, interleaving_depth)| {
            Config::arbitrary(embedding.clone(), num_vectors, size * interleaving_depth, 0, interleaving_depth)
        });
        proptest!(|(
            seed: u64,
            config in config,
        )| {
            test(seed, &config);
        });
    }

    #[test]
    fn test_field64_1() {
        proptest(&Identity::<fields::Field64>::new());
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field64_2() {
        proptest(&Identity::<fields::Field64_2>::new());
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field64_3() {
        proptest(&Identity::<fields::Field64_3>::new());
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field128() {
        proptest(&Identity::<fields::Field128>::new());
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field192() {
        proptest(&Identity::<fields::Field192>::new());
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field256() {
        proptest(&Identity::<fields::Field256>::new());
    }

    #[test]
    fn test_basefield_field64_2() {
        proptest(&Basefield::<fields::Field64_2>::new());
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_basefield_field64_3() {
        proptest(&Basefield::<fields::Field64_3>::new());
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_base_frob_field64_3() {
        let embedding = Compose::new(Basefield::<fields::Field64_3>::new(), Frobenius::new(2));
        proptest(&embedding);
    }
}
