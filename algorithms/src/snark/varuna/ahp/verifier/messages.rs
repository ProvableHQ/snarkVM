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

use crate::snark::varuna::{witness_label, CircuitId, SNARKMode};
use snarkvm_fields::PrimeField;

use itertools::Itertools;
use std::collections::BTreeMap;

/// Randomizers used to combine circuit-specific and instance-specific elements in the AHP sumchecks
#[derive(Clone, Debug)]
pub(crate) struct BatchCombiners<F> {
    pub(crate) circuit_combiner: F,
    pub(crate) instance_combiners: Vec<F>,
}

/// First message of the verifier.
/// We only need randomizers for B and C to get a linear combination for {A,B,C}
#[derive(Clone, Debug)]
pub struct FirstMessage<F: PrimeField> {
    /// Randomizers for combining checks from the batch
    pub(crate) batch_combiners: BTreeMap<CircuitId, BatchCombiners<F>>,
}

/// Second verifier message.
#[derive(Copy, Clone, Debug)]
pub struct SecondMessage<F> {
    /// Query for lineval.
    pub alpha: F,
    /// Randomizer for the lineval for `B`.
    pub eta_b: F,
    /// Randomizer for the lineval for `C`.
    pub eta_c: F,
}

/// Third verifier message.
#[derive(Copy, Clone, Debug)]
pub struct ThirdMessage<F> {
    /// Query for the third round of polynomials.
    pub beta: F,
}

/// Fourth message of the verifier.
#[derive(Clone, Debug)]
pub struct FourthMessage<F> {
    /// Randomizers for the h-polynomial for `A_i`, `B_i`, `C_i` for circuit i.
    pub delta_a: Vec<F>,
    pub delta_b: Vec<F>,
    pub delta_c: Vec<F>,
}

impl<F: PrimeField> FourthMessage<F> {
    pub fn into_iter(self) -> impl Iterator<Item = F> {
        self.delta_a.into_iter().zip_eq(self.delta_b).zip_eq(self.delta_c).flat_map(|((r_a, r_b), r_c)| [r_a, r_b, r_c])
    }
}

/// Query set of the verifier.
#[derive(Clone, Debug)]
pub struct QuerySet<F: PrimeField> {
    pub batch_sizes: BTreeMap<CircuitId, usize>,

    pub rowcheck_zerocheck_query: (String, F),

    pub g_1_query: (String, F),
    pub lineval_sumcheck_query: (String, F),

    pub g_a_query: (String, F),
    pub g_b_query: (String, F),
    pub g_c_query: (String, F),
    pub matrix_sumcheck_query: (String, F),
}

impl<F: PrimeField> QuerySet<F> {
    pub fn new<SM: SNARKMode>(state: &super::State<F, SM>) -> Self {
        let alpha = state.second_round_message.as_ref().unwrap().alpha;
        let beta = state.third_round_message.unwrap().beta;
        let gamma = state.gamma.unwrap();
        // The rowcheck_zerocheck, lineval_sumcheck and matrix_sumcheck are linear combinations ("virtual oracles") of other oracles
        // The rowcheck_zerocheck evaluates whether our polynomial constraints (e.g. R1CS) hold
        // The lineval_sumcheck evaluates whether those constraints hold on an evaluation of assignments multiplied by constraint matrices
        // The matrix_sumcheck evaluates whether the lineval sumcheck holds on an evaluation of constraint matrices over the domain of non-zero entries
        Self {
            batch_sizes: state.circuit_specific_states.iter().map(|(c, s)| (*c, s.batch_size)).collect(),

            rowcheck_zerocheck_query: ("alpha".into(), alpha),

            g_1_query: ("beta".into(), beta),
            lineval_sumcheck_query: ("beta".into(), beta),

            g_a_query: ("gamma".into(), gamma),
            g_b_query: ("gamma".into(), gamma),
            g_c_query: ("gamma".into(), gamma),
            matrix_sumcheck_query: ("gamma".into(), gamma),
        }
    }

    /// Returns a `BTreeSet` containing elements of the form
    /// `(polynomial_label, (query_label, query))`.
    pub fn to_set(&self) -> crate::polycommit::sonic_pc::QuerySet<F> {
        let mut query_set = crate::polycommit::sonic_pc::QuerySet::new();
        for &circuit_id in self.batch_sizes.keys() {
            query_set.insert((witness_label(circuit_id, "g_a", 0), self.g_a_query.clone()));
            query_set.insert((witness_label(circuit_id, "g_b", 0), self.g_b_query.clone()));
            query_set.insert((witness_label(circuit_id, "g_c", 0), self.g_c_query.clone()));
        }
        query_set.insert(("g_1".into(), self.g_1_query.clone()));
        query_set.insert(("rowcheck_zerocheck".into(), self.rowcheck_zerocheck_query.clone()));
        query_set.insert(("lineval_sumcheck".into(), self.lineval_sumcheck_query.clone()));
        query_set.insert(("matrix_sumcheck".into(), self.matrix_sumcheck_query.clone()));
        query_set
    }
}

/// Helper struct to collect query points
#[derive(Debug, Clone, Copy)]
pub(crate) struct QueryPoints<F: PrimeField> {
    pub(crate) alpha: F,
    pub(crate) beta: F,
    pub(crate) gamma: F,
}

impl<F: PrimeField> QueryPoints<F> {
    pub(crate) fn new(alpha: F, beta: F, gamma: F) -> Self {
        Self { alpha, beta, gamma }
    }

    pub(crate) fn into_iter(self) -> impl IntoIterator<Item = F> {
        [self.alpha, self.beta, self.gamma]
    }
}
