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

use std::collections::BTreeMap;

use crate::{
    fft::{DensePolynomial, EvaluationDomain, Evaluations as EvaluationsOnDomain, polynomial::PolyMultiplier},
    polycommit::sonic_pc::{LabeledPolynomialWithBasis, PolynomialInfo, PolynomialLabel, PolynomialWithBasis},
    snark::varuna::{
        Circuit,
        CircuitId,
        SNARKMode,
        ahp::{AHPForR1CS, verifier},
        prover,
        selectors::apply_randomized_selector,
        witness_label,
    },
};
use anyhow::Result;
use rand::RngCore;
use snarkvm_fields::PrimeField;
use snarkvm_utilities::{ExecutionPool, cfg_into_iter, cfg_iter_mut, cfg_reduce};

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

impl<F: PrimeField, SM: SNARKMode> AHPForR1CS<F, SM> {
    /// Output the number of oracles sent by the prover in the second round.
    pub const fn num_second_round_oracles() -> usize {
        1
    }

    /// Output the degree bounds of oracles in the second round.
    pub fn second_round_polynomial_info() -> BTreeMap<PolynomialLabel, PolynomialInfo> {
        [PolynomialInfo::new("h_0".into(), None, None)].into_iter().map(|info| (info.label().into(), info)).collect()
    }

    /// Output the second round message and the next state.
    pub fn prover_second_round<'a, R: RngCore>(
        verifier_message: &verifier::FirstMessage<F>,
        mut state: prover::State<'a, F, SM>,
        _r: &mut R,
    ) -> Result<(prover::SecondOracles<F>, prover::State<'a, F, SM>)> {
        let round_time = start_timer!(|| "AHP::Prover::SecondRound");

        let zk_bound = Self::zk_bound();

        let max_constraint_domain = state.max_constraint_domain;

        let verifier::FirstMessage { first_round_batch_combiners, .. } = verifier_message;

        let h_0 = Self::calculate_rowcheck_witness(&mut state, first_round_batch_combiners)?;

        assert!(h_0.degree() <= 2 * max_constraint_domain.size() + 2 * zk_bound.unwrap_or(0) - 2);

        // let h_0 = prover::to_prover_oracle_poly::<F, SM>("h_0", Some(h_0), None,
        // None, None);
        let oracles = prover::SecondOracles { h_0 };
        assert!(oracles.matches_info(&Self::second_round_polynomial_info()));

        end_timer!(round_time);

        Ok((oracles, state))
    }

    fn calculate_rowcheck_witness(
        state: &mut prover::State<F, SM>,
        batch_combiners: &BTreeMap<CircuitId, verifier::BatchCombiners<F>>,
    ) -> Result<LabeledPolynomialWithBasis<'static, F>> {
        let mut job_pool = ExecutionPool::with_capacity(state.circuit_specific_states.len());
        let max_constraint_domain = state.max_constraint_domain;

        for (circuit, circuit_specific_state) in state.circuit_specific_states.iter_mut() {
            let z_a = circuit_specific_state.z_a.take().unwrap();
            let z_b = circuit_specific_state.z_b.take().unwrap();
            let z_c = circuit_specific_state.z_c.take().unwrap();

            let circuit_combiner = batch_combiners[&circuit.id].circuit_combiner;
            let instance_combiners = batch_combiners[&circuit.id].instance_combiners.clone();
            let constraint_domain = circuit_specific_state.constraint_domain;
            let fft_precomputation = &circuit.fft_precomputation;
            let ifft_precomputation = &circuit.ifft_precomputation;

            let _circuit_id = &circuit.id; // seems like a compiler bug marks this as unused

            for (j, (instance_combiner, z_a, z_b, z_c)) in
                itertools::izip!(instance_combiners, z_a, z_b, z_c).enumerate()
            {
                job_pool.add_job(move || {
                    let za_label = witness_label(circuit.id, "z_a", j);
                    let zb_label = witness_label(circuit.id, "z_b", j);
                    let zc_label = witness_label(circuit.id, "z_c", j);
                    let rowcheck = if SM::MONOMIAL {
                        // Monomial path (existing): interpolate z_m and multiply in coefficient form.
                        let z_a = Self::calculate_z_m(za_label, z_a, constraint_domain, circuit);
                        let z_b = Self::calculate_z_m(zb_label, z_b, constraint_domain, circuit);
                        let z_c = Self::calculate_z_m(zc_label, z_c, constraint_domain, circuit);
                        let mut multiplier_2 = PolyMultiplier::new();
                        multiplier_2.add_precomputation(fft_precomputation, ifft_precomputation);
                        multiplier_2.add_polynomial(z_a, "z_a");
                        multiplier_2.add_polynomial(z_b, "z_b");
                        let mut rowcheck = multiplier_2.multiply().unwrap();
                        cfg_iter_mut!(rowcheck.coeffs).zip(&z_c.coeffs).for_each(|(ab, c)| *ab -= c);
                        PolynomialWithBasis::new_dense_monomial_basis(rowcheck, None)
                    } else {
                        // Lagrange path: compute rowcheck directly in evaluation form.
                        // rowcheck[k] = z_a[k] * z_b[k] - z_c[k]
                        let rowcheck_evals = cfg_into_iter!(z_a)
                            .zip_eq(z_b)
                            .zip_eq(z_c)
                            .map(|((a, b), c)| a * b - c)
                            .collect::<Vec<_>>();
                        let evals = EvaluationsOnDomain::from_vec_and_domain(rowcheck_evals, constraint_domain);
                        PolynomialWithBasis::new_lagrange_basis(evals)
                    };

                    let rowcheck = match rowcheck {
                        PolynomialWithBasis::Monomial { polynomial, .. } => {
                            let mut dense = polynomial.as_ref().to_dense().into_owned();
                            dense *= instance_combiner;
                            PolynomialWithBasis::new_dense_monomial_basis(dense, None)
                        }
                        PolynomialWithBasis::Lagrange { evaluations } => {
                            let mut evals = evaluations.as_ref().clone();
                            evals.evaluations.iter_mut().for_each(|e| *e *= instance_combiner);
                            PolynomialWithBasis::new_lagrange_basis(evals)
                        }
                    };

                    let (h_0_i, remainder) = apply_randomized_selector(
                        rowcheck,
                        circuit_combiner,
                        &max_constraint_domain,
                        &constraint_domain,
                        false,
                    )?;
                    assert!(remainder.is_none());

                    Ok::<_, anyhow::Error>(h_0_i)
                });
            }
        }

        let h_sum_time = start_timer!(|| "AHP::Prover::SecondRound h_sum");
        let label = "h_0".into();
        let degree_bound = None;
        let hiding_bound = None;
        let h_sum: LabeledPolynomialWithBasis<F> = match SM::MONOMIAL {
            true => {
                let mut result = DensePolynomial::zero();
                // TODO: consider using cfg_reduce!
                for poly in job_pool.execute_all() {
                    let PolynomialWithBasis::Monomial { polynomial, .. } = poly.unwrap() else { todo!() };
                    result += polynomial.as_ref();
                }
                let polynomial = PolynomialWithBasis::new_dense_monomial_basis(result, degree_bound);
                let info = PolynomialInfo::new(label, degree_bound, hiding_bound);
                LabeledPolynomialWithBasis { info, polynomial }
            }
            false => {
                let mut result = EvaluationsOnDomain::zero(max_constraint_domain);
                // TODO: consider using cfg_reduce!
                for poly in job_pool.execute_all() {
                    let PolynomialWithBasis::Lagrange { evaluations } = poly.unwrap() else { todo!() };
                    result += evaluations.as_ref();
                }
                let polynomial = PolynomialWithBasis::new_lagrange_basis(result);
                let info = PolynomialInfo::new(label, degree_bound, hiding_bound);
                LabeledPolynomialWithBasis { info, polynomial }
            }
        };
        end_timer!(h_sum_time);

        Ok(h_sum)
    }

    fn calculate_z_m(
        label: impl ToString,
        evaluations: Vec<F>,
        constraint_domain: EvaluationDomain<F>,
        circuit: &Circuit<F, SM>,
    ) -> DensePolynomial<F> {
        let label = label.to_string();
        let poly_time = start_timer!(|| format!("Computing {label}"));

        let evals = EvaluationsOnDomain::from_vec_and_domain(evaluations, constraint_domain);
        let poly = evals.interpolate_with_pc_by_ref(&circuit.ifft_precomputation);

        debug_assert!(
            poly.evaluate_over_domain_by_ref(constraint_domain)
                .evaluations
                .into_iter()
                .zip(&evals.evaluations)
                .all(|(z, e)| *e == z),
            "Label: {label}\n1: {:#?}\n2: {:#?}",
            poly.evaluate_over_domain_by_ref(constraint_domain).evaluations,
            &evals.evaluations,
        );

        end_timer!(poly_time);

        poly
    }
}
