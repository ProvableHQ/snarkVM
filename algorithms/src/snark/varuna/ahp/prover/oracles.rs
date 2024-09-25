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

use std::collections::BTreeMap;

use snarkvm_fields::PrimeField;

use crate::{
    polycommit::sonic_pc::{LabeledPolynomial, PolynomialInfo, PolynomialLabel},
    snark::varuna::CircuitId,
};

/// The first set of prover oracles.
#[derive(Debug, Clone)]
pub struct FirstOracles<F: PrimeField> {
    pub(in crate::snark::varuna) batches: BTreeMap<CircuitId, Vec<WitnessPoly<F>>>,
    /// The sum-check hiding polynomial.
    pub mask_poly: Option<LabeledPolynomial<F>>,
}

impl<F: PrimeField> FirstOracles<F> {
    /// Iterate over the polynomials output by the prover in the first round.
    pub fn iter(&self) -> impl Iterator<Item = &'_ LabeledPolynomial<F>> {
        self.batches.values().flat_map(|b| b.iter()).flat_map(|b| b.iter()).chain(self.mask_poly.as_ref())
    }

    /// Iterate over the polynomials output by the prover in the first round.
    pub fn into_iter(self) -> impl Iterator<Item = LabeledPolynomial<F>> {
        self.batches.into_values().flat_map(|b| b.into_iter()).map(|b| b.0).chain(self.mask_poly)
    }

    pub fn matches_info(&self, info: &BTreeMap<PolynomialLabel, PolynomialInfo>) -> bool {
        self.batches.values().all(|b| b.iter().all(|b| b.matches_info(info)))
            && self.mask_poly.as_ref().map_or(true, |p| Some(p.info()) == info.get(p.label()))
    }
}

/// The LDE of `w`.
#[derive(Debug, Clone)]
pub(in crate::snark::varuna) struct WitnessPoly<F: PrimeField>(pub(in crate::snark::varuna) LabeledPolynomial<F>);

impl<F: PrimeField> WitnessPoly<F> {
    /// Iterate over the polynomials output by the prover in the first round.
    pub fn iter(&self) -> impl Iterator<Item = &LabeledPolynomial<F>> {
        [(&self.0)].into_iter()
    }

    pub fn matches_info(&self, info: &BTreeMap<PolynomialLabel, PolynomialInfo>) -> bool {
        Some(self.0.info()) == info.get(self.0.label())
    }
}

/// The second set of prover oracles.
#[derive(Debug)]
pub struct SecondOracles<F: PrimeField> {
    /// The polynomial `h` resulting from the first zerocheck.
    pub h_0: LabeledPolynomial<F>,
}

impl<F: PrimeField> SecondOracles<F> {
    /// Iterate over the polynomials output by the prover in the second round.
    pub fn iter(&self) -> impl Iterator<Item = &LabeledPolynomial<F>> {
        [&self.h_0].into_iter()
    }

    /// Iterate over the polynomials output by the prover in the second round.
    pub fn into_iter(self) -> impl Iterator<Item = LabeledPolynomial<F>> {
        [self.h_0].into_iter()
    }

    pub fn matches_info(&self, info: &BTreeMap<PolynomialLabel, PolynomialInfo>) -> bool {
        Some(self.h_0.info()) == info.get(self.h_0.label())
    }
}

/// The third set of prover oracles.
#[derive(Debug)]
pub struct ThirdOracles<F: PrimeField> {
    /// The polynomial `g` resulting from the first sumcheck.
    pub g_1: LabeledPolynomial<F>,
    /// The polynomial `h` resulting from the first sumcheck.
    pub h_1: LabeledPolynomial<F>,
}

impl<F: PrimeField> ThirdOracles<F> {
    /// Iterate over the polynomials output by the prover in the third round.
    pub fn iter(&self) -> impl Iterator<Item = &LabeledPolynomial<F>> {
        [&self.g_1, &self.h_1].into_iter()
    }

    /// Iterate over the polynomials output by the prover in the third round.
    pub fn into_iter(self) -> impl Iterator<Item = LabeledPolynomial<F>> {
        [self.g_1, self.h_1].into_iter()
    }

    pub fn matches_info(&self, info: &BTreeMap<PolynomialLabel, PolynomialInfo>) -> bool {
        Some(self.h_1.info()) == info.get(self.h_1.label()) && Some(self.g_1.info()) == info.get(self.g_1.label())
    }
}

/// The fourth set of prover oracles.
#[derive(Debug)]
pub struct FourthOracles<F: PrimeField> {
    pub(in crate::snark::varuna) gs: BTreeMap<CircuitId, MatrixGs<F>>,
}

#[derive(Debug)]
pub(in crate::snark::varuna) struct MatrixGs<F: PrimeField> {
    /// The polynomial `g_a` resulting from the second sumcheck.
    pub(in crate::snark::varuna) g_a: LabeledPolynomial<F>,
    /// The polynomial `g_b` resulting from the second sumcheck.
    pub(in crate::snark::varuna) g_b: LabeledPolynomial<F>,
    /// The polynomial `g_c` resulting from the second sumcheck.
    pub(in crate::snark::varuna) g_c: LabeledPolynomial<F>,
}

impl<F: PrimeField> MatrixGs<F> {
    pub fn matches_matrix_info(&self, info: &BTreeMap<PolynomialLabel, PolynomialInfo>) -> bool {
        Some(self.g_a.info()) == info.get(self.g_a.label())
            && Some(self.g_b.info()) == info.get(self.g_b.label())
            && Some(self.g_c.info()) == info.get(self.g_c.label())
    }
}

impl<F: PrimeField> FourthOracles<F> {
    /// Iterate over the polynomials output by the prover in the fourth round.
    pub fn iter(&self) -> impl Iterator<Item = &LabeledPolynomial<F>> {
        self.gs.values().flat_map(|gs| [&gs.g_a, &gs.g_b, &gs.g_c].into_iter())
    }

    /// Iterate over the polynomials output by the prover in the fourth round.
    pub fn into_iter(self) -> impl Iterator<Item = LabeledPolynomial<F>> {
        self.gs.into_values().flat_map(|gs| [gs.g_a, gs.g_b, gs.g_c].into_iter())
    }

    pub fn matches_info(&self, info: &BTreeMap<PolynomialLabel, PolynomialInfo>) -> bool {
        self.gs.values().all(|b| b.matches_matrix_info(info))
    }
}

#[derive(Debug)]
pub struct FifthOracles<F: PrimeField> {
    /// The polynomial `h_2` resulting from the second sumcheck.
    pub h_2: LabeledPolynomial<F>,
}

impl<F: PrimeField> FifthOracles<F> {
    /// Iterate over the polynomials output by the prover in the previous round.
    pub fn iter(&self) -> impl Iterator<Item = &LabeledPolynomial<F>> {
        [&self.h_2].into_iter()
    }

    /// Iterate over the polynomials output by the prover in the previous round.
    pub fn into_iter(self) -> impl Iterator<Item = LabeledPolynomial<F>> {
        [self.h_2].into_iter()
    }

    pub fn matches_info(&self, info: &BTreeMap<PolynomialLabel, PolynomialInfo>) -> bool {
        Some(self.h_2.info()) == info.get(self.h_2.label())
    }
}
