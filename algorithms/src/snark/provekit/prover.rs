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

// Originally derived from ProveKit, Copyright 2026 World Foundation (MIT).

use crate::snark::provekit::{
    common::{
        Base,
        Ext,
        FieldHash,
        PrefixCovector,
        ProofField,
        PublicInputs,
        R1CS,
        WhirR1CSProof,
        WhirR1CSScheme,
        prefix_covector::{
            OffsetCovector,
            build_prefix_covectors,
            compute_alpha_evals,
            compute_challenge_eval,
            compute_public_eval,
            expand_powers,
            make_challenge_weight,
            make_public_weight,
        },
        utils::{
            pad_to_power_of_two,
            sumcheck::{
                calculate_evaluations_over_boolean_hypercube_for_eq,
                calculate_witness_bounds,
                eval_cubic_poly,
                mixed_fold,
                mixed_sumcheck_map_reduce,
                multiply_transposed_by_eq_alpha,
                sumcheck_fold_map_reduce,
                transpose_r1cs_matrices,
            },
        },
    },
    whir::{
        algebra::{
            dot,
            embedding::{Embedding, Identity},
            linear_form::LinearForm,
            mixed_dot,
        },
        protocols::whir::Witness as WhirWitness,
        transcript::{Codec, DuplexSpongeInterface, FieldElem, ProverState, VerifierMessage},
    },
};
use anyhow::{Result, ensure};
use snarkvm_fields::{Field, One, Zero};
use std::borrow::Cow;

/// Spartan sumcheck blinding `g`, committed separately in the extension field.
pub struct BlindingState<P: ProofField> {
    /// The `m_0` cubic blinding univariates (4 ext coefficients each).
    pub polynomial: Vec<[Ext<P>; 4]>,
    /// `polynomial` flattened (length `4 * m_0`) and zero-padded to the
    /// blinding commitment domain — the vector actually committed.
    pub vector: Vec<Ext<P>>,
    /// WHIR witness for the ext blinding commitment.
    pub witness: WhirWitness<Ext<P>, Identity<Ext<P>>>,
}

pub struct WhirR1CSCommitment<P: ProofField> {
    pub witness: WhirWitness<Ext<P>, P::Embedding>,
    pub polynomial: Vec<Base<P>>,
    pub blinding: Option<BlindingState<P>>,
}

/// A single column-axis SPARK query produced during a witness open: the final
/// WHIR evaluation point (column axis) plus the claimed evaluations of the
/// three alpha covectors (A, B, C matrices) at that point.
pub struct SparkColQueryData<P: ProofField> {
    pub col: Vec<Ext<P>>,
    pub claimed_a: Ext<P>,
    pub claimed_b: Ext<P>,
    pub claimed_c: Ext<P>,
}

/// SPARK query material produced as a side output of proving: the Spartan
/// sumcheck point (shared row axis) plus one column query per witness
/// commitment. The concrete backend converts this into its serialized SPARK
/// query batch type.
pub struct SparkQueryData<P: ProofField> {
    pub row: Vec<Ext<P>>,
    pub queries: Vec<SparkColQueryData<P>>,
}

pub trait WhirR1CSProver<P: FieldHash> {
    fn commit(
        &self,
        merlin: &mut ProverState<P::Sponge>,
        num_witnesses: usize,
        num_constraints: usize,
        witness: Vec<Base<P>>,
        is_w1: bool,
    ) -> Result<WhirR1CSCommitment<P>>;

    fn prove_noir(
        &self,
        merlin: ProverState<P::Sponge>,
        r1cs: &R1CS<Base<P>>,
        commitments: Vec<WhirR1CSCommitment<P>>,
        full_witness: Vec<Base<P>>,
        public_inputs: &PublicInputs<Base<P>>,
    ) -> Result<WhirR1CSProof>;

    /// Like [`Self::prove_noir`], but also produces the SPARK query batch as
    /// a side output (the transcript is identical either way).
    fn prove_noir_with_spark(
        &self,
        merlin: ProverState<P::Sponge>,
        r1cs: &R1CS<Base<P>>,
        commitments: Vec<WhirR1CSCommitment<P>>,
        full_witness: Vec<Base<P>>,
        public_inputs: &PublicInputs<Base<P>>,
    ) -> Result<(WhirR1CSProof, SparkQueryData<P>)>;
}

impl<P: FieldHash> WhirR1CSProver<P> for WhirR1CSScheme<P>
where
    Base<P>: snarkvm_utilities::Uniform,
    Ext<P>: snarkvm_utilities::Uniform,
{
    fn commit(
        &self,
        merlin: &mut ProverState<P::Sponge>,
        num_witnesses: usize,
        num_constraints: usize,
        witness: Vec<Base<P>>,
        is_w1: bool,
    ) -> Result<WhirR1CSCommitment<P>> {
        let witness_size = if is_w1 { self.w1_size } else { num_witnesses - self.w1_size };

        ensure!(witness.len() == witness_size, "Unexpected witness length for R1CS instance");
        ensure!(witness_size <= self.domain_size(), "R1CS witness length exceeds scheme capacity");
        ensure!(num_constraints <= 1 << self.m_0, "R1CS constraints exceed scheme capacity");

        let num_vars = self.whir_witness.initial_num_variables();
        let target_len = 1usize << num_vars;

        let mut padded_witness = pad_to_power_of_two(witness);
        if padded_witness.len() < target_len {
            padded_witness.resize(target_len, <Base<P>>::zero());
        }

        // Commit the base-field witness directly (non-hiding — openings leak
        // witness values; see the `whir_witness` field docs).
        let witness_commitment = self.whir_witness.commit(merlin, &[&padded_witness]);

        // Commit the Spartan sumcheck blinding `g` separately, natively in the
        // extension field. Transcript order: this commitment is absorbed
        // immediately after the witness commitment (mirrored in the verifier).
        let blinding = if is_w1 {
            let g = generate_blinding_univariates::<Ext<P>>(self.m_0);
            let blind_len = self.blinding_domain_size();
            let mut g_vector: Vec<Ext<P>> = g.iter().flatten().copied().collect();
            g_vector.resize(blind_len, <Ext<P>>::zero());
            let blinding_witness = self.whir_blinding.commit(merlin, &[&g_vector]);
            Some(BlindingState { polynomial: g, vector: g_vector, witness: blinding_witness })
        } else {
            None
        };

        Ok(WhirR1CSCommitment { witness: witness_commitment, polynomial: padded_witness, blinding })
    }

    fn prove_noir(
        &self,
        merlin: ProverState<P::Sponge>,
        r1cs: &R1CS<Base<P>>,
        commitments: Vec<WhirR1CSCommitment<P>>,
        full_witness: Vec<Base<P>>,
        public_inputs: &PublicInputs<Base<P>>,
    ) -> Result<WhirR1CSProof> {
        let (proof, _) = prove_noir_inner(self, merlin, r1cs, commitments, full_witness, public_inputs, false)?;
        Ok(proof)
    }

    fn prove_noir_with_spark(
        &self,
        merlin: ProverState<P::Sponge>,
        r1cs: &R1CS<Base<P>>,
        commitments: Vec<WhirR1CSCommitment<P>>,
        full_witness: Vec<Base<P>>,
        public_inputs: &PublicInputs<Base<P>>,
    ) -> Result<(WhirR1CSProof, SparkQueryData<P>)> {
        let (proof, queries) = prove_noir_inner(self, merlin, r1cs, commitments, full_witness, public_inputs, true)?;
        let queries = queries.ok_or_else(|| anyhow::anyhow!("SPARK query data must be produced when requested"))?;
        Ok((proof, queries))
    }
}

fn prove_noir_inner<P: FieldHash>(
    scheme: &WhirR1CSScheme<P>,
    mut merlin: ProverState<P::Sponge>,
    r1cs: &R1CS<Base<P>>,
    commitments: Vec<WhirR1CSCommitment<P>>,
    full_witness: Vec<Base<P>>,
    public_inputs: &PublicInputs<Base<P>>,
    produce_spark_query: bool,
) -> Result<(WhirR1CSProof, Option<SparkQueryData<P>>)>
where
    Base<P>: snarkvm_utilities::Uniform,
    Ext<P>: snarkvm_utilities::Uniform,
{
    ensure!(!commitments.is_empty(), "Need at least one commitment");

    let (a, b, c) = calculate_witness_bounds(r1cs, &full_witness);
    drop(full_witness);

    let embedding = <P::Embedding>::default();

    let blinding = commitments[0].blinding.as_ref().ok_or_else(|| anyhow::anyhow!("c1 must carry blinding state"))?;

    // The witness bounds stay in the base field for the first sumcheck
    // round; the fold at the first challenge lifts them into the
    // extension. `g` is native ext, committed separately; `blinding_eval`
    // opens the ext blinding vector (which holds `g` flattened at
    // offset 0).
    let (alpha, blinding_eval) =
        run_zk_sumcheck_prover(&embedding, [a, b, c], &mut merlin, scheme.m_0, &blinding.polynomial, &blinding.vector);

    let (at, bt, ct) = transpose_r1cs_matrices(r1cs);
    let alphas = multiply_transposed_by_eq_alpha(&embedding, &at, &bt, &ct, &alpha, r1cs);

    let blinding_weights = expand_powers::<4, _>(&alpha);
    prove_from_alphas_ctx(
        scheme,
        merlin,
        ProveFromAlphasCtx {
            alphas,
            blinding_eval,
            blinding_weights,
            commitments,
            spark_row: produce_spark_query.then_some(alpha),
        },
        public_inputs,
    )
}

/// Owned inputs to the post-sumcheck proving stage ([`prove_from_alphas_ctx`]).
pub struct ProveFromAlphasCtx<P: ProofField> {
    pub alphas: [Vec<Ext<P>>; 3],
    pub blinding_eval: Ext<P>,
    pub blinding_weights: Vec<Ext<P>>,
    pub commitments: Vec<WhirR1CSCommitment<P>>,
    /// When set, SPARK query data is produced with this Spartan sumcheck
    /// point as the shared row axis.
    pub spark_row: Option<Vec<Ext<P>>>,
}

pub fn prove_from_alphas<P: FieldHash>(
    scheme: &WhirR1CSScheme<P>,
    merlin: ProverState<P::Sponge>,
    alphas: [Vec<Ext<P>>; 3],
    blinding_eval: Ext<P>,
    blinding_weights: Vec<Ext<P>>,
    commitments: Vec<WhirR1CSCommitment<P>>,
    public_inputs: &PublicInputs<Base<P>>,
) -> Result<WhirR1CSProof>
where
    Base<P>: snarkvm_utilities::Uniform,
    Ext<P>: snarkvm_utilities::Uniform,
{
    let (proof, _) = prove_from_alphas_ctx(
        scheme,
        merlin,
        ProveFromAlphasCtx { alphas, blinding_eval, blinding_weights, commitments, spark_row: None },
        public_inputs,
    )?;
    Ok(proof)
}

pub fn prove_from_alphas_ctx<P: FieldHash>(
    scheme: &WhirR1CSScheme<P>,
    mut merlin: ProverState<P::Sponge>,
    ctx: ProveFromAlphasCtx<P>,
    public_inputs: &PublicInputs<Base<P>>,
) -> Result<(WhirR1CSProof, Option<SparkQueryData<P>>)>
where
    Base<P>: snarkvm_utilities::Uniform,
    Ext<P>: snarkvm_utilities::Uniform,
{
    let ProveFromAlphasCtx { alphas, blinding_eval, blinding_weights, mut commitments, spark_row } = ctx;

    let public_inputs_hash = P::hash_public_inputs(scheme.hash_config, &public_inputs.0);
    let public_inputs_len = public_inputs.len();

    // The ext blinding commitment lives on c0; pull it out before the base
    // commitments are consumed below. It is opened at the very end, after all
    // base-witness opens (mirrored in the verifier).
    let blinding_state =
        commitments[0].blinding.take().ok_or_else(|| anyhow::anyhow!("c0 must carry blinding state"))?;

    let is_single = commitments.len() == 1;
    let (x, public_weight) = get_public_weights(public_inputs_hash, public_inputs_len, &mut merlin, scheme.m);

    // Witness is base, covectors/evaluations are ext: evaluations use mixed
    // products through this embedding (a no-op under `Identity`).
    let embedding = <P::Embedding>::default();

    let spark_queries: Option<SparkQueryData<P>> = if is_single {
        // Single commitment path
        let commitment =
            commitments.into_iter().next().expect("single-commitment path requires at least one commitment");
        let (mut weights, evals) =
            create_weights_and_evaluations::<3, _>(&embedding, scheme.m, &commitment.polynomial, alphas);

        for eval in &evals {
            merlin.prover_field(eval);
        }

        // Snapshot the three alpha covectors (A, B, C) before the public
        // weight is inserted; they are re-evaluated at the final WHIR point
        // to form the SPARK query.
        let spark_weights: Option<Vec<(Vec<Ext<P>>, usize)>> =
            spark_row.as_ref().map(|_| weights.iter().map(|w| (w.vector().to_vec(), w.size())).collect());

        if public_inputs_len > 0 {
            let public_eval =
                compute_public_weight_evaluation(&embedding, &mut weights, &commitment.polynomial, public_weight);
            merlin.prover_field(&public_eval);
        }

        let evaluations = compute_evaluations(&embedding, &weights, &commitment.polynomial);

        let boxed_weights: Vec<Box<dyn LinearForm<Ext<P>>>> =
            weights.into_iter().map(|w| Box::new(w) as Box<dyn LinearForm<Ext<P>>>).collect();

        let final_claim = scheme.whir_witness.prove(
            &mut merlin,
            vec![Cow::Borrowed(commitment.polynomial.as_slice())],
            vec![Cow::Owned(commitment.witness)],
            boxed_weights,
            Cow::Borrowed(&evaluations),
        );

        spark_row.zip(spark_weights).map(|(row, spark_weights)| {
            let [claimed_a, claimed_b, claimed_c] =
                evaluate_spark_weights(spark_weights, &final_claim.evaluation_point);
            SparkQueryData {
                row,
                queries: vec![SparkColQueryData { col: final_claim.evaluation_point, claimed_a, claimed_b, claimed_c }],
            }
        })
    } else {
        // Dual commitment path
        let mut commitments = commitments.into_iter();
        let c1 = commitments.next().expect("dual-commitment path requires first commitment");
        let c2 = commitments.next().expect("dual-commitment path requires second commitment");

        let (alphas_1, alphas_2): (Vec<_>, Vec<_>) = alphas
            .into_iter()
            .map(|mut v| {
                let v2 = v.split_off(scheme.w1_size);
                (v, v2)
            })
            .unzip();

        let alphas_1: [Vec<Ext<P>>; 3] = alphas_1.try_into().expect("alphas_1 must have exactly 3 elements");
        let alphas_2: [Vec<Ext<P>>; 3] = alphas_2.try_into().expect("alphas_2 must have exactly 3 elements");

        let evals_1 = compute_alpha_evals(&embedding, &c1.polynomial, &alphas_1);
        let evals_2 = compute_alpha_evals(&embedding, &c2.polynomial, &alphas_2);
        for eval in &evals_1 {
            merlin.prover_field(eval);
        }
        for eval in &evals_2 {
            merlin.prover_field(eval);
        }

        let public_1 = if public_inputs_len > 0 {
            let p1 = compute_public_eval(&embedding, x, public_inputs_len, &c1.polynomial);
            merlin.prover_field(&p1);
            Some(p1)
        } else {
            None
        };

        // Challenge binding: prove that w2 contains the correct Fiat-Shamir
        // challenge values at the expected positions.
        let challenge_eval = if !scheme.challenge_offsets.is_empty() {
            let ce = compute_challenge_eval(&embedding, x, &scheme.challenge_offsets, &c2.polynomial);
            merlin.prover_field(&ce);
            Some(ce)
        } else {
            None
        };

        let WhirR1CSCommitment { witness: w1, polynomial: p1, .. } = c1;
        let (final_claim_1, claimed_1) = {
            let mut weights = build_prefix_covectors(scheme.m, alphas_1);

            // Snapshot the three alpha covectors (A, B, C) before the public
            // weight is inserted; they are re-evaluated at the final WHIR
            // point to form the SPARK query.
            let spark_weights: Option<Vec<(Vec<Ext<P>>, usize)>> =
                spark_row.as_ref().map(|_| weights.iter().map(|w| (w.vector().to_vec(), w.size())).collect());

            let mut evaluations: Vec<Ext<P>> = Vec::new();
            if let Some(pe) = public_1 {
                weights.insert(0, make_public_weight(x, public_inputs_len, scheme.m));
                evaluations.push(pe);
            }
            evaluations.extend_from_slice(&evals_1);

            let boxed_weights: Vec<Box<dyn LinearForm<Ext<P>>>> =
                weights.into_iter().map(|w| Box::new(w) as Box<dyn LinearForm<Ext<P>>>).collect();

            let final_claim = scheme.whir_witness.prove(
                &mut merlin,
                vec![Cow::Borrowed(p1.as_slice())],
                vec![Cow::Owned(w1)],
                boxed_weights,
                Cow::Borrowed(&evaluations),
            );

            let claimed = spark_weights.map(|sw| evaluate_spark_weights(sw, &final_claim.evaluation_point));
            (final_claim, claimed)
        };
        drop(p1);

        let WhirR1CSCommitment { witness: w2, polynomial: p2, .. } = c2;
        let (final_claim_2, claimed_2) = {
            let weights = build_prefix_covectors(scheme.m, alphas_2);

            // The first three weights are the alpha covectors (A, B, C); the
            // challenge weight, if any, is appended after them.
            let spark_weights: Option<Vec<(Vec<Ext<P>>, usize)>> =
                spark_row.as_ref().map(|_| weights.iter().map(|w| (w.vector().to_vec(), w.size())).collect());

            let mut evaluations: Vec<Ext<P>> = evals_2;

            let mut boxed_weights: Vec<Box<dyn LinearForm<Ext<P>>>> =
                weights.into_iter().map(|w| Box::new(w) as Box<dyn LinearForm<Ext<P>>>).collect();

            if let Some(ce) = challenge_eval {
                let cw = make_challenge_weight(x, &scheme.challenge_offsets, scheme.m);
                evaluations.push(ce);
                boxed_weights.push(Box::new(cw));
            }

            let final_claim = scheme.whir_witness.prove(
                &mut merlin,
                vec![Cow::Borrowed(p2.as_slice())],
                vec![Cow::Owned(w2)],
                boxed_weights,
                Cow::Borrowed(&evaluations),
            );

            let claimed = spark_weights.map(|sw| evaluate_spark_weights(sw, &final_claim.evaluation_point));
            (final_claim, claimed)
        };

        // The SPARK column axis spans w1 | w2: prefix the per-commitment WHIR
        // points with a selector coordinate (0 selects w1, 1 selects w2).
        spark_row.zip(claimed_1.zip(claimed_2)).map(|(row, ([a1, b1, c1], [a2, b2, c2]))| {
            let mut col_1 = final_claim_1.evaluation_point;
            col_1.insert(0, <Ext<P>>::zero());
            let query_1 = SparkColQueryData { col: col_1, claimed_a: a1, claimed_b: b1, claimed_c: c1 };

            let mut col_2 = final_claim_2.evaluation_point;
            col_2.insert(0, <Ext<P>>::one());
            let query_2 = SparkColQueryData { col: col_2, claimed_a: a2, claimed_b: b2, claimed_c: c2 };

            SparkQueryData { row, queries: vec![query_1, query_2] }
        })
    };

    // Open the ext blinding commitment: prove `blinding_eval` is the evaluation
    // of the committed blinding vector (g flattened at offset 0) against the
    // sumcheck power covector. Sent after all base-witness opens (mirrored in
    // the verifier).
    {
        let blind_domain = scheme.blinding_domain_size();
        let blinding_covector = OffsetCovector::new(blinding_weights, 0, blind_domain);
        let _ = scheme.whir_blinding.prove(
            &mut merlin,
            vec![Cow::Borrowed(blinding_state.vector.as_slice())],
            vec![Cow::Owned(blinding_state.witness)],
            vec![Box::new(blinding_covector) as Box<dyn LinearForm<Ext<P>>>],
            Cow::Borrowed(&[blinding_eval]),
        );
    }

    let proof = merlin.proof();
    Ok((
        WhirR1CSProof {
            narg_string: proof.narg_string,
            hints: proof.hints,
            #[cfg(debug_assertions)]
            pattern: proof.pattern,
        },
        spark_queries,
    ))
}

/// Re-evaluate snapshotted alpha covectors (as `(prefix, domain_size)` pairs)
/// at the final WHIR evaluation point, yielding the claimed A, B, C matrix
/// evaluations for a SPARK query.
fn evaluate_spark_weights<F: Field>(spark_weights: Vec<(Vec<F>, usize)>, evaluation_point: &[F]) -> [F; 3] {
    let evals: Vec<F> = spark_weights
        .into_iter()
        .take(3)
        .map(|(vector, domain_size)| PrefixCovector::new(vector, domain_size).mle_evaluate(evaluation_point))
        .collect();
    evals.try_into().unwrap_or_else(|_| panic!("exactly 3 alpha-weight evaluations"))
}

pub fn compute_blinding_coefficients_for_round<F: Field>(
    g_univariates: &[[F; 4]],
    compute_for: usize,
    alphas: &[F],
) -> [F; 4] {
    let mut compute_for = compute_for;
    let n = g_univariates.len();
    assert!(compute_for <= n);
    assert_eq!(alphas.len(), compute_for);
    let mut all_fixed = false;
    if compute_for == n {
        all_fixed = true;
        compute_for = n - 1;
    }

    // p = Σ_{i<r} g_i(α_i)
    let mut prefix_sum = F::zero();
    for i in 0..compute_for {
        prefix_sum += eval_cubic_poly(g_univariates[i], alphas[i]);
    }

    // s = Σ_{i>r}(g_i(0) + g_i(1))
    let mut suffix_sum = F::zero();
    for g_coeffs in g_univariates.iter().skip(compute_for + 1) {
        suffix_sum += eval_cubic_poly(*g_coeffs, F::zero()) + eval_cubic_poly(*g_coeffs, F::one());
    }

    let two = F::one() + F::one();
    let mut prefix_multiplier = F::one();
    for _ in 0..(n - 1 - compute_for) {
        prefix_multiplier = prefix_multiplier + prefix_multiplier;
    }
    let suffix_multiplier = prefix_multiplier / two;

    let constant_term_from_other_items = prefix_multiplier * prefix_sum + suffix_multiplier * suffix_sum;

    let coefficient_for_current_index = &g_univariates[compute_for];

    if all_fixed {
        let value = eval_cubic_poly(
            [
                prefix_multiplier * coefficient_for_current_index[0] + constant_term_from_other_items,
                prefix_multiplier * coefficient_for_current_index[1],
                prefix_multiplier * coefficient_for_current_index[2],
                prefix_multiplier * coefficient_for_current_index[3],
            ],
            alphas[compute_for],
        );
        return [value, F::zero(), F::zero(), F::zero()];
    }

    [
        prefix_multiplier * coefficient_for_current_index[0] + constant_term_from_other_items,
        prefix_multiplier * coefficient_for_current_index[1],
        prefix_multiplier * coefficient_for_current_index[2],
        prefix_multiplier * coefficient_for_current_index[3],
    ]
}

pub fn sum_over_hypercube<F: Field>(g_univariates: &[[F; 4]]) -> F {
    let fixed_variables: &[F] = &[];
    let polynomial_coefficient = compute_blinding_coefficients_for_round(g_univariates, 0, fixed_variables);

    eval_cubic_poly(polynomial_coefficient, F::zero()) + eval_cubic_poly(polynomial_coefficient, F::one())
}

fn generate_blinding_univariates<F: Field>(m_0: usize) -> Vec<[F; 4]> {
    let mut rng = rand::rng();
    (0..m_0).map(|_| std::array::from_fn(|_| F::rand(&mut rng))).collect()
}

#[inline]
pub fn pad_to_pow2_len_min2<F: Field>(v: &mut Vec<F>) {
    let target = v.len().max(2).next_power_of_two();
    if v.len() < target {
        v.resize(target, F::zero());
    }
}

/// Run the blinded Spartan sumcheck. The first round consumes the base-field
/// mles directly; the fold at the first challenge lifts them into the
/// extension, where the remaining rounds run.
pub fn run_zk_sumcheck_prover<M, S>(
    embedding: &M,
    mles: [Vec<M::Source>; 3],
    merlin: &mut ProverState<S>,
    m_0: usize,
    blinding_polynomial: &[[M::Target; 4]],
    blinding_vector: &[M::Target],
) -> (Vec<M::Target>, M::Target)
where
    M: Embedding,
    FieldElem<M::Target>: Codec,
    S: DuplexSpongeInterface<U = u8>,
{
    let [mut a, mut b, mut c] = mles;
    let r: Vec<M::Target> = merlin.verifier_field_vec(m_0);
    let mut eq = calculate_evaluations_over_boolean_hypercube_for_eq(&r, 1 << r.len());

    pad_to_pow2_len_min2(&mut a);
    pad_to_pow2_len_min2(&mut b);
    pad_to_pow2_len_min2(&mut c);
    pad_to_pow2_len_min2(&mut eq);

    let mut alpha = Vec::<M::Target>::with_capacity(m_0);

    let sum_g_reduce = sum_over_hypercube(blinding_polynomial);

    merlin.prover_field(&sum_g_reduce);

    let rho: M::Target = merlin.verifier_field();

    // Prove that sum of F + ρ·G over the boolean hypercube equals ρ·Σ(G).
    let mut saved_val_for_sumcheck_equality_assertion = rho * sum_g_reduce;

    // 2 is invertible in any field of odd characteristic; precompute 1/2 once
    // and reuse it for each round's cubic-coefficient solve.
    let half = M::Target::one() / (M::Target::one() + M::Target::one());

    // First round: a, b, c are base-field, so the constraint products stay in
    // the base field with one mixed mul per point against the ext eq.
    let hhat = mixed_sumcheck_map_reduce([&a[..], &b[..], &c[..]], &eq, |[a, b, c], eq| {
        let f0 = embedding.mixed_mul(eq.0, a.0 * b.0 - c.0);
        let f_em1 = embedding.mixed_mul(eq.0 + eq.0 - eq.1, (a.0 + a.0 - a.1) * (b.0 + b.0 - b.1) - (c.0 + c.0 - c.1));
        let f_inf = embedding.mixed_mul(eq.1 - eq.0, (a.1 - a.0) * (b.1 - b.0));

        [f0, f_em1, f_inf]
    });
    let g_poly = compute_blinding_coefficients_for_round(blinding_polynomial, 0, &[]);
    let (alpha_0, saved_val) =
        combined_round_message(merlin, hhat, g_poly, rho, half, saved_val_for_sumcheck_equality_assertion);
    saved_val_for_sumcheck_equality_assertion = saved_val;
    alpha.push(alpha_0);

    // Fold at the first challenge, lifting the mles into the extension.
    let folded_a = mixed_fold(embedding, &a, alpha_0);
    let folded_b = mixed_fold(embedding, &b, alpha_0);
    let folded_c = mixed_fold(embedding, &c, alpha_0);
    let folded_eq = mixed_fold(&Identity::new(), &eq, alpha_0);
    drop((a, b, c, eq));
    let (mut a, mut b, mut c, mut eq) = (folded_a, folded_b, folded_c, folded_eq);

    let mut fold = None;

    for idx in 1..m_0 {
        let hhat = sumcheck_fold_map_reduce([&mut a, &mut b, &mut c, &mut eq], fold, |[a, b, c, eq]| {
            let f0 = eq.0 * (a.0 * b.0 - c.0);
            let f_em1 = (eq.0 + eq.0 - eq.1) * ((a.0 + a.0 - a.1) * (b.0 + b.0 - b.1) - (c.0 + c.0 - c.1));
            let f_inf = (eq.1 - eq.0) * (a.1 - a.0) * (b.1 - b.0);

            [f0, f_em1, f_inf]
        });
        if fold.is_some() {
            a.truncate(a.len() / 2);
            b.truncate(b.len() / 2);
            c.truncate(c.len() / 2);
            eq.truncate(eq.len() / 2);
        }

        let g_poly = compute_blinding_coefficients_for_round(blinding_polynomial, idx, alpha.as_slice());
        let (alpha_i, saved_val) =
            combined_round_message(merlin, hhat, g_poly, rho, half, saved_val_for_sumcheck_equality_assertion);
        saved_val_for_sumcheck_equality_assertion = saved_val;
        alpha.push(alpha_i);

        fold = Some(alpha_i);
    }
    drop((a, b, c, eq));

    let weight_vec = expand_powers::<4, _>(alpha.as_slice());
    let blinding_eval = dot(&weight_vec, &blinding_vector[..weight_vec.len()]);
    merlin.prover_field(&blinding_eval);

    (alpha, blinding_eval)
}

/// Absorb the combined round polynomial `hhat_i + ρ·g_i` into the transcript
/// and draw the round challenge; returns it with the sumcheck equality value
/// for the next round.
fn combined_round_message<F: Field, S: DuplexSpongeInterface<U = u8>>(
    merlin: &mut ProverState<S>,
    hhat: [F; 3],
    g_poly: [F; 4],
    rho: F,
    half: F,
    saved_val_for_sumcheck_equality_assertion: F,
) -> (F, F)
where
    FieldElem<F>: Codec,
{
    let [hhat_i_at_0, hhat_i_at_em1, hhat_i_at_inf_over_x_cube] = hhat;

    let mut combined_hhat_i_coeffs = [F::zero(); 4];

    combined_hhat_i_coeffs[0] = hhat_i_at_0 + rho * g_poly[0];

    let g_at_minus_one = g_poly[0] - g_poly[1] + g_poly[2] - g_poly[3];
    let combined_at_em1 = hhat_i_at_em1 + rho * g_at_minus_one;

    combined_hhat_i_coeffs[2] = (saved_val_for_sumcheck_equality_assertion + combined_at_em1
        - combined_hhat_i_coeffs[0]
        - combined_hhat_i_coeffs[0]
        - combined_hhat_i_coeffs[0])
        * half;

    combined_hhat_i_coeffs[3] = hhat_i_at_inf_over_x_cube + rho * g_poly[3];

    combined_hhat_i_coeffs[1] = saved_val_for_sumcheck_equality_assertion
        - combined_hhat_i_coeffs[0]
        - combined_hhat_i_coeffs[0]
        - combined_hhat_i_coeffs[3]
        - combined_hhat_i_coeffs[2];

    assert_eq!(
        saved_val_for_sumcheck_equality_assertion,
        combined_hhat_i_coeffs[0]
            + combined_hhat_i_coeffs[0]
            + combined_hhat_i_coeffs[1]
            + combined_hhat_i_coeffs[2]
            + combined_hhat_i_coeffs[3]
    );

    for coeff in &combined_hhat_i_coeffs {
        merlin.prover_field(coeff);
    }
    let alpha_i: F = merlin.verifier_field();

    (alpha_i, eval_cubic_poly(combined_hhat_i_coeffs, alpha_i))
}

fn create_weights_and_evaluations<const N: usize, M: Embedding>(
    embedding: &M,
    m: usize,
    polynomial: &[M::Source],
    alphas: [Vec<M::Target>; N],
) -> (Vec<PrefixCovector<M::Target>>, Vec<M::Target>) {
    let domain_size = 1usize << m;

    let mut weights = Vec::with_capacity(N);
    let mut evals = Vec::with_capacity(N);

    for mut w in alphas {
        let base_len = w.len().next_power_of_two().max(2);
        w.resize(base_len, M::Target::zero());

        evals.push(mixed_dot(embedding, &w, &polynomial[..base_len]));
        weights.push(PrefixCovector::new(w, domain_size));
    }

    (weights, evals)
}

fn compute_evaluations<M: Embedding>(
    embedding: &M,
    weights: &[PrefixCovector<M::Target>],
    polynomial: &[M::Source],
) -> Vec<M::Target> {
    weights.iter().map(|w| mixed_dot(embedding, w.vector(), &polynomial[..w.vector().len()])).collect()
}

fn compute_public_weight_evaluation<M: Embedding>(
    embedding: &M,
    weights: &mut Vec<PrefixCovector<M::Target>>,
    polynomial: &[M::Source],
    public_weights: PrefixCovector<M::Target>,
) -> M::Target {
    let n = public_weights.vector().len();
    let eval = mixed_dot(embedding, public_weights.vector(), &polynomial[..n]);
    weights.insert(0, public_weights);
    eval
}

fn get_public_weights<F: Field, S: DuplexSpongeInterface<U = u8>>(
    public_inputs_hash: F,
    public_inputs_len: usize,
    merlin: &mut ProverState<S>,
    m: usize,
) -> (F, PrefixCovector<F>)
where
    FieldElem<F>: Codec,
{
    merlin.prover_field(&public_inputs_hash);

    let x: F = merlin.verifier_field();

    (x, make_public_weight(x, public_inputs_len, m))
}
