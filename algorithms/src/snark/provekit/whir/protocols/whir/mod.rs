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

#![allow(type_alias_bounds)] // We need the bound to reference F::BasePrimeField.

mod config;
mod prover;
mod verifier;

use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use snarkvm_fields::Field;
#[cfg(any())]
use tracing::instrument;

use crate::snark::provekit::whir::{
    algebra::{
        embedding::{Embedding, Identity},
        linear_form::LinearForm,
    },
    hash::Hash,
    protocols::{irs_commit, proof_of_work, sumcheck},
    transcript::{
        Codec,
        DuplexSpongeInterface,
        FieldElem,
        ProverMessage,
        ProverState,
        VerificationResult,
        VerifierState,
    },
    utils::zip_strict,
    verify,
};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(bound = "")]
pub struct Config<M: Embedding> {
    pub initial_committer: irs_commit::Config<M>,
    pub initial_sumcheck: sumcheck::Config<M::Target>,
    pub initial_skip_pow: proof_of_work::Config,
    pub round_configs: Vec<RoundConfig<M::Target>>,
    pub final_sumcheck: sumcheck::Config<M::Target>,
    pub final_pow: proof_of_work::Config,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct RoundConfig<F: Field> {
    pub irs_committer: irs_commit::Config<Identity<F>>,
    pub sumcheck: sumcheck::Config<F>,
    pub pow: proof_of_work::Config,
}

pub type Witness<F: Field, M: Embedding<Target = F>> = irs_commit::Witness<M::Source, F>;
pub type Commitment<F: Field> = irs_commit::Commitment<F>;

#[must_use = "The final claim must be checked if there where any linear forms."]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FinalClaim<F: Field> {
    /// Multinlinear extension evaluation point.
    pub evaluation_point: Vec<F>,
    /// The random linear combination coefficients.
    pub rlc_coefficients: Vec<F>,
    /// Claimed value of the rlc of the mle of the linears forms in the point.
    /// Note: not computed on the prover side, set to zero instead.
    pub linear_form_rlc: F,
}

impl<F: Field> FinalClaim<F> {
    pub fn verify<'a>(
        &'a self,
        linear_forms: impl IntoIterator<Item = &'a dyn LinearForm<F>>,
    ) -> VerificationResult<()> {
        let rlc = zip_strict(&self.rlc_coefficients, linear_forms)
            .map(|(&c, l)| c * l.mle_evaluate(&self.evaluation_point))
            .sum::<F>();
        verify!(rlc == self.linear_form_rlc);
        Ok(())
    }
}

impl<M: Embedding> Config<M> {
    /// Commit to one or more vectors.
    #[cfg_attr(any(), instrument(skip_all, fields(size = vectors.first().unwrap().len())))]
    pub fn commit<H>(&self, prover_state: &mut ProverState<H>, vectors: &[&[M::Source]]) -> Witness<M::Target, M>
    where
        H: DuplexSpongeInterface,
        FieldElem<M::Target>: Codec<[H::U]>,
        Hash: ProverMessage<[H::U]>,
    {
        self.initial_committer.commit(prover_state, vectors)
    }

    /// Receive a commitment to vectors.
    pub fn receive_commitment<H>(
        &self,
        verifier_state: &mut VerifierState<H>,
    ) -> VerificationResult<Commitment<M::Target>>
    where
        H: DuplexSpongeInterface,
        FieldElem<M::Target>: Codec<[H::U]>,
        Hash: ProverMessage<[H::U]>,
    {
        self.initial_committer.receive_commitment(verifier_state)
    }

    /// Disable proof-of-work for test.
    #[cfg(any())]
    pub(crate) fn disable_pow(&mut self) {
        self.initial_sumcheck.round_pow.threshold = u64::MAX;
        self.initial_skip_pow.threshold = u64::MAX;
        for round in &mut self.round_configs {
            round.sumcheck.round_pow.threshold = u64::MAX;
            round.pow.threshold = u64::MAX;
        }
        self.final_sumcheck.round_pow.threshold = u64::MAX;
        self.final_pow.threshold = u64::MAX;
    }
}

#[cfg(any())]
mod tests {
    use std::borrow::Cow;

    use rand::thread_rng;
    use snarkvm_fields::{Field, One, Zero};

    use super::*;
    use crate::snark::provekit::whir::{
        algebra::{
            embedding::Basefield,
            fields::{Field64, Field64_3},
            linear_form::{Covector, Evaluate, LinearForm, MultilinearExtension},
            random_vector,
        },
        hash,
        parameters::ProtocolParameters,
        transcript::{DomainSeparator, ProverState, VerifierState, codecs::Empty},
        utils::test_serde,
    };

    /// Field type used in the tests.
    type F = Field64;

    /// Extension field type used in the tests.
    type EF = Field64_3;

    /// Build owned linear forms for `prove()` (which consumes them).
    fn build_prove_forms<F: Field>(
        points: &[Vec<F>],
        num_variables: usize,
        include_covector: bool,
    ) -> Vec<Box<dyn LinearForm<F>>> {
        let mut forms: Vec<Box<dyn LinearForm<F>>> = Vec::new();
        for point in points {
            forms.push(Box::new(MultilinearExtension { point: point.clone() }));
        }
        if include_covector {
            forms.push(Box::new(Covector { vector: (0..1 << num_variables).map(F::from).collect() }));
        }
        forms
    }

    /// Run a complete WHIR proof lifecycle: commit, prove, and verify.
    ///
    /// This function:
    /// - builds a multilinear polynomial with a specified number of variables,
    /// - constructs a statement with constraints based on evaluations and
    ///   linear relations,
    /// - commits to the polynomial using a Merkle-based commitment scheme,
    /// - generates a proof using the WHIR prover,
    /// - verifies the proof using the WHIR verifier.
    fn make_whir_things(
        num_variables: usize,
        initial_folding_factor: usize,
        folding_factor: usize,
        num_points: usize,
        unique_decoding: bool,
        pow_bits: usize,
    ) {
        // Number of coefficients in the multilinear polynomial (2^num_variables)
        let num_coeffs = 1 << num_variables;

        // Configure the WHIR protocol parameters
        let whir_params = ProtocolParameters {
            security_level: 32,
            pow_bits,
            initial_folding_factor,
            folding_factor,
            unique_decoding,
            starting_log_inv_rate: 1,
            batch_size: 1,
            hash_id: hash::SHA2,
        };

        // Build global configuration from protocol parameters
        let mut params = Config::<Basefield<EF>>::new(1 << num_variables, &whir_params);
        params.disable_pow();
        eprintln!("{params}");

        // Test that the config is serializable
        test_serde(&params);

        // Our test vector is all ones in the basefield.
        let vector = vec![F::one(); num_coeffs];

        // Generate `num_points` random points in the multilinear domain
        let points: Vec<_> = (0..num_points).map(|_| random_vector(thread_rng(), num_variables)).collect();

        let mut linear_forms: Vec<Box<dyn LinearForm<EF>>> = Vec::new();
        let mut evaluations = Vec::new();

        for point in &points {
            let linear_form = MultilinearExtension { point: point.clone() };
            evaluations.push(linear_form.evaluate(params.embedding(), &vector));
            linear_forms.push(Box::new(linear_form));
        }

        let covector = Covector { vector: (0..1 << num_variables).map(EF::from).collect() };
        let sum = covector.evaluate(params.embedding(), &vector);
        linear_forms.push(Box::new(covector));
        evaluations.push(sum);

        // Define the Fiat-Shamir domain separator for committing and proving
        let ds =
            DomainSeparator::protocol(&params).session(&format!("Test at {}:{}", file!(), line!())).instance(&Empty);

        // Initialize the Merlin transcript from the domain separator
        let mut prover_state = ProverState::new_std(&ds);

        // Commit to the polynomial and generate auxiliary witness data
        let witness = params.commit(&mut prover_state, &[&vector]);

        let prove_linear_forms = build_prove_forms(&points, num_variables, true);

        // Generate a proof for the given statement and witness
        let _ = params.prove(
            &mut prover_state,
            vec![Cow::from(vector)],
            vec![Cow::Owned(witness)],
            prove_linear_forms,
            Cow::Borrowed(evaluations.as_slice()),
        );

        // Reconstruct verifier's view of the transcript
        let proof = prover_state.proof();
        let mut verifier_state = VerifierState::new_std(&ds, &proof);
        let commitment = params.receive_commitment(&mut verifier_state).unwrap();

        // Verify the proof
        let final_claim = params.verify(&mut verifier_state, &[&commitment], &evaluations).unwrap();
        final_claim.verify(linear_forms.iter().map(|l| l.as_ref() as &dyn LinearForm<EF>)).unwrap();
    }

    #[test]
    fn test_whir_1() {
        for folding_factor in [1, 2, 3, 4] {
            let num_variables = folding_factor..=3 * folding_factor;
            for num_variable in num_variables {
                for num_points in [0, 1, 2] {
                    for unique_decoding in [true, false] {
                        for pow_bits in [0, 5, 10] {
                            eprintln!();
                            dbg!(folding_factor, num_variable, num_points, unique_decoding, pow_bits);

                            make_whir_things(
                                num_variable,
                                folding_factor,
                                folding_factor,
                                num_points,
                                unique_decoding,
                                pow_bits,
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_fail() {
        make_whir_things(3, 2, 2, 0, false, 0);
    }

    #[test]
    fn test_whir_mixed_folding_factors() {
        let folding_factors = [1, 2, 3, 4];
        let num_points = [0, 1, 2];

        for initial_folding_factor in folding_factors {
            for folding_factor in folding_factors {
                if initial_folding_factor == folding_factor {
                    continue;
                }
                let n = std::cmp::max(initial_folding_factor, folding_factor);
                let num_variables = n..=3 * n;
                for num_variable in num_variables {
                    for num_points in num_points {
                        eprintln!();
                        dbg!(initial_folding_factor, folding_factor, num_variable, num_points,);

                        make_whir_things(num_variable, initial_folding_factor, folding_factor, num_points, false, 5);
                    }
                }
            }
        }
    }

    /// Test batch proving with multiple independent polynomials and statements.
    ///
    /// Creates N separate polynomials, commits to each independently, and uses
    /// RLC to batch-prove them together. This verifies the full lifecycle:
    /// commitment, batch proving, and verification.
    fn make_whir_batch_things(
        num_variables: usize,
        initial_folding_factor: usize,
        folding_factor: usize,
        num_points_per_poly: usize,
        num_vectors: usize,
        unique_decoding: bool,
        pow_bits: usize,
    ) {
        let num_coeffs = 1 << num_variables;

        let whir_params = ProtocolParameters {
            security_level: 32,
            pow_bits,
            initial_folding_factor,
            folding_factor,
            unique_decoding,
            starting_log_inv_rate: 1,
            batch_size: 1,
            hash_id: hash::SHA2,
        };

        let mut params = Config::new(1 << num_variables, &whir_params);
        params.disable_pow();
        eprintln!("{params}");

        // Create N different vectors
        let vectors: Vec<_> = (0..num_vectors)
            .map(|i| {
                // Different vectors: first is all 1s, second is all 2s, etc.
                vec![F::from((i + 1) as u64); num_coeffs]
            })
            .collect();
        let vec_refs = vectors.iter().map(|v| v.as_slice()).collect::<Vec<_>>();

        let points: Vec<_> = (0..num_points_per_poly).map(|_| random_vector(thread_rng(), num_variables)).collect();

        let mut linear_forms: Vec<Box<dyn Evaluate<Basefield<EF>>>> = Vec::new();
        for point in &points {
            linear_forms.push(Box::new(MultilinearExtension { point: point.clone() }));
        }
        linear_forms.push(Box::new(Covector { vector: ((0..1 << num_variables).map(EF::from).collect()) }));

        let evaluations = linear_forms
            .iter()
            .flat_map(|linear_form| vec_refs.iter().map(|vec| linear_form.evaluate(params.embedding(), vec)))
            .collect::<Vec<_>>();

        // Set up domain separator for batch proving
        let ds =
            DomainSeparator::protocol(&params).session(&format!("Test at {}:{}", file!(), line!())).instance(&Empty);
        let mut prover_state = ProverState::new_std(&ds);

        // Commit to each polynomial and generate witnesses
        let mut witnesses = Vec::new();
        for &vec in &vec_refs {
            let witness = params.commit(&mut prover_state, &[vec]);
            witnesses.push(witness);
        }

        let prove_linear_forms = build_prove_forms(&points, num_variables, true);

        // Batch prove all polynomials together
        let _ = params.prove(
            &mut prover_state,
            vectors.iter().map(|v| Cow::Borrowed(v.as_slice())).collect(),
            witnesses.into_iter().map(Cow::Owned).collect(),
            prove_linear_forms,
            Cow::Borrowed(evaluations.as_slice()),
        );

        // Reconstruct verifier's transcript view
        let proof = prover_state.proof();
        let mut verifier_state = VerifierState::new_std(&ds, &proof);

        let mut commitments = Vec::new();
        for _ in 0..num_vectors {
            let commitment = params.receive_commitment(&mut verifier_state).unwrap();
            commitments.push(commitment);
        }
        let commitment_refs = commitments.iter().collect::<Vec<_>>();

        // Verify the batched proof
        let final_claim = params.verify(&mut verifier_state, &commitment_refs, &evaluations).unwrap();
        final_claim.verify(linear_forms.iter().map(|l| l.as_ref() as &dyn LinearForm<EF>)).unwrap();
    }

    #[test]
    fn test_whir_batch_1() {
        // Test with different configurations
        let folding_factors = [1, 2, 3, 4];
        let num_polynomials = [2, 3, 4];
        let num_points = [0, 1, 2];

        for initial_folding_factor in folding_factors {
            for folding_factor in folding_factors {
                let n = std::cmp::max(initial_folding_factor, folding_factor);
                // TODO: Batching with small number of variables..
                for num_variables in (initial_folding_factor + folding_factor)..=3 * n {
                    for num_polys in num_polynomials {
                        for num_points_per_poly in num_points {
                            eprintln!();
                            dbg!(initial_folding_factor, folding_factor, num_variables, num_polys, num_points_per_poly,);
                            make_whir_batch_things(
                                num_variables,
                                initial_folding_factor,
                                folding_factor,
                                num_points_per_poly,
                                num_polys,
                                false,
                                0, // pow_bits
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_whir_batch_single_polynomial() {
        // Edge case: batch proving with just one polynomial should also work
        make_whir_batch_things(
            6, // num_variables
            2, // initial_folding_factor
            2, // folding_factor
            2, // num_points_per_poly
            1, // num_polynomials (single!)
            false, 0,
        );
    }

    /// Test that batch verification rejects proofs with mismatched polynomials.
    ///
    /// This security test verifies that the cross-term commitment prevents the
    /// prover from using a different polynomial than what was committed.
    /// The prover commits to poly2 but attempts to use poly_wrong for
    /// evaluation, which should cause verification to fail.
    #[test]
    #[cfg_attr(any(), should_panic)]
    #[cfg_attr(debug_assertions, ignore = "debug_assert in prover panics on intentionally invalid input")]
    fn test_whir_batch_rejects_invalid_constraint() {
        // Setup parameters
        let num_variables = 4;
        let initial_folding_factor = 2;
        let folding_factor = 2;
        let num_polynomials = 2;
        let num_coeffs = 1 << num_variables;

        let whir_params = ProtocolParameters {
            security_level: 32,
            pow_bits: 0,
            initial_folding_factor,
            folding_factor,
            unique_decoding: false,
            starting_log_inv_rate: 1,
            batch_size: 1,
            hash_id: hash::SHA2,
        };

        let mut params = Config::<Basefield<EF>>::new(1 << num_variables, &whir_params);
        params.disable_pow();

        // Create test vectors
        let vec1 = vec![F::one(); num_coeffs];
        let vec2 = vec![F::from(2u64); num_coeffs];
        let vec_wrong = vec![F::from(999u64); num_coeffs];

        let constraint_points: Vec<_> = (0..2).map(|_| random_vector(thread_rng(), num_variables)).collect();

        let linear_forms: [Box<dyn Evaluate<Basefield<EF>>>; 2] = [
            Box::new(MultilinearExtension { point: constraint_points[0].clone() }),
            Box::new(MultilinearExtension { point: constraint_points[1].clone() }),
        ];
        let evaluations = linear_forms
            .iter()
            .flat_map(|weights| [&vec1, &vec_wrong].map(|v| weights.evaluate(params.embedding(), v)))
            .collect::<Vec<_>>();

        let ds =
            DomainSeparator::protocol(&params).session(&format!("Test at {}:{}", file!(), line!())).instance(&Empty);
        let mut prover_state = ProverState::new_std(&ds);

        let witness1 = params.commit(&mut prover_state, &[&vec1]);
        let witness2 = params.commit(&mut prover_state, &[&vec2]);

        let prove_linear_forms = build_prove_forms(&constraint_points, num_variables, false);

        // Generate proof with mismatched polynomials
        let _ = params.prove(
            &mut prover_state,
            vec![Cow::Borrowed(vec1.as_slice()), Cow::from(vec_wrong)],
            vec![Cow::Owned(witness1), Cow::Owned(witness2)],
            prove_linear_forms,
            Cow::Borrowed(evaluations.as_slice()),
        );

        // Verification should fail because the cross-terms don't match the commitment
        let proof = prover_state.proof();
        let mut verifier_state = VerifierState::new_std(&ds, &proof);

        let mut commitments = Vec::new();
        for _ in 0..num_polynomials {
            let parsed_commitment = params.receive_commitment(&mut verifier_state).unwrap();
            commitments.push(parsed_commitment);
        }

        let final_claim =
            params.verify(&mut verifier_state, &[&commitments[0], &commitments[1]], &evaluations).unwrap();
        let verifier_result = final_claim.verify(linear_forms.iter().map(|l| l.as_ref() as &dyn LinearForm<EF>));
        assert!(verifier_result.is_err(), "Verifier should reject mismatched polynomial");
    }

    /// Test batch proving with batch_size > 1 (multiple polynomials per
    /// commitment).
    ///
    /// This tests the case where each commitment contains multiple stacked
    /// polynomials (e.g., masked witness + random blinding for ZK), and we
    /// batch-prove multiple such commitments together.
    ///
    /// This was a regression test for a bug where the RLC combination of
    /// stacked leaf answers was incorrect when batch_size > 1.
    #[allow(clippy::too_many_arguments)]
    fn make_whir_batch_with_batch_size(
        num_variables: usize,
        initial_folding_factor: usize,
        folding_factor: usize,
        num_points_per_poly: usize,
        num_witnesses: usize,
        batch_size: usize,
        unique_decoding: bool,
        pow_bits: usize,
    ) {
        let num_coeffs = 1 << num_variables;

        let whir_params = ProtocolParameters {
            security_level: 32,
            pow_bits,
            initial_folding_factor,
            folding_factor,
            unique_decoding,
            starting_log_inv_rate: 1,
            batch_size, // KEY: batch_size > 1
            hash_id: hash::SHA2,
        };

        let mut params = Config::<Basefield<EF>>::new(1 << num_variables, &whir_params);
        params.disable_pow();

        // Create polynomials for each witness
        // Each witness will contain batch_size polynomials committed together
        let all_vectors: Vec<Vec<F>> =
            (0..num_witnesses * batch_size).map(|i| vec![F::from((i + 1) as u64); num_coeffs]).collect::<Vec<_>>();
        let vec_refs = all_vectors.iter().map(|p| p.as_slice()).collect::<Vec<_>>();

        let points: Vec<_> = (0..num_points_per_poly).map(|_| random_vector(thread_rng(), num_variables)).collect();

        let mut linear_forms: Vec<Box<dyn Evaluate<Basefield<EF>>>> = Vec::new();
        for point in &points {
            linear_forms.push(Box::new(MultilinearExtension { point: point.clone() }));
        }
        linear_forms.push(Box::new(Covector { vector: (0..1 << num_variables).map(EF::from).collect() }));

        let evaluations = linear_forms
            .iter()
            .flat_map(|linear_form| vec_refs.iter().map(|vec| linear_form.evaluate(params.embedding(), vec)))
            .collect::<Vec<_>>();

        // Set up domain separator
        let ds =
            DomainSeparator::protocol(&params).session(&format!("Test at {}:{}", file!(), line!())).instance(&Empty);
        let mut prover_state = ProverState::new_std(&ds);

        // Commit using commit_batch (stacks batch_size polynomials per witness)
        let mut witnesses = Vec::new();
        for witness_polys in vec_refs.chunks(batch_size) {
            let witness = params.commit(&mut prover_state, witness_polys);
            witnesses.push(witness);
        }

        let prove_linear_forms = build_prove_forms(&points, num_variables, true);

        // Batch prove all witnesses together
        let _ = params.prove(
            &mut prover_state,
            all_vectors.iter().map(|v| Cow::Borrowed(v.as_slice())).collect(),
            witnesses.into_iter().map(Cow::Owned).collect(),
            prove_linear_forms,
            Cow::Borrowed(evaluations.as_slice()),
        );

        // Verify
        let proof = prover_state.proof();
        let mut verifier_state = VerifierState::new_std(&ds, &proof);

        let mut commitments = Vec::new();
        for _ in 0..num_witnesses {
            let commitment = params.receive_commitment(&mut verifier_state).unwrap();
            commitments.push(commitment);
        }
        let commitment_refs = commitments.iter().collect::<Vec<_>>();

        let final_claim = params.verify(&mut verifier_state, &commitment_refs, &evaluations).unwrap();
        final_claim.verify(linear_forms.iter().map(|l| l.as_ref() as &dyn LinearForm<EF>)).unwrap();
    }

    #[test]
    fn test_whir_batch_with_batch_size_2() {
        // This is the key regression test for the batch_size > 1 bug
        let batch_sizes = [2, 3];
        let num_witnesses = [2, 3];
        let folding_factors = [2, 3];

        for batch_size in batch_sizes {
            for num_witness in num_witnesses {
                for folding_factor in folding_factors {
                    make_whir_batch_with_batch_size(
                        folding_factor * 2, // num_variables
                        folding_factor,
                        folding_factor,
                        1, // num_points_per_poly
                        num_witness,
                        batch_size,
                        false,
                        0, // pow_bits
                    );
                }
            }
        }
    }

    /// Run a complete WHIR proof lifecycle: commit, prove, and verify.
    fn make_batched_whir_things(
        batch_size: usize,
        num_variables: usize,
        initial_folding_factor: usize,
        folding_factor: usize,
        num_points: usize,
        unique_decoding: bool,
        pow_bits: usize,
    ) {
        eprintln!("\n---------------------");
        eprintln!("Test parameters: ");
        eprintln!("  num_vectors     : {batch_size}");
        eprintln!("  num_variables   : {num_variables}");
        eprintln!("  initial_folding : {initial_folding_factor}");
        eprintln!("  folding_factor  : {folding_factor}");
        eprintln!("  num_points      : {num_points:?}");
        eprintln!("  unique_decoding : {unique_decoding:?}");
        eprintln!("  pow_bits        : {pow_bits}");

        // Number of coefficients in the multilinear polynomial (2^num_variables)
        let num_coeffs = 1 << num_variables;

        // Configure the WHIR protocol parameters
        let whir_params = ProtocolParameters {
            security_level: 32,
            pow_bits,
            initial_folding_factor,
            folding_factor,
            unique_decoding,
            starting_log_inv_rate: 1,
            batch_size,
            hash_id: hash::SHA2,
        };

        // Build global configuration from multivariate + protocol parameters
        let mut params = Config::new(1 << num_variables, &whir_params);
        params.disable_pow();

        let vectors: Vec<Vec<F>> = (0..batch_size).map(|_| random_vector(thread_rng(), num_coeffs)).collect();
        let vec_refs = vectors.iter().map(|v| v.as_slice()).collect::<Vec<_>>();

        // Generate `num_points` random points in the multilinear domain
        let points: Vec<_> = (0..num_points).map(|_| random_vector(thread_rng(), num_variables)).collect();

        // Define the Fiat-Shamir IOPattern for committing and proving
        let ds =
            DomainSeparator::protocol(&params).session(&format!("Test at {}:{}", file!(), line!())).instance(&Empty);

        // Initialize the Merlin transcript from the domain separator
        let mut prover_state = ProverState::new_std(&ds);

        // Create a commitment to the polynomial and generate auxiliary witness data
        let batched_witness = params.commit(&mut prover_state, &vec_refs);

        // Create a weights matrix and evaluations for each polynomial
        let mut linear_forms: Vec<Box<dyn Evaluate<Basefield<F>>>> = Vec::new();
        for point in &points {
            linear_forms.push(Box::new(MultilinearExtension { point: point.clone() }));
        }
        linear_forms.push(Box::new(Covector { vector: (0..1 << num_variables).map(F::from).collect() }));
        let values = linear_forms
            .iter()
            .flat_map(|linear_form| vec_refs.iter().map(|vec| linear_form.evaluate(params.embedding(), vec)))
            .collect::<Vec<_>>();

        let prove_linear_forms = build_prove_forms(&points, num_variables, true);

        // Generate a proof for the given statement and witness
        let weights_dyn_refs = linear_forms.iter().map(|w| w.as_ref() as &dyn LinearForm<F>).collect::<Vec<_>>();
        let _ = params.prove(
            &mut prover_state,
            vectors.iter().map(|v| Cow::Borrowed(v.as_slice())).collect(),
            vec![Cow::Owned(batched_witness)],
            prove_linear_forms,
            Cow::Borrowed(values.as_slice()),
        );

        // Reconstruct verifier's view of the transcript using the IOPattern and
        // prover's data
        let proof = prover_state.proof();
        let mut verifier_state = VerifierState::new_std(&ds, &proof);

        let commitment = params.receive_commitment(&mut verifier_state).unwrap();

        // Verify that the generated proof satisfies the statement
        params.verify(&mut verifier_state, &[&commitment], &values).unwrap().verify(weights_dyn_refs).unwrap();
    }

    #[test]
    fn test_batched_whir() {
        let folding_factors = [1, 4];
        let unique_decoding_options = [false, true];
        let num_points = [0, 2];
        let pow_bits = [0, 10];

        for folding_factor in folding_factors {
            let num_variables = (2 * folding_factor)..=3 * folding_factor;
            for num_variable in num_variables {
                for num_points in num_points {
                    for unique_decoding in unique_decoding_options {
                        for pow_bits in pow_bits {
                            for batch_size in 1..=4 {
                                make_batched_whir_things(
                                    batch_size,
                                    num_variable,
                                    folding_factor,
                                    folding_factor,
                                    num_points,
                                    unique_decoding,
                                    pow_bits,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
