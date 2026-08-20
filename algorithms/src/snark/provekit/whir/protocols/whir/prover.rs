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

use std::{any::Any, borrow::Cow, mem};

use snarkvm_fields::{Field, One, Zero};
#[cfg(any())]
use tracing::instrument;

use super::{Config, Witness};
use crate::snark::provekit::whir::{
    algebra::{
        dot,
        embedding::Embedding,
        eq_weights,
        lift,
        linear_form::{Covector, Evaluate, LinearForm, UnivariateEvaluation},
        mixed_scalar_mul_add,
        sumcheck::fold,
        tensor_product,
    },
    hash::Hash,
    protocols::{geometric_challenge::geometric_challenge, irs_commit, whir::FinalClaim},
    transcript::{
        Codec,
        Decoding,
        DuplexSpongeInterface,
        FieldElem,
        ProverMessage,
        ProverState,
        VerifierMessage,
        codecs::U64,
    },
    utils::zip_strict,
};

enum RoundWitness<'a, F: Field, M: Embedding<Target = F>> {
    Initial(Vec<Cow<'a, irs_commit::Witness<M::Source, F>>>),
    Round(irs_commit::Witness<F, F>),
}

impl<M: Embedding> Config<M> {
    /// Prove a WHIR opening.
    ///
    /// * `prover_state` the mutable transcript to write the proof to.
    /// * `vectors` all the vectors we are opening.
    /// * `witnesses` witnesses corresponding to the `vectors`, in the same
    ///   order. Multiple vectors may share the same witness, in which case only
    ///   one witness should be provided.
    /// * `linear_forms` the covectors (if any) to evaluate each vector at.
    /// * `evaluations` a matrix of each vector evaluated at each linear form.
    ///
    /// The `evaluations` matrix is in row-major order with the number of rows
    /// equal to the `linear_forms.len()` and the number of columns equal to
    /// `vectors.len()`.
    #[cfg_attr(any(), instrument(skip_all))]
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    pub fn prove<'a, H>(
        &self,
        prover_state: &mut ProverState<H>,
        vectors: Vec<Cow<'a, [M::Source]>>,
        witnesses: Vec<Cow<'a, Witness<M::Target, M>>>,
        linear_forms: Vec<Box<dyn LinearForm<M::Target>>>,
        evaluations: Cow<'a, [M::Target]>,
    ) -> FinalClaim<M::Target>
    where
        H: DuplexSpongeInterface,
        FieldElem<M::Target>: Codec<[H::U]>,
        [u8; 32]: Decoding<[H::U]>,
        U64: Codec<[H::U]>,
        u8: Decoding<[H::U]>,
        Hash: ProverMessage<[H::U]>,
    {
        let num_vectors = vectors.len();

        // Input validation
        assert_eq!(num_vectors, witnesses.len() * self.initial_committer.num_vectors);
        assert_eq!(evaluations.len(), num_vectors * linear_forms.len());
        for vector in &vectors {
            assert_eq!(vector.len(), self.initial_size());
        }
        for linear_form in &linear_forms {
            assert_eq!(linear_form.size(), self.initial_size());
        }
        #[cfg(debug_assertions)]
        for (linear_form, evaluations) in zip_strict(linear_forms.iter(), evaluations.chunks_exact(num_vectors)) {
            use crate::snark::provekit::whir::algebra::linear_form::Covector;
            let covector = Covector::from(&**linear_form);
            for (vector, evaluation) in zip_strict(&vectors, evaluations) {
                debug_assert_eq!(covector.evaluate(self.embedding(), vector), *evaluation);
            }
        }
        if vectors.is_empty() {
            // TODO: Should we draw a random evaluation point of the right size?
            return FinalClaim::default();
        }

        // Complete evaluations of EVERY vector at EVERY linear form.
        let (oods_evals, oods_matrix) = {
            let mut oods_evals = Vec::new();
            let mut oods_matrix = Vec::new();

            // Out of domain samples. Compute missing cross-terms and send to verifier.
            let mut vector_offset = 0;
            for witness in &witnesses {
                for (oods_eval, oods_row) in
                    zip_strict(witness.out_of_domain().evaluators(self.initial_size()), witness.out_of_domain().rows())
                {
                    for (j, vector) in vectors.iter().enumerate() {
                        if j >= vector_offset && j < oods_row.len() + vector_offset {
                            debug_assert_eq!(oods_row[j - vector_offset], oods_eval.evaluate(self.embedding(), vector));

                            oods_matrix.push(oods_row[j - vector_offset]);
                        } else {
                            let eval = oods_eval.evaluate(self.embedding(), vector);
                            prover_state.prover_field(&eval);
                            oods_matrix.push(eval);
                        }
                    }
                    oods_evals.push(oods_eval);
                }
                vector_offset += witness.num_vectors();
            }
            (oods_evals, oods_matrix)
        };

        // Random linear combination of the vectors.
        let mut vector_rlc_coeffs: Vec<M::Target> = geometric_challenge(prover_state, num_vectors);
        assert_eq!(vector_rlc_coeffs[0], M::Target::one());
        // Recycle the first input as the accumulator (its coefficient is always ONE).
        let mut vectors = vectors.into_iter();
        let first = vectors.next().expect("non-empty");
        let mut vector = match first {
            Cow::Borrowed(slice) => lift(self.embedding(), slice),
            Cow::Owned(vec) => self.embedding().map_vec(vec),
        };
        for (rlc_coeff, input_vector) in zip_strict(&vector_rlc_coeffs[1..], vectors) {
            mixed_scalar_mul_add(self.embedding(), &mut vector, *rlc_coeff, &input_vector);
        }

        let mut prev_witness: RoundWitness<'a, M::Target, M> = RoundWitness::Initial(witnesses);

        // Random linear combination of the constraints.
        let constraint_rlc_coeffs: Vec<M::Target> =
            geometric_challenge(prover_state, linear_forms.len() + oods_evals.len());
        let has_constraints = !constraint_rlc_coeffs.is_empty();
        let (initial_forms_rlc_coeffs, oods_rlc_coeffs) = constraint_rlc_coeffs.split_at(linear_forms.len());
        // Try to recycle the first linear form as Covector.
        let mut covector = vec![];
        let mut linear_forms = linear_forms;
        if let Some((first, linear_forms)) = linear_forms.split_first_mut() {
            debug_assert_eq!(initial_forms_rlc_coeffs[0], M::Target::one());
            if let Some(covector_form) = (first.as_mut() as &mut dyn Any).downcast_mut::<Covector<M::Target>>() {
                mem::swap(&mut covector, &mut covector_form.vector);
            } else {
                covector.resize(self.initial_size(), M::Target::zero());
                first.accumulate(&mut covector, M::Target::one());
            }
            for (rlc_coeff, linear_form) in zip_strict(&initial_forms_rlc_coeffs[1..], linear_forms) {
                linear_form.accumulate(&mut covector, *rlc_coeff);
            }
        } else if has_constraints {
            covector.resize(self.initial_size(), M::Target::zero());
        }
        drop(linear_forms);

        // Compute "The Sum"
        let mut the_sum: M::Target = zip_strict(initial_forms_rlc_coeffs, evaluations.chunks_exact(num_vectors))
            .map(|(poly_coeff, row)| *poly_coeff * dot(&vector_rlc_coeffs, row))
            .sum();
        drop(evaluations);

        debug_assert!(!has_constraints || dot(&vector, &covector) == the_sum);

        // Add OODS constraints
        UnivariateEvaluation::accumulate_many(&oods_evals, &mut covector, oods_rlc_coeffs);
        the_sum += zip_strict(oods_rlc_coeffs, oods_matrix.chunks_exact(num_vectors))
            .map(|(poly_coeff, row)| *poly_coeff * dot(&vector_rlc_coeffs, row))
            .sum::<M::Target>();
        drop(oods_evals);
        drop(oods_matrix);

        debug_assert!(!has_constraints || dot(&vector, &covector) == the_sum);

        // Run initial sumcheck on batched vectors with combined statement
        let mut folding_randomness = if has_constraints {
            self.initial_sumcheck.prove(prover_state, &mut vector, &mut covector, &mut the_sum, &[]).0
        } else {
            // There are no constraints yet, so we can skip the sumcheck.
            // (If we did run it, all sumcheck vectors would be constant zero)
            // TODO: Don't compute evaluations and constraints in the first place.
            let folding_randomness =
                (0..self.initial_sumcheck.num_rounds).map(|_| prover_state.verifier_field()).collect();
            self.initial_skip_pow.prove(prover_state);
            // Fold vector
            for &f in &folding_randomness {
                fold(&mut vector, f);
            }
            // Covector must be all zeros.
            covector = vec![M::Target::zero(); self.initial_sumcheck.final_size()];
            folding_randomness
        };
        let mut evaluation_point = folding_randomness.clone();

        debug_assert_eq!(dot(&vector, &covector), the_sum);

        // Execute standard WHIR rounds on the batched vectors
        for (round_index, round_config) in self.round_configs.iter().enumerate() {
            // Commit to the vector, this generates out-of-domain evaluations.
            let new_witness = round_config.irs_committer.commit(prover_state, &[&vector]);

            // Proof of work before in-domain challenges
            round_config.pow.prove(prover_state);

            // Open the previous round's witness.
            let in_domain = match prev_witness {
                RoundWitness::Initial(init_witnesses) => {
                    let witness_refs: Vec<&_> = init_witnesses.iter().map(|c| &**c).collect();
                    self.initial_committer.open(prover_state, &witness_refs).lift(self.embedding())
                }
                RoundWitness::Round(old_witness) => {
                    let prev_round_config = &self.round_configs[round_index - 1];
                    prev_round_config.irs_committer.open(prover_state, &[&old_witness])
                }
            };

            // Collect constraints for this round and RLC them in
            let stir_challenges = new_witness
                .out_of_domain()
                .evaluators(round_config.initial_size())
                .chain(in_domain.evaluators(round_config.initial_size()))
                .collect::<Vec<_>>();
            let stir_evaluations = new_witness
                .out_of_domain()
                .values(&[M::Target::one()])
                .chain(in_domain.values(&tensor_product(&vector_rlc_coeffs, &eq_weights(&folding_randomness))))
                .collect::<Vec<_>>();
            let stir_rlc_coeffs = geometric_challenge(prover_state, stir_challenges.len());
            UnivariateEvaluation::accumulate_many(&stir_challenges, &mut covector, &stir_rlc_coeffs);
            the_sum += dot(&stir_rlc_coeffs, &stir_evaluations);
            debug_assert_eq!(dot(&vector, &covector), the_sum);

            // Run sumcheck for this round
            folding_randomness =
                round_config.sumcheck.prove(prover_state, &mut vector, &mut covector, &mut the_sum, &[]).0;

            evaluation_point.extend(folding_randomness.iter().copied());
            debug_assert_eq!(dot(&vector, &covector), the_sum);

            prev_witness = RoundWitness::Round(new_witness);
            vector_rlc_coeffs = vec![M::Target::one()];
        }

        // Directly send the vector to the verifier.
        assert_eq!(vector.len(), self.final_sumcheck.initial_size);
        for coeff in &vector {
            prover_state.prover_field(coeff);
        }

        // PoW
        self.final_pow.prove(prover_state);

        // Open and consume the final previous witness.
        match prev_witness {
            RoundWitness::Initial(init_witnesses) => {
                let witness_refs: Vec<&_> = init_witnesses.iter().map(|c| &**c).collect();
                let _in_domain = self.initial_committer.open(prover_state, &witness_refs);
            }
            RoundWitness::Round(old_witness) => {
                let prev_config = self.round_configs.last().unwrap();
                let _in_domain = prev_config.irs_committer.open(prover_state, &[&old_witness]);
            }
        }

        // Final sumcheck
        let final_folding_randomness =
            self.final_sumcheck.prove(prover_state, &mut vector, &mut covector, &mut the_sum, &[]).0;
        evaluation_point.extend(final_folding_randomness.iter().copied());

        FinalClaim {
            evaluation_point,
            rlc_coefficients: initial_forms_rlc_coeffs.to_vec(),
            linear_form_rlc: M::Target::zero(),
        }
    }
}
