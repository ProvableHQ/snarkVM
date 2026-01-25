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

use crate::{
    fft::{
        DensePolynomial,
        EvaluationDomain,
        Evaluations,
        Polynomial,
        domain::{FFTPrecomputation, IFFTPrecomputation},
        polynomial::PolyMultiplier,
    },
    polycommit::sonic_pc::{LabeledPolynomialWithBasis, PolynomialInfo, PolynomialLabel, PolynomialWithBasis},
    snark::varuna::{
        AHPError,
        Matrix,
        SNARKMode,
        VarunaVersion,
        ahp::{AHPForR1CS, indexer::CircuitId, verifier},
        matrices::transpose,
        prover::{self, MatrixSums, ThirdMessage},
        selectors::apply_randomized_selector,
        verifier::select_third_round_challenges,
    },
};
use snarkvm_fields::PrimeField;
use snarkvm_utilities::{ExecutionPool, cfg_iter};

use anyhow::{Result, ensure};
use itertools::Itertools;
use rand::RngCore;
use std::collections::BTreeMap;

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

struct LinevalInstance<F: PrimeField> {
    h_1_i: PolynomialWithBasis<'static, F>,
    xg_1_i: PolynomialWithBasis<'static, F>,
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
        verifier_first_message: &verifier::FirstMessage<F>,
        verifier_second_message: &verifier::SecondMessage<F>,
        verifier_prepare_third_message: &Option<verifier::PrepareThirdMessage<F>>,
        mut state: prover::State<'a, F, SM>,
        _r: &mut R,
        varuna_version: VarunaVersion,
    ) -> Result<(Option<prover::ThirdMessage<F>>, prover::ThirdOracles<F>, prover::State<'a, F, SM>), AHPError> {
        let round_time = start_timer!(|| "AHP::Prover::ThirdRound");

        let zk_bound = Self::zk_bound();

        let max_variable_domain = state.max_variable_domain;

        // Choose challenges based on the proof system version.
        let (alpha, third_round_batch_combiners, eta_b, eta_c) = select_third_round_challenges(
            verifier_first_message,
            verifier_second_message,
            verifier_prepare_third_message.as_ref(),
            varuna_version,
        )
        .map_err(AHPError::AnyhowError)?;

        let assignments = Self::calculate_assignments(&mut state)?;
        let matrix_transposes = Self::calculate_matrix_transpose(&mut state)?;

        let (h_1, x_g_1_sum, msg) = Self::calculate_lineval_sumcheck_witness(
            &mut state,
            &third_round_batch_combiners,
            assignments,
            matrix_transposes,
            &alpha,
            &eta_b,
            &eta_c,
            varuna_version,
        )?;

        let x_g_1_sum = match &x_g_1_sum {
            PolynomialWithBasis::Monomial { polynomial, .. } => polynomial.as_ref().into_dense(),
            PolynomialWithBasis::Lagrange { evaluations } => evaluations.as_ref().interpolate_by_ref(),
        };

        #[cfg(debug_assertions)]
        {
            let h_1 = match &h_1 {
                PolynomialWithBasis::Monomial { polynomial, .. } => polynomial.as_ref().into_dense(),
                PolynomialWithBasis::Lagrange { evaluations } => evaluations.as_ref().interpolate_by_ref(),
            };
            let mut sumcheck_lhs = h_1.mul_by_vanishing_poly(max_variable_domain);
            sumcheck_lhs += &x_g_1_sum;
            debug_assert!(
                sumcheck_lhs.evaluate_over_domain_by_ref(max_variable_domain).evaluations.into_iter().sum::<F>()
                    == msg.sum(&third_round_batch_combiners, eta_b, eta_c)
            );
        }

        // Send the assigned matrix sums to the verifier only in VarunaVersion::V1.
        let msg = match varuna_version {
            VarunaVersion::V1 => Some(msg),
            VarunaVersion::V2 => None,
        };

        let g_1 = DensePolynomial::from_coefficients_slice(&x_g_1_sum.coeffs[1..]);

        let (g_1, h_1) = match (SM::MONOMIAL, h_1) {
            (true, PolynomialWithBasis::Monomial { polynomial, .. }) => {
                let g_1 = Polynomial::from(g_1);
                let g_1 = LabeledPolynomialWithBasis::new_monomial_basis_owned(
                    "g_1".into(),
                    g_1,
                    Some(max_variable_domain.size() - 2),
                    Self::zk_bound(),
                );
                let h_1 = polynomial.into_owned();
                let h_1 = LabeledPolynomialWithBasis::new_monomial_basis_owned("h_1".into(), h_1, None, None);
                (g_1, h_1)
            }
            (false, PolynomialWithBasis::Lagrange { evaluations }) => {
                let g_1 = Polynomial::from(g_1);
                let g_1 = LabeledPolynomialWithBasis::new_monomial_basis_owned(
                    "g_1".into(),
                    g_1,
                    Some(max_variable_domain.size() - 2),
                    Self::zk_bound(),
                );
                let h_1_evals = evaluations.into_owned();
                let h_1 = LabeledPolynomialWithBasis::new_lagrange_basis("h_1".into(), h_1_evals, None);
                (g_1, h_1)
            }
            _ => todo!(),
        };

        drop(x_g_1_sum); // Be assured we don't use x_g_1_sum anymore

        assert!(g_1.degree() <= max_variable_domain.size() - 2);
        assert!(h_1.degree() <= 2 * max_variable_domain.size() + 2 * zk_bound.unwrap_or(0) - 2);

        let oracles = prover::ThirdOracles { g_1, h_1 };
        assert!(oracles.matches_info(&Self::third_round_polynomial_info(state.max_variable_domain.size())));

        end_timer!(round_time);

        Ok((msg, oracles, state))
    }

    #[allow(clippy::too_many_arguments)]
    fn calculate_lineval_sumcheck_witness(
        state: &mut prover::State<F, SM>,
        third_round_batch_combiners: &BTreeMap<CircuitId, verifier::BatchCombiners<F>>,
        assignments: BTreeMap<CircuitId, Vec<PolynomialWithBasis<F>>>,
        matrix_transposes: BTreeMap<CircuitId, BTreeMap<String, Matrix<F>>>,
        alpha: &F,
        eta_b: &F,
        eta_c: &F,
        varuna_version: VarunaVersion,
    ) -> Result<(PolynomialWithBasis<'static, F>, PolynomialWithBasis<'static, F>, ThirdMessage<F>)> {
        let num_instances = third_round_batch_combiners.values().map(|c| c.instance_combiners.len()).collect_vec();
        let total_instances = num_instances.iter().sum::<usize>();
        let max_variable_domain = &state.max_variable_domain;
        let matrix_labels = ["a", "b", "c"];
        let matrix_combiners = [F::one(), *eta_b, *eta_c];

        // Compute lineval sumcheck witnesses
        let mut job_pool = ExecutionPool::with_capacity(total_instances * 3);
        // Iterate for each circuit in the batch.
        for ((((circuit, circuit_specific_state), batch_combiner), assignments_i), matrix_transposes_i) in state
            .circuit_specific_states
            .iter_mut()
            .zip_eq(third_round_batch_combiners.values())
            .zip_eq(assignments.values())
            .zip_eq(matrix_transposes.values())
        {
            let circuit_combiner = batch_combiner.circuit_combiner;
            let instance_combiners = &batch_combiner.instance_combiners;
            let constraint_domain = &circuit_specific_state.constraint_domain;
            let variable_domain = &circuit_specific_state.variable_domain;
            let fft_precomputation = &circuit.fft_precomputation;
            let ifft_precomputation = &circuit.ifft_precomputation;

            // Iterate for each instance in the batch.
            for (instance_combiner, assignment) in itertools::izip!(instance_combiners, assignments_i) {
                // Destructure the optional z_m_at_alpha_polys to a vector of optional
                // DensePolynomials.
                let z_m_at_alpha_for_circuit = match &mut circuit_specific_state.z_m_at_alpha_polys {
                    Some(z_m_at_alpha) => {
                        ensure!(z_m_at_alpha.len() > 0);
                        let Some([z_a_at_alpha, z_b_at_alpha, z_c_at_alpha]) = z_m_at_alpha.pop_front() else {
                            anyhow::bail!("Expected z_m_at_alpha_polys to contain sufficient elements.")
                        };
                        [Some(z_a_at_alpha), Some(z_b_at_alpha), Some(z_c_at_alpha)]
                    }
                    None => [None, None, None],
                };
                // Iterate for each R1CS matrix corresponding to the circuit and instance.
                for (label, matrix_combiner, z_m_at_alpha) in
                    itertools::izip!(matrix_labels, matrix_combiners, z_m_at_alpha_for_circuit)
                {
                    let matrix_transpose = &matrix_transposes_i[label];
                    let combiner = circuit_combiner * instance_combiner * matrix_combiner;
                    job_pool.add_job(move || match varuna_version {
                        VarunaVersion::V1 => {
                            todo!()
                            // let z_m_at_alpha =
                            // Self::calculate_lineval_sumcheck_instance_witness(
                            //     label,
                            //     constraint_domain,
                            //     variable_domain,
                            //     fft_precomputation,
                            //     ifft_precomputation,
                            //     assignment,
                            //     matrix_transpose,
                            //     *alpha,
                            // )?;
                            // Self::calculate_lineval_sumcheck_instance_witness_polys(
                            //     label,
                            //     variable_domain,
                            //     max_variable_domain,
                            //     combiner,
                            //     Some(z_m_at_alpha),
                            // )
                        }
                        VarunaVersion::V2 => Self::calculate_lineval_sumcheck_instance_witness_polys(
                            label,
                            variable_domain,
                            max_variable_domain,
                            combiner,
                            z_m_at_alpha,
                        ),
                    });
                }
            }
        }

        let mut sums = num_instances.iter().map(|n| Vec::with_capacity(*n)).collect_vec();
        let mut circuit_index = 0;
        let mut instances_seen = 0;
        let (h_1_sum, xg_1_sum) = match SM::MONOMIAL {
            true => {
                let mut h_1_sum = DensePolynomial::zero();
                let mut xg_1_sum = DensePolynomial::zero();
                for (i, (lineval_a, lineval_b, lineval_c)) in
                    job_pool.execute_all().into_iter().collect::<Result<Vec<_>>>()?.into_iter().tuples().enumerate()
                {
                    let PolynomialWithBasis::Monomial { polynomial: lineval_a_h_1_i, .. } = lineval_a.h_1_i else {
                        todo!()
                    };
                    let PolynomialWithBasis::Monomial { polynomial: lineval_b_h_1_i, .. } = lineval_b.h_1_i else {
                        todo!()
                    };
                    let PolynomialWithBasis::Monomial { polynomial: lineval_c_h_1_i, .. } = lineval_c.h_1_i else {
                        todo!()
                    };
                    let PolynomialWithBasis::Monomial { polynomial: lineval_a_xg_1_i, .. } = lineval_a.xg_1_i else {
                        todo!()
                    };
                    let PolynomialWithBasis::Monomial { polynomial: lineval_b_xg_1_i, .. } = lineval_b.xg_1_i else {
                        todo!()
                    };
                    let PolynomialWithBasis::Monomial { polynomial: lineval_c_xg_1_i, .. } = lineval_c.xg_1_i else {
                        todo!()
                    };
                    h_1_sum += &*lineval_a_h_1_i.to_owned();
                    h_1_sum += &*lineval_b_h_1_i.to_owned();
                    h_1_sum += &*lineval_c_h_1_i.to_owned();
                    xg_1_sum += &*lineval_a_xg_1_i.to_owned();
                    xg_1_sum += &*lineval_b_xg_1_i.to_owned();
                    xg_1_sum += &*lineval_c_xg_1_i.to_owned();
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
                (
                    PolynomialWithBasis::new_dense_monomial_basis(h_1_sum, None),
                    PolynomialWithBasis::new_dense_monomial_basis(xg_1_sum, None),
                )
            }
            false => {
                let mut h_1_sum = Evaluations::zero(*max_variable_domain);
                let mut xg_1_sum = Evaluations::zero(*max_variable_domain);
                for (i, (lineval_a, lineval_b, lineval_c)) in
                    job_pool.execute_all().into_iter().collect::<Result<Vec<_>>>()?.into_iter().tuples().enumerate()
                {
                    let PolynomialWithBasis::Lagrange { evaluations: lineval_a_h_1_i, .. } = lineval_a.h_1_i else {
                        todo!()
                    };
                    let PolynomialWithBasis::Lagrange { evaluations: lineval_b_h_1_i, .. } = lineval_b.h_1_i else {
                        todo!()
                    };
                    let PolynomialWithBasis::Lagrange { evaluations: lineval_c_h_1_i, .. } = lineval_c.h_1_i else {
                        todo!()
                    };
                    let PolynomialWithBasis::Lagrange { evaluations: lineval_a_xg_1_i, .. } = lineval_a.xg_1_i else {
                        todo!()
                    };
                    let PolynomialWithBasis::Lagrange { evaluations: lineval_b_xg_1_i, .. } = lineval_b.xg_1_i else {
                        todo!()
                    };
                    let PolynomialWithBasis::Lagrange { evaluations: lineval_c_xg_1_i, .. } = lineval_c.xg_1_i else {
                        todo!()
                    };
                    h_1_sum += &lineval_a_h_1_i;
                    h_1_sum += &lineval_b_h_1_i;
                    h_1_sum += &lineval_c_h_1_i;
                    xg_1_sum += &lineval_a_xg_1_i;
                    xg_1_sum += &lineval_b_xg_1_i;
                    xg_1_sum += &lineval_c_xg_1_i;
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
                (PolynomialWithBasis::new_lagrange_basis(h_1_sum), PolynomialWithBasis::new_lagrange_basis(xg_1_sum))
            }
        };
        // let mask_poly =
        // state.first_round_oracles.as_ref().unwrap().mask_poly.as_ref();
        // assert_eq!(SM::ZK, mask_poly.is_some());
        // assert_eq!(!SM::ZK, mask_poly.is_none());
        // let mask_poly = &mask_poly.map_or(DensePolynomial::zero(), |p| match
        // &p.polynomial {     PolynomialWithBasis::Monomial { polynomial, .. }
        // => polynomial.as_ref().into_dense(),
        //     PolynomialWithBasis::Lagrange { evaluations } =>
        // evaluations.as_ref().interpolate_by_ref(), });
        // let (mut h_1_mask, mut xg_1_mask) =
        // mask_poly.divide_by_vanishing_poly(*max_variable_domain).unwrap();
        // h_1_sum += &core::mem::take(&mut h_1_mask);
        // xg_1_sum += &core::mem::take(&mut xg_1_mask);

        let msg = ThirdMessage { sums };

        Ok((h_1_sum, xg_1_sum, msg))
    }

    pub(in crate::snark::varuna) fn calculate_assignments(
        state: &mut prover::State<F, SM>,
    ) -> Result<BTreeMap<CircuitId, Vec<PolynomialWithBasis<'static, F>>>> {
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
                        let w_dense = match &w_poly.0.polynomial {
                            PolynomialWithBasis::Monomial { polynomial, .. } => {
                                let mut poly = polynomial.as_ref().into_dense().mul_by_vanishing_poly(*input_domain);
                                // Zip safety: `x_poly` is smaller than `z_poly`.
                                poly.coeffs.iter_mut().zip(&x_poly.coeffs).for_each(|(z, x)| *z += x);
                                PolynomialWithBasis::new_dense_monomial_basis(poly, None)
                            }
                            PolynomialWithBasis::Lagrange { evaluations } => {
                                // TODO: make this more efficient and ergonomic.
                                let mut evals = evaluations.evaluations.clone();
                                for i in 0..input_domain.size() {
                                    evals[i] = F::zero();
                                }
                                evals.iter_mut().zip(&x_poly.coeffs).for_each(|(z, x)| *z += x);
                                let domain_size = evals.len().next_power_of_two();
                                let domain = EvaluationDomain::new(domain_size).unwrap();
                                let evals = Evaluations::from_vec_and_domain(evals, domain);
                                PolynomialWithBasis::new_lagrange_basis(evals)
                            }
                        };
                        end_timer!(z_time);
                        w_dense
                    })
                    .collect();
                (circuit.id, assignments_i)
            })
            .collect();
        end_timer!(assignments_time);
        Ok(assignments)
    }

    pub(in crate::snark::varuna) fn calculate_matrix_transpose(
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
    pub(in crate::snark::varuna) fn calculate_lineval_sumcheck_instance_witness(
        _matrix_label: &str,
        constraint_domain: &EvaluationDomain<F>,
        variable_domain: &EvaluationDomain<F>,
        fft_precomputation: &FFTPrecomputation<F>,
        ifft_precomputation: &IFFTPrecomputation<F>,
        assignment: &PolynomialWithBasis<'static, F>,
        matrix_transpose: &Matrix<F>,
        alpha: F,
    ) -> Result<PolynomialWithBasis<'static, F>> {
        // Let C = variable_domain
        // Let R = constraint_domain
        // Let K = non_zero_domain
        // Let L^S_t(X) = Lagrange polynomial evaluating to 1 on S when any X∈S==t

        // Compute for each c∈C: M(α,c) = \sum_{κ∈K} val(κ)·L^R_row(κ)(α)·L^C_col(κ)(c)
        // We do this by iterating over the sparse transpose of matrix M
        // Instead of calculating L^C_col(κ)(c), we add val(k)*L^R_row(α) where we know
        // L^C_col(k)(X) will be 1
        let m_at_alpha_evals_time = start_timer!(|| format!("Compute m_at_alpha_evals parallel for {_matrix_label}"));
        let l_at_alpha = constraint_domain.evaluate_all_lagrange_coefficients(alpha);
        let m_at_alpha_evals: Vec<_> = cfg_iter!(matrix_transpose)
            .map(|col| col.iter().map(|(val, row_index)| *val * l_at_alpha[*row_index]).sum::<F>())
            .collect();
        end_timer!(m_at_alpha_evals_time);

        let z_m_at_alpha_time = start_timer!(|| format!("Compute z_m_at_alpha_time for {_matrix_label}"));
        let z_m_at_alpha = match assignment {
            PolynomialWithBasis::Monomial { polynomial, degree_bound } => {
                let m_at_alpha = Evaluations::from_vec_and_domain(m_at_alpha_evals, *variable_domain)
                    .interpolate_with_pc(ifft_precomputation);
                let mut multiplier = PolyMultiplier::new();
                let dense_poly = polynomial.as_ref().into_dense();
                multiplier.add_precomputation(fft_precomputation, ifft_precomputation);
                multiplier.add_polynomial(m_at_alpha, "m_at_alpha");
                multiplier.add_polynomial_ref(&dense_poly, "assignment");
                let poly = multiplier.multiply().unwrap();
                PolynomialWithBasis::new_dense_monomial_basis(poly, *degree_bound)
            }
            PolynomialWithBasis::Lagrange { evaluations } => {
                let mut m_at_alpha = Evaluations::from_vec_and_domain(m_at_alpha_evals, *variable_domain);
                m_at_alpha *= evaluations.as_ref();
                PolynomialWithBasis::new_lagrange_basis(m_at_alpha)
            }
        };
        end_timer!(z_m_at_alpha_time);

        Ok(z_m_at_alpha)
    }

    fn calculate_lineval_sumcheck_instance_witness_polys(
        _matrix_label: &str,
        variable_domain: &EvaluationDomain<F>,
        max_variable_domain: &EvaluationDomain<F>,
        combiner: F,
        z_m_at_alpha: Option<PolynomialWithBasis<'static, F>>,
    ) -> Result<LinevalInstance<F>> {
        let z_m_at_alpha = z_m_at_alpha.ok_or(anyhow::anyhow!(format!("Expected z_{_matrix_label}_at_alpha")))?;
        let sum = match &z_m_at_alpha {
            PolynomialWithBasis::Monomial { polynomial, .. } => {
                polynomial.into_dense().evaluate_over_domain_by_ref(*variable_domain).evaluations.into_iter().sum::<F>()
            }
            PolynomialWithBasis::Lagrange { evaluations } => evaluations.evaluations.iter().copied().sum::<F>(),
        };

        let (h_1_i, xg_1_i) =
            apply_randomized_selector(z_m_at_alpha, combiner, max_variable_domain, variable_domain, true)?;
        let xg_1_i = xg_1_i.ok_or(anyhow::anyhow!("Expected remainder when applying selector."))?;

        Ok(LinevalInstance { h_1_i, xg_1_i, sum })
    }
}
