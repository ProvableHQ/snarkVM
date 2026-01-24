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

#![allow(non_snake_case)]

mod constraint_system;
pub(crate) use constraint_system::*;

mod message;
pub(crate) use message::*;

mod oracles;
pub(crate) use oracles::*;

mod round_functions;

mod state;
use state::*;

use crate::{
    fft::{DensePolynomial, EvaluationDomain},
    polycommit::sonic_pc::{LabeledPolynomialWithBasis, PolynomialInfo, PolynomialWithBasis},
};
use snarkvm_fields::PrimeField;

/// Wrap a dense polynomial as a prover oracle, selecting a basis according to
/// `SM::MONOMIAL`.
pub(in crate::snark::varuna::ahp::prover) fn to_prover_oracle_poly<
    F: PrimeField,
    SM: crate::snark::varuna::SNARKMode,
>(
    label: impl Into<String>,
    polynomial: Option<DensePolynomial<F>>,
    evals: Option<Vec<F>>,
    degree_bound: Option<usize>,
    hiding_bound: Option<usize>,
) -> LabeledPolynomialWithBasis<'static, F> {
    let label = label.into();
    if evals.is_none() { // TODO: make a robust abstraction.
        let info = PolynomialInfo::new(label, degree_bound, hiding_bound);
        let polynomial = polynomial.expect("Monomial mode requires a polynomial in dense basis");
        let polynomial = PolynomialWithBasis::new_dense_monomial_basis(polynomial, degree_bound);
        LabeledPolynomialWithBasis { info, polynomial }
    } else {
        let evals = evals.expect("Lagrange mode requires evals in prover State");
        let evals_power_of_two = evals.len().next_power_of_two();
        let domain = EvaluationDomain::new(evals_power_of_two).expect("Lagrange mode requires a domain large enough to hold the evals");
        let info = PolynomialInfo::new(label, degree_bound, hiding_bound);
        let evals = crate::fft::Evaluations::from_vec_and_domain(evals, domain);
        let polynomial = PolynomialWithBasis::new_lagrange_basis(evals);
        LabeledPolynomialWithBasis { info, polynomial }
    }
}
