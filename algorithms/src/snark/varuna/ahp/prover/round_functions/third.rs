// Copyright 2024 Aleo Network Foundation
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

use crate::{
    fft::{
        domain::{FFTPrecomputation, IFFTPrecomputation},
        polynomial::PolyMultiplier,
        DensePolynomial,
        EvaluationDomain,
        Evaluations,
    },
    polycommit::sonic_pc::{LabeledPolynomial, PolynomialInfo, PolynomialLabel},
    snark::varuna::{
        ahp::{indexer::CircuitId, verifier, AHPForR1CS},
        matrices::transpose,
        prover::{self, MatrixSums, ThirdMessage},
        selectors::apply_randomized_selector,
        AHPError,
        Matrix,
        SNARKMode,
    },
};
use snarkvm_fields::PrimeField;
use snarkvm_utilities::{cfg_iter, ExecutionPool};

use anyhow::{ensure, Result};
use itertools::Itertools;
use rand_core::RngCore;
use std::collections::BTreeMap;

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

struct LinevalInstance<F: PrimeField> {
    h_1_i: DensePolynomial<F>,
    xg_1_i: DensePolynomial<F>,
    sum: F,
}

impl<F: PrimeField, SM: SNARKMode> AHPForR1CS<F, SM> {
    /// Output the number of oracles sent by the prover in the third round.
    pub const fn num_third_round_oracles() -> usize {
        2
    }

    /// Output the degree bounds of oracles in the first round.
    pub fn third_round_polynomial_info(variable_domain_size: usize) -> BTreeMap<PolynomialLabel, PolynomialInfo> {
        [
            PolynomialInfo::new("g_1".into(), Some(variable_domain_size - 2), Self::zk_bound()),
            PolynomialInfo::new("h_1".into(), None, None),
        ]
        .into_iter()
        .map(|info| (info.label().into(), info))
        .collect()
    }

    /// Output the third round message and the next state.
    pub fn prover_third_round<'a, R: RngCore>(
        verifier_message: &verifier::FirstMessage<F>,
        verifier_second_message: &verifier::SecondMessage<F>,
        mut state: prover::State<'a, F, SM>,
        _r: &mut R,
    ) -> Result<(prover::ThirdMessage<F>, prover::ThirdOracles<F>, prover::State<'a, F, SM>), AHPError> {
        let round_time = start_timer!(|| "AHP::Prover::ThirdRound");

        let zk_bound = Self::zk_bound();

        let max_variable_domain = state.max_variable_domain;

        let verifier::FirstMessage { batch_combiners } = verifier_message;
        let verifier::SecondMessage { alpha, eta_b, eta_c } = verifier_second_message;

        let assignments = Self::calculate_assignments(&mut state)?;
        let matrix_transposes = Self::calculate_matrix_transpose(&mut state)?;

        let (h_1, x_g_1_sum, msg) = Self::calculate_lineval_sumcheck_witness(
            &mut state,
            batch_combiners,
            assignments,
            matrix_transposes,
            alpha,
            eta_b,
            eta_c,
        )?;

        #[cfg(debug_assertions)]
        {
            let mut sumcheck_lhs = h_1.mul_by_vanishing_poly(max_variable_domain);
            sumcheck_lhs += &x_g_1_sum;
            debug_assert!(
                sumcheck_lhs.evaluate_over_domain_by_ref(max_variable_domain).evaluations.into_iter().sum::<F>()
                    == msg.sum(batch_combiners, *eta_b, *eta_c)
            );
        }

        let g_1 = DensePolynomial::from_coefficients_slice(&x_g_1_sum.coeffs[1..]);

        drop(x_g_1_sum); // Be assured we don't use x_g_1_sum anymore

        assert!(g_1.degree() <= max_variable_domain.size() - 2);
        assert!(h_1.degree() <= 2 * max_variable_domain.size() + 2 * zk_bound.unwrap_or(0) - 2);

        let oracles = prover::ThirdOracles {
            g_1: LabeledPolynomial::new("g_1", g_1, max_variable_domain.size() - 2, zk_bound),
            h_1: LabeledPolynomial::new("h_1", h_1, None, None),
        };
        assert!(oracles.matches_info(&Self::third_round_polynomial_info(state.max_variable_domain.size())));

        end_timer!(round_time);

        Ok((msg, oracles, state))
    }

    fn calculate_lineval_sumcheck_witness(
        state: &mut prover::State<F, SM>,
        batch_combiners: &BTreeMap<CircuitId, verifier::BatchCombiners<F>>,
        assignments: BTreeMap<CircuitId, Vec<DensePolynomial<F>>>,
        matrix_transposes: BTreeMap<CircuitId, BTreeMap<String, Matrix<F>>>,
        alpha: &F,
        eta_b: &F,
        eta_c: &F,
    ) -> Result<(DensePolynomial<F>, DensePolynomial<F>, ThirdMessage<F>)> {
        let num_instances = batch_combiners.values().map(|c| c.instance_combiners.len()).collect_vec();
        let total_instances = num_instances.iter().sum::<usize>();
        let max_variable_domain = &state.max_variable_domain;
        let matrix_labels = ["a", "b", "c"];
        let matrix_combiners = [F::one(), *eta_b, *eta_c];

        // Compute lineval sumcheck witnesses
        let mut job_pool = ExecutionPool::with_capacity(total_instances * 3);
        for ((((circuit, circuit_specific_state), batch_combiner), assignments_i), matrix_transposes_i) in state
            .circuit_specific_states
            .iter_mut()
            .zip_eq(batch_combiners.values())
            .zip_eq(assignments.values())
            .zip_eq(matrix_transposes.values())
        {
            let circuit_combiner = batch_combiner.circuit_combiner;
            let instance_combiners = &batch_combiner.instance_combiners;
            let constraint_domain = &circuit_specific_state.constraint_domain;
            let variable_domain = &circuit_specific_state.variable_domain;
            let fft_precomputation = &circuit.fft_precomputation;
            let ifft_precomputation = &circuit.ifft_precomputation;

            for (&instance_combiner, assignment) in itertools::izip!(instance_combiners, assignments_i) {
                for (label, matrix_combiner) in itertools::izip!(matrix_labels, matrix_combiners) {
                    let matrix_transpose = &matrix_transposes_i[label];
                    let combiner = circuit_combiner * instance_combiner * matrix_combiner;
                    job_pool.add_job(move || {
                        Self::calculate_lineval_sumcheck_instance_witness(
                            label,
                            constraint_domain,
                            variable_domain,
                            max_variable_domain,
                            fft_precomputation,
                            ifft_precomputation,
                            assignment,
                            matrix_transpose,
                            *alpha,
                            combiner,
                        )
                    });
                }
            }
        }

        let mut sums = num_instances.iter().map(|n| Vec::with_capacity(*n)).collect_vec();
        let mut h_1_sum = DensePolynomial::zero();
        let mut xg_1_sum = DensePolynomial::zero();
        let mut circuit_index = 0;
        let mut instances_seen = 0;
        for (i, linevals) in job_pool.execute_all().chunks_exact_mut(3).enumerate() {
            if linevals[0].is_ok() && linevals[1].is_ok() && linevals[2].is_ok() {
                let lineval_a = linevals[0].as_ref().unwrap();
                let lineval_b = linevals[1].as_ref().unwrap();
                let lineval_c = linevals[2].as_ref().unwrap();
                h_1_sum += &lineval_a.h_1_i;
                h_1_sum += &lineval_b.h_1_i;
                h_1_sum += &lineval_c.h_1_i;
                xg_1_sum += &lineval_a.xg_1_i;
                xg_1_sum += &lineval_b.xg_1_i;
                xg_1_sum += &lineval_c.xg_1_i;
                sums[circuit_index].push(MatrixSums {
                    sum_a: lineval_a.sum,
                    sum_b: lineval_b.sum,
                    sum_c: lineval_c.sum,
                });
                if 1 + i - instances_seen == num_instances[circuit_index] {
                    instances_seen += num_instances[circuit_index];
                    circuit_index += 1;
                }
            }
        }

        let mask_poly = state.first_round_oracles.as_ref().unwrap().mask_poly.as_ref();
        assert_eq!(SM::ZK, mask_poly.is_some());
        assert_eq!(!SM::ZK, mask_poly.is_none());
        let mask_poly = &mask_poly.map_or(DensePolynomial::zero(), |p| p.polynomial().into_dense());
        let (mut h_1_mask, mut xg_1_mask) = mask_poly.divide_by_vanishing_poly(*max_variable_domain).unwrap();
        h_1_sum += &core::mem::take(&mut h_1_mask);
        xg_1_sum += &core::mem::take(&mut xg_1_mask);

        let msg = ThirdMessage { sums };

        Ok((h_1_sum, xg_1_sum, msg))
    }

    pub(in crate::snark::varuna) fn calculate_assignments(
        state: &mut prover::State<F, SM>,
    ) -> Result<BTreeMap<CircuitId, Vec<DensePolynomial<F>>>> {
        let assignments_time = start_timer!(|| "Calculate assignments");
        let assignments: BTreeMap<_, _> = state
            .circuit_specific_states
            .iter()
            .zip_eq(state.first_round_oracles.as_ref().unwrap().batches.values())
            .map(|((circuit, circuit_specific_state), w_polys)| {
                let x_polys = &circuit_specific_state.x_polys;
                let input_domain = &circuit_specific_state.input_domain;
                let assignments_i: Vec<_> = cfg_iter!(w_polys)
                    .zip_eq(x_polys)
                    .enumerate()
                    .map(|(_j, (w_poly, x_poly))| {
                        let z_time = start_timer!(move || format!("Compute z poly for circuit {} {}", circuit.id, _j));
                        let mut assignment =
                            w_poly.0.polynomial().as_dense().unwrap().mul_by_vanishing_poly(*input_domain);
                        // Zip safety: `x_poly` is smaller than `z_poly`.
                        assignment.coeffs.iter_mut().zip(&x_poly.coeffs).for_each(|(z, x)| *z += x);
                        end_timer!(z_time);
                        assignment
                    })
                    .collect();
                (circuit.id, assignments_i)
            })
            .collect();
        end_timer!(assignments_time);
        Ok(assignments)
    }

    fn calculate_matrix_transpose(
        state: &mut prover::State<F, SM>,
    ) -> Result<BTreeMap<CircuitId, BTreeMap<String, Matrix<F>>>> {
        let transpose_time = start_timer!(|| "Transpose of matrices");
        let mut job_pool = ExecutionPool::with_capacity(state.circuit_specific_states.len() * 3);
        state.circuit_specific_states.iter().for_each(|(circuit, circuit_specific_state)| {
            let variable_domain = &circuit_specific_state.variable_domain;
            let input_domain = &circuit_specific_state.input_domain;
            let matrices = [&circuit.a, &circuit.b, &circuit.c];
            let circuit_id = circuit.id;
            for matrix in matrices.into_iter() {
                job_pool.add_job(move || (circuit_id, transpose(matrix, variable_domain, input_domain)));
            }
        });
        let mut matrix_transposes = BTreeMap::new();
        for ((id_a, matrix_a), (id_b, matrix_b), (id_c, matrix_c)) in job_pool.execute_all().into_iter().tuples() {
            ensure!(id_a == id_b);
            ensure!(id_a == id_c);
            let mut matrix_transposes_i = BTreeMap::new();
            matrix_transposes_i.insert("a".into(), matrix_a?);
            matrix_transposes_i.insert("b".into(), matrix_b?);
            matrix_transposes_i.insert("c".into(), matrix_c?);
            matrix_transposes.insert(id_a, matrix_transposes_i);
        }
        end_timer!(transpose_time);
        Ok(matrix_transposes)
    }

    #[allow(clippy::too_many_arguments)]
    fn calculate_lineval_sumcheck_instance_witness(
        _label: &str,
        constraint_domain: &EvaluationDomain<F>,
        variable_domain: &EvaluationDomain<F>,
        max_variable_domain: &EvaluationDomain<F>,
        fft_precomputation: &FFTPrecomputation<F>,
        ifft_precomputation: &IFFTPrecomputation<F>,
        assignment: &DensePolynomial<F>,
        matrix_transpose: &Matrix<F>,
        alpha: F,
        combiner: F,
    ) -> Result<LinevalInstance<F>> {
        let sumcheck_time = start_timer!(|| format!("Compute LHS of sumcheck for {_label}"));

        // Let C = variable_domain
        // Let R = constraint_domain
        // Let K = non_zero_domain
        // Let L^S_t(X) = Lagrange polynomial evaluating to 1 on S when any X∈S==t

        // Compute for each c∈C: M(α,c) = \sum_{κ∈K} val(κ)·L^R_row(κ)(α)·L^C_col(κ)(c)
        // We do this by iterating over the sparse transpose of matrix M
        // Instead of calculating L^C_col(κ)(c), we add val(k)*L^R_row(α) where we know L^C_col(k)(X) will be 1
        let m_at_alpha_evals_time = start_timer!(|| format!("Compute m_at_alpha_evals parallel for {_label}"));
        let l_at_alpha = constraint_domain.evaluate_all_lagrange_coefficients(alpha);
        let m_at_alpha_evals: Vec<_> = cfg_iter!(matrix_transpose)
            .map(|col| col.iter().map(|(val, row_index)| *val * l_at_alpha[*row_index]).sum::<F>())
            .collect();
        end_timer!(m_at_alpha_evals_time);

        let z_m_at_alpha_time = start_timer!(|| format!("Compute z_m_at_alpha_time for {_label}"));
        let m_at_alpha = Evaluations::from_vec_and_domain(m_at_alpha_evals, *variable_domain)
            .interpolate_with_pc(ifft_precomputation);
        let mut multiplier = PolyMultiplier::new();
        multiplier.add_precomputation(fft_precomputation, ifft_precomputation);
        multiplier.add_polynomial(m_at_alpha, "m_at_alpha");
        multiplier.add_polynomial_ref(assignment, "assignment");
        let mut z_m_at_alpha = multiplier.multiply().unwrap();
        let sum = z_m_at_alpha.evaluate_over_domain_by_ref(*variable_domain).evaluations.into_iter().sum::<F>();
        end_timer!(z_m_at_alpha_time);

        let (h_1_i, xg_1_i) =
            apply_randomized_selector(&mut z_m_at_alpha, combiner, max_variable_domain, variable_domain, true)?;
        let xg_1_i = xg_1_i.ok_or(anyhow::anyhow!("Expected remainder when applying selector"))?;

        end_timer!(sumcheck_time);

        Ok(LinevalInstance { h_1_i, xg_1_i, sum })
    }
}
