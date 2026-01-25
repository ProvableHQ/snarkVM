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

use super::verifier::QueryPoints;
use crate::{
    fft::{EvaluationDomain, Evaluations},
    polycommit::sonic_pc::PolynomialWithBasis,
};
use snarkvm_fields::{PrimeField, batch_inversion};
use snarkvm_utilities::{cfg_into_iter, cfg_iter_mut};

use anyhow::{Result, ensure};
use itertools::Itertools;
use std::collections::{BTreeMap, HashSet};

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

/// Precompute a batch of selectors at challenges. We batch:
/// - constraint domain selectors at `alpha`
/// - variable domain selectors at `beta`
/// - non_zero domain selectors at `gamma`
pub(crate) fn precompute_selectors<F: PrimeField>(
    max_constraint_domain: EvaluationDomain<F>,
    constraint_domains: HashSet<EvaluationDomain<F>>,
    max_variable_domain: EvaluationDomain<F>,
    variable_domains: HashSet<EvaluationDomain<F>>,
    max_non_zero_domain: EvaluationDomain<F>,
    non_zero_domains: HashSet<EvaluationDomain<F>>,
    challenges: QueryPoints<F>,
) -> BTreeMap<(u64, u64, F), F> {
    let max_domains = [max_constraint_domain, max_variable_domain, max_non_zero_domain];
    let domains = [constraint_domains, variable_domains, non_zero_domains];
    let (numerators, mut denominators, keys) = max_domains
        .into_iter()
        .zip_eq(domains)
        .zip_eq(challenges.into_iter())
        .flat_map(|((max_domain, domains), challenge)| {
            let max_domain_at_challenge = max_domain.evaluate_vanishing_polynomial(challenge);
            domains.into_iter().map(move |domain| {
                let domain_at_challenge = domain.evaluate_vanishing_polynomial(challenge);
                // Given two domains H and K such that H \subseteq K,
                // evaluate polynomial that outputs 0 on all elements in K \ H, but 1 on all
                // elements of H.
                (
                    max_domain_at_challenge * domain.size_as_field_element,
                    domain_at_challenge * max_domain.size_as_field_element,
                    (max_domain.size, domain.size, challenge),
                )
            })
        })
        .multiunzip::<(Vec<F>, Vec<F>, Vec<(u64, u64, F)>)>();
    batch_inversion(&mut denominators);
    cfg_into_iter!(numerators).zip_eq(denominators).zip_eq(keys).map(|((num, denom), key)| (key, num * denom)).collect()
}

/// Throughout the protocol, we are tasked with computing a zerocheck or
/// sumcheck of multiple polynomials over different domains.
/// These can be combined into a single check by taking a random linear
/// combination of the polynomials and multiplying them by an appropriate
/// selector polynomial. This function applies the random combiner and selector
/// in an optimized way
pub(crate) fn apply_randomized_selector<F: PrimeField>(
    poly: PolynomialWithBasis<'static, F>,
    combiner: F,
    target_domain: &EvaluationDomain<F>,
    src_domain: &EvaluationDomain<F>,
    remainder_witness: bool,
) -> Result<(PolynomialWithBasis<'static, F>, Option<PolynomialWithBasis<'static, F>>)> {
    // Let H = target_domain;
    // Let H_i = src_domain;
    // Let v_H := H.vanishing_polynomial();
    // Let v_H_i := H_i.vanishing_polynomial();
    // Let s_i := H.selector_polynomial(H_i) = (v_H / v_H_i) * (H_i.size() /
    // H.size()); Let c_i := circuit combiner
    // Let poly_i := circuit specific polynomial which is being checked

    // Instead of just multiplying each poly_i by `s_i*c_i`, we reorder the check to
    // cancel out division by v_H This removes a mul and div by v_H operation
    // over each circuit's (target_domain - src_domain) We have two scenario's:
    // either we return a remainder witness or there is none.
    if !remainder_witness {
        // Substituting in s_i, we get that poly_i * s_i / v_H = poly_i / v_H_i *
        // (H_i.size() / H.size());
        let selector_time = start_timer!(|| "Compute selector without remainder witness");

        let multiplier = combiner * src_domain.size_as_field_element * target_domain.size_inv;

        let polys = match poly {
            PolynomialWithBasis::Monomial { polynomial, .. } => {
                let mut poly = polynomial.as_ref().to_dense().into_owned();
                let (mut h_i, remainder) = poly.divide_by_vanishing_poly(*src_domain)?;
                ensure!(
                    remainder.is_zero(),
                    "Failed to divide by vanishing polynomial - non-zero remainder ({remainder:?})"
                );

                cfg_iter_mut!(h_i.coeffs).for_each(|c| *c *= multiplier);

                (PolynomialWithBasis::new_dense_monomial_basis(h_i, None), None)
            }
            PolynomialWithBasis::Lagrange { evaluations } => {
                let mut evals = evaluations.evaluations.clone();
                for i in 0..src_domain.size() {
                    evals[i] = F::zero();
                }
                evals.iter_mut().for_each(|e| *e *= multiplier);
                let domain_size = evals.len().next_power_of_two();
                let domain = EvaluationDomain::new(domain_size).unwrap();
                let evals = Evaluations::from_vec_and_domain(evals, domain);
                (PolynomialWithBasis::new_lagrange_basis(evals), None)
            }
        };
        end_timer!(selector_time);
        Ok(polys)
    } else {
        // In Lagrange mode, we minimize extra transforms by allowing evaluation-form
        // input, but we perform the vanishing-division logic in coefficient
        // form.

        // Substituting in s_i, we get that:
        // \sum_i{poly_i}/v_H = \sum{h_i*v_H + x_g_i}
        // \sum_i{c_i*s_i*(poly_i/v_H - x_g_i)} = \sum{h_i*v_H}
        // \sum_i{c_i*(H_i.size()/H.size())*(poly_i/v_H_i - x_g_i*v_H/v_H_i)} =
        // \sum{h_i*v_H} \sum_i{c_i*(H_i.size()/H.size())*(poly_i/v_H_i} =
        // \sum{h_i*v_H} + \sum{c_i*x_g_i*(v_H/v_H_i)*(H_i.size()/H.size())}
        // (\sum_i{c_i*s_i*poly_i})/v_H = \sum{h_i*v_H} + \sum{c_i*s_i*x_g_i}
        // (\sum_i{c_i*s_i*poly_i})/v_H = h_1*v_H + x_g_1
        // That's what we're computing here.
        let selector_time = start_timer!(|| "Compute selector with remainder witness");

        let multiplier = combiner * src_domain.size_as_field_element * target_domain.size_inv;
        let (h_i, xg_i) = match poly {
            PolynomialWithBasis::Monomial { polynomial, degree_bound } => {
                let mut dense_poly = polynomial.as_ref().to_dense().into_owned();
                cfg_iter_mut!(dense_poly.coeffs).for_each(|c| *c *= multiplier);
                let (h_i, mut xg_i) = dense_poly.divide_by_vanishing_poly(*src_domain)?;
                xg_i = xg_i.mul_by_vanishing_poly(*target_domain);

                let (xg_i, remainder) = xg_i.divide_by_vanishing_poly(*src_domain)?;
                ensure!(
                    remainder.is_zero(),
                    "Failed to divide by vanishing polynomial - non-zero remainder ({remainder:?})"
                );

                (
                    PolynomialWithBasis::new_dense_monomial_basis(h_i, None),
                    Some(PolynomialWithBasis::new_dense_monomial_basis(xg_i, None)),
                )
            }
            PolynomialWithBasis::Lagrange { evaluations } => {
                let mut dense_poly = evaluations.as_ref().interpolate_by_ref();
                cfg_iter_mut!(dense_poly.coeffs).for_each(|c| *c *= multiplier);
                let (h_i, mut xg_i) = dense_poly.divide_by_vanishing_poly(*src_domain)?;
                xg_i = xg_i.mul_by_vanishing_poly(*target_domain);

                let (xg_i, remainder) = xg_i.divide_by_vanishing_poly(*src_domain)?;
                ensure!(
                    remainder.is_zero(),
                    "Failed to divide by vanishing polynomial - non-zero remainder ({remainder:?})"
                );

                let h_i_evals = h_i.evaluate_over_domain(*src_domain);
                let xg_i_evals = xg_i.evaluate_over_domain(*src_domain);

                (
                    PolynomialWithBasis::new_lagrange_basis(h_i_evals),
                    Some(PolynomialWithBasis::new_lagrange_basis(xg_i_evals)),
                )

                // TODO: research how to do this using evaluations only.
                // let mut h_i = evaluations.evaluations.clone();
                // cfg_iter_mut!(h_i).for_each(|e| *e *= multiplier);
                // let mut xg_i = Vec::with_capacity(src_domain.size());
                // xg_i.extend_from_slice(&evaluations.evaluations[src_domain.
                // size()..]); // TODO: parallelize
                // for i in 0..src_domain.size() {
                //     h_i[i] = F::zero();
                // }
                // for i in 0..target_domain.size() {
                //     xg_i[i] = F::zero();
                // }
                // let (xg_i, _remainder) = xg_i.split_at(src_domain.size());
                // let h_i_domain_size = h_i.len().next_power_of_two();
                // let xg_i_domain_size = xg_i.len().next_power_of_two();
                // let h_i_domain =
                // EvaluationDomain::new(h_i_domain_size).unwrap();
                // let xg_i_domain =
                // EvaluationDomain::new(xg_i_domain_size).unwrap();
                // let h_i = Evaluations::from_vec_and_domain(h_i, h_i_domain);
                // let xg_i =
                // Evaluations::from_vec_and_domain(xg_i.into_iter().copied().
                // collect(), xg_i_domain);
                // (PolynomialWithBasis::new_lagrange_basis(h_i),
                // Some(PolynomialWithBasis::new_lagrange_basis(xg_i)))
            }
        };

        end_timer!(selector_time);
        Ok((h_i, xg_i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fft::Evaluations;
    use snarkvm_curves::bls12_377::fr::Fr;
    use snarkvm_fields::{One, Zero};
    use snarkvm_utilities::rand::TestRng;

    /// Given two domains H and K such that H \subseteq K,
    /// evaluate polynomial that outputs 0 on all elements in K \ H, but 1 on
    /// all elements of H.
    fn evaluate_selector_polynomial<F: PrimeField>(
        this: EvaluationDomain<F>,
        other: EvaluationDomain<F>,
        point: F,
    ) -> F {
        let numerator = this.evaluate_vanishing_polynomial(point) * other.size_as_field_element;
        let denominator = other.evaluate_vanishing_polynomial(point) * this.size_as_field_element;
        numerator / denominator
    }

    #[test]
    fn test_alternator_polynomial() {
        let mut rng = TestRng::default();

        let mut domain_is = vec![];
        let mut domain_js = vec![];
        let mut points = vec![];
        let mut selectors_at_points = vec![];

        for i in 2..10 {
            let domain_i = EvaluationDomain::<Fr>::new(1 << i).unwrap();
            let point = domain_i.sample_element_outside_domain(&mut rng);

            let mut selectors_at_points_at_i = vec![];
            let mut domain_js_at_i = vec![];
            for j in 1..i {
                let domain_j = EvaluationDomain::<Fr>::new(1 << j).unwrap();
                assert!(!domain_i.evaluate_vanishing_polynomial(point).is_zero());
                assert!(!domain_j.evaluate_vanishing_polynomial(point).is_zero());
                domain_js_at_i.push(domain_j);
                let j_elements = domain_j.elements().collect::<Vec<_>>();
                let slow_selector = {
                    let evals = domain_i
                        .elements()
                        .map(|e| if j_elements.contains(&e) { Fr::one() } else { Fr::zero() })
                        .collect();
                    Evaluations::from_vec_and_domain(evals, domain_i).interpolate()
                };
                let selector_at_point = evaluate_selector_polynomial(domain_i, domain_j, point);
                selectors_at_points_at_i.push(selector_at_point);

                assert_eq!(slow_selector.evaluate(point), selector_at_point);

                for element in domain_i.elements() {
                    if j_elements.contains(&element) {
                        assert_eq!(slow_selector.evaluate(element), Fr::one(), "failed for {i} vs {j}");
                    } else {
                        assert_eq!(slow_selector.evaluate(element), Fr::zero());
                    }
                }
            }
            points.push(point);
            selectors_at_points.push(selectors_at_points_at_i);
            domain_is.push(domain_i);
            domain_js.push(domain_js_at_i);
        }

        for i in 0..domain_is.len() {
            let selectors = precompute_selectors(
                domain_is[i],
                domain_js[i][0..].iter().copied().collect(),
                domain_is[i],
                domain_js[i][0..].iter().copied().collect(),
                domain_is[i],
                domain_js[i][0..].iter().copied().collect(),
                QueryPoints { alpha: points[i], beta: points[i], gamma: points[i] },
            );
            for j in 0..domain_js[i].len() {
                assert_eq!(selectors[&(domain_is[i].size, domain_js[i][j].size, points[i])], selectors_at_points[i][j]);
            }
        }
    }
}
