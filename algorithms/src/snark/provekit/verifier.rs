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
        ProofField,
        PublicInputs,
        R1CS,
        WhirR1CSProof,
        WhirR1CSScheme,
        prefix_covector::{
            OffsetCovector,
            build_prefix_covectors,
            expand_powers,
            make_challenge_weight,
            make_public_weight,
        },
        utils::sumcheck::{calculate_eq, eval_cubic_poly, multiply_transposed_by_eq_alpha, transpose_r1cs_matrices},
    },
    whir::{
        algebra::{embedding::Embedding, linear_form::LinearForm},
        transcript::{Codec, DuplexSpongeInterface, FieldElem, Proof, VerifierMessage, VerifierState},
    },
};
use anyhow::{Context, Result, ensure};
use snarkvm_fields::{Field, One};

#[derive(Debug)]
pub struct DataFromSumcheckVerifier<F: Field> {
    r: Vec<F>,
    alpha: Vec<F>,
    blinding_eval: F,
    f_at_alpha: F,
}

pub trait WhirR1CSVerifier<P: ProofField> {
    fn verify(&self, proof: &WhirR1CSProof, public_inputs: &PublicInputs<Base<P>>, r1cs: &R1CS<Base<P>>) -> Result<()>;
}

impl<P: FieldHash> WhirR1CSVerifier<P> for WhirR1CSScheme<P> {
    fn verify(&self, proof: &WhirR1CSProof, public_inputs: &PublicInputs<Base<P>>, r1cs: &R1CS<Base<P>>) -> Result<()> {
        let actual_r1cs_hash = r1cs.hash();
        anyhow::ensure!(
            self.r1cs_hash == actual_r1cs_hash,
            "R1CS hash mismatch: scheme expects {:?}, got {:?}",
            self.r1cs_hash,
            actual_r1cs_hash
        );

        let public_inputs_hash = P::hash_public_inputs(self.hash_config, &public_inputs.0);
        let instance = P::ext_to_bytes_le(&public_inputs_hash);
        let ds = self.create_domain_separator().instance(&instance);
        let whir_proof = Proof {
            narg_string: proof.narg_string.clone(),
            hints: proof.hints.clone(),
            #[cfg(debug_assertions)]
            pattern: proof.pattern.clone(),
        };
        let mut arthur = VerifierState::new(&ds, &whir_proof, P::transcript_sponge(self.hash_config));

        let commitment_1 = self
            .whir_witness
            .receive_commitment(&mut arthur)
            .map_err(|_| anyhow::anyhow!("Failed to parse commitment 1"))?;

        // Mirror the prover: the ext blinding commitment is absorbed immediately
        // after the (w1) witness commitment, before any challenge sampling.
        let blinding_commitment = self
            .whir_blinding
            .receive_commitment(&mut arthur)
            .map_err(|_| anyhow::anyhow!("Failed to parse blinding commitment"))?;

        let (commitment_2, logup_challenges) = if self.num_challenges > 0 {
            let challenges: Vec<Ext<P>> = arthur.verifier_field_vec(self.num_challenges);
            let commitment = self
                .whir_witness
                .receive_commitment(&mut arthur)
                .map_err(|_| anyhow::anyhow!("Failed to parse commitment 2"))?;
            (Some(commitment), Some(challenges))
        } else {
            (None, None)
        };
        let (transposed, sumcheck_result) =
            rayon::join(|| transpose_r1cs_matrices(r1cs), || run_sumcheck_verifier::<Ext<P>, _>(&mut arthur, self.m_0));
        let data_from_sumcheck_verifier = sumcheck_result.context("while verifying sumcheck")?;
        let (at, bt, ct) = transposed;

        let public_inputs_hash_buf: Ext<P> =
            arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read public inputs hash"))?;
        ensure!(
            public_inputs_hash_buf == public_inputs_hash,
            "Public inputs hash mismatch: expected {public_inputs_hash:?}, got {public_inputs_hash_buf:?}"
        );
        let x: Ext<P> = arthur.verifier_field();

        let embedding = <P::Embedding>::default();
        let alphas =
            multiply_transposed_by_eq_alpha(&embedding, &at, &bt, &ct, &data_from_sumcheck_verifier.alpha, r1cs);

        let blinding_eval = data_from_sumcheck_verifier.blinding_eval;
        let blinding_weights = expand_powers::<4, _>(&data_from_sumcheck_verifier.alpha);

        let (az_at_alpha, bz_at_alpha, cz_at_alpha) = if let Some(commitment_2) = commitment_2 {
            let (alphas_1, alphas_2): (Vec<_>, Vec<_>) = alphas
                .into_iter()
                .map(|mut v| {
                    let v2 = v.split_off(self.w1_size);
                    (v, v2)
                })
                .unzip();
            let alphas_1: [Vec<Ext<P>>; 3] =
                alphas_1.try_into().map_err(|_| anyhow::anyhow!("Expected 3 alpha vectors for commitment 1"))?;
            let alphas_2: [Vec<Ext<P>>; 3] =
                alphas_2.try_into().map_err(|_| anyhow::anyhow!("Expected 3 alpha vectors for commitment 2"))?;

            let evals_1: [Ext<P>; 3] = [
                arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read evals_1[0]"))?,
                arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read evals_1[1]"))?,
                arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read evals_1[2]"))?,
            ];
            let evals_2: [Ext<P>; 3] = [
                arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read evals_2[0]"))?,
                arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read evals_2[1]"))?,
                arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read evals_2[2]"))?,
            ];

            let mut weights_1 = build_prefix_covectors(self.m, alphas_1);
            let weights_2 = build_prefix_covectors(self.m, alphas_2);

            let evaluations_1 = if !public_inputs.is_empty() {
                let public_1: Ext<P> = arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read public_1"))?;
                verify_public_input_binding(&embedding, public_1, x, public_inputs)?;
                weights_1.insert(0, make_public_weight(x, public_inputs.len(), self.m));
                vec![public_1, evals_1[0], evals_1[1], evals_1[2]]
            } else {
                evals_1.to_vec()
            };
            let mut evaluations_2 = evals_2.to_vec();

            // Challenge binding: verify that w2 contains the correct
            // Fiat-Shamir challenge values at the expected positions.
            let challenge_covector = if let Some(ref challenges) = logup_challenges {
                let challenge_eval: Ext<P> =
                    arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read challenge_eval"))?;
                verify_challenge_binding(challenge_eval, x, challenges)?;
                let cw = make_challenge_weight(x, &self.challenge_offsets, self.m);
                evaluations_2.push(challenge_eval);
                Some(cw)
            } else {
                None
            };

            let weight_refs_1: Vec<&dyn LinearForm<Ext<P>>> =
                weights_1.iter().map(|w| w as &dyn LinearForm<Ext<P>>).collect();

            let fc_1 = self
                .whir_witness
                .verify(&mut arthur, &[&commitment_1], &evaluations_1)
                .map_err(|_| anyhow::anyhow!("WHIR verification failed for c1"))?;
            fc_1.verify(weight_refs_1.iter().copied())
                .map_err(|_| anyhow::anyhow!("WHIR final-claim check failed for c1"))?;

            let mut weight_refs_2: Vec<&dyn LinearForm<Ext<P>>> =
                weights_2.iter().map(|w| w as &dyn LinearForm<Ext<P>>).collect();
            if let Some(ref cw) = challenge_covector {
                weight_refs_2.push(cw as &dyn LinearForm<Ext<P>>);
            }
            let fc_2 = self
                .whir_witness
                .verify(&mut arthur, &[&commitment_2], &evaluations_2)
                .map_err(|_| anyhow::anyhow!("WHIR verification failed for c2"))?;
            fc_2.verify(weight_refs_2.iter().copied())
                .map_err(|_| anyhow::anyhow!("WHIR final-claim check failed for c2"))?;

            (evals_1[0] + evals_2[0], evals_1[1] + evals_2[1], evals_1[2] + evals_2[2])
        } else {
            let evals: [Ext<P>; 3] = [
                arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read evals[0]"))?,
                arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read evals[1]"))?,
                arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read evals[2]"))?,
            ];

            let mut weights = build_prefix_covectors(self.m, alphas);

            let evaluations = if !public_inputs.is_empty() {
                let public_eval: Ext<P> =
                    arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read public eval"))?;
                verify_public_input_binding(&embedding, public_eval, x, public_inputs)?;
                weights.insert(0, make_public_weight(x, public_inputs.len(), self.m));
                vec![public_eval, evals[0], evals[1], evals[2]]
            } else {
                evals.to_vec()
            };

            let weight_refs: Vec<&dyn LinearForm<Ext<P>>> =
                weights.iter().map(|w| w as &dyn LinearForm<Ext<P>>).collect();

            let fc = self
                .whir_witness
                .verify(&mut arthur, &[&commitment_1], &evaluations)
                .map_err(|_| anyhow::anyhow!("WHIR verification failed"))?;
            fc.verify(weight_refs.iter().copied()).map_err(|_| anyhow::anyhow!("WHIR final-claim check failed"))?;

            (evals[0], evals[1], evals[2])
        };

        // Open the ext blinding commitment: verify `blinding_eval` against the
        // sumcheck power covector over the committed blinding vector (g at
        // offset 0). Mirrors the prover's single blinding open after all base
        // opens.
        {
            let blind_domain = self.blinding_domain_size();
            let blinding_covector = OffsetCovector::new(blinding_weights, 0, blind_domain);
            let fc_b = self
                .whir_blinding
                .verify(&mut arthur, &[&blinding_commitment], &[blinding_eval])
                .map_err(|_| anyhow::anyhow!("WHIR verification failed for blinding"))?;
            fc_b.verify(std::iter::once(&blinding_covector as &dyn LinearForm<Ext<P>>))
                .map_err(|_| anyhow::anyhow!("WHIR final-claim check failed for blinding"))?;
        }

        ensure!(
            data_from_sumcheck_verifier.f_at_alpha
                == (az_at_alpha * bz_at_alpha - cz_at_alpha)
                    * calculate_eq(&data_from_sumcheck_verifier.r, &data_from_sumcheck_verifier.alpha),
            "last sumcheck value does not match"
        );

        arthur.check_eof().map_err(|_| anyhow::anyhow!("Proof contains unparsed trailing bytes"))
    }
}

pub fn run_sumcheck_verifier<F: Field, S: DuplexSpongeInterface<U = u8>>(
    arthur: &mut VerifierState<'_, S>,
    m_0: usize,
) -> Result<DataFromSumcheckVerifier<F>>
where
    FieldElem<F>: Codec,
{
    let r: Vec<F> = arthur.verifier_field_vec(m_0);

    let sum_g: F = arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read sum_g"))?;

    let rho: F = arthur.verifier_field();

    let mut saved_val_for_sumcheck_equality_assertion = rho * sum_g;

    let mut alpha = vec![F::zero(); m_0];

    for item in &mut alpha {
        let hhat_i: [F; 4] = [
            arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read hhat coeff"))?,
            arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read hhat coeff"))?,
            arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read hhat coeff"))?,
            arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read hhat coeff"))?,
        ];
        let alpha_i: F = arthur.verifier_field();
        *item = alpha_i;
        let hhat_i_at_zero = eval_cubic_poly(hhat_i, F::zero());
        let hhat_i_at_one = eval_cubic_poly(hhat_i, F::one());
        ensure!(
            saved_val_for_sumcheck_equality_assertion == hhat_i_at_zero + hhat_i_at_one,
            "Sumcheck equality assertion failed"
        );
        saved_val_for_sumcheck_equality_assertion = eval_cubic_poly(hhat_i, alpha_i);
    }

    let blinding_eval: F = arthur.prover_field().map_err(|_| anyhow::anyhow!("Failed to read blinding eval"))?;

    let f_at_alpha = saved_val_for_sumcheck_equality_assertion - rho * blinding_eval;

    Ok(DataFromSumcheckVerifier { r, alpha, blinding_eval, f_at_alpha })
}

/// Verify that the prover's claimed challenge evaluation matches the
/// Fiat-Shamir challenges sampled by the verifier. This binds the committed
/// w2 polynomial to the transcript-derived challenge values.
fn verify_challenge_binding<F: Field>(challenge_eval: F, x: F, challenges: &[F]) -> Result<()> {
    let mut expected = F::zero();
    let mut x_pow = F::one();
    for &ch in challenges {
        expected += x_pow * ch;
        x_pow *= x;
    }
    ensure!(
        challenge_eval == expected,
        "Challenge binding check failed: expected {expected:?}, got {challenge_eval:?}"
    );
    Ok(())
}

/// Verify that the prover's claimed public evaluation matches the known public
/// inputs. The weight covers positions `[0, 1, ..., N]` where position 0 is the
/// R1CS constant `1` and positions `1..=N` are the public inputs.
///
/// Public inputs are base-field values; each is lifted into the extension via
/// the embedding (`mixed_mul`) as it is folded against the extension point `x`.
fn verify_public_input_binding<M: Embedding>(
    embedding: &M,
    public_eval: M::Target,
    x: M::Target,
    public_inputs: &PublicInputs<M::Source>,
) -> Result<()> {
    let mut expected = M::Target::one();
    let mut x_pow = x;
    for &pi in &public_inputs.0 {
        expected += embedding.mixed_mul(x_pow, pi);
        x_pow *= x;
    }
    ensure!(public_eval == expected, "Public input binding check failed: expected {expected:?}, got {public_eval:?}");
    Ok(())
}
