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

use core::marker::PhantomData;

use crate::{
    fft::EvaluationDomain,
    snark::varuna::{
        ahp::verifier::{FirstMessage, FourthMessage, SecondMessage, ThirdMessage},
        CircuitId,
        SNARKMode,
    },
};
use snarkvm_fields::PrimeField;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug)]
/// Circuit Specific State of the Verifier
pub struct CircuitSpecificState<F: PrimeField> {
    pub(crate) input_domain: EvaluationDomain<F>,
    pub(crate) variable_domain: EvaluationDomain<F>,
    pub(crate) constraint_domain: EvaluationDomain<F>,
    pub(crate) non_zero_a_domain: EvaluationDomain<F>,
    pub(crate) non_zero_b_domain: EvaluationDomain<F>,
    pub(crate) non_zero_c_domain: EvaluationDomain<F>,

    /// The number of instances being proved in this batch.
    pub(in crate::snark::varuna) batch_size: usize,
}
/// State of the AHP verifier.
#[derive(Debug)]
pub struct State<F: PrimeField, SM: SNARKMode> {
    /// The state for each circuit in the batch.
    pub(crate) circuit_specific_states: BTreeMap<CircuitId, CircuitSpecificState<F>>,
    /// The largest constraint domain of all circuits in the batch.
    pub(crate) max_constraint_domain: EvaluationDomain<F>,
    /// The largest variable domain of all circuits in the batch.
    pub(crate) max_variable_domain: EvaluationDomain<F>,
    /// The largest non_zero domain of all circuits in the batch.
    pub(crate) max_non_zero_domain: EvaluationDomain<F>,

    /// The verifier message in the first round of the AHP
    pub(crate) first_round_message: Option<FirstMessage<F>>,
    /// The verifier message in the second round of the AHP
    pub(crate) second_round_message: Option<SecondMessage<F>>,
    /// The verifier message in the third round of the AHP
    pub(crate) third_round_message: Option<ThirdMessage<F>>,
    /// The verifier message in the fourth round of the AHP
    pub(crate) fourth_round_message: Option<FourthMessage<F>>,
    /// The verifier's random challenge in the last round of the AHP
    pub(crate) gamma: Option<F>,
    pub(crate) mode: PhantomData<SM>,
}

impl<F: PrimeField, MM: SNARKMode> State<F, MM> {
    pub(crate) fn constraint_domains(&self) -> HashSet<EvaluationDomain<F>> {
        self.circuit_specific_states.values().map(|s| s.constraint_domain).collect()
    }

    pub(crate) fn variable_domains(&self) -> HashSet<EvaluationDomain<F>> {
        self.circuit_specific_states.values().map(|s| s.variable_domain).collect()
    }

    pub(crate) fn non_zero_domains(&self) -> HashSet<EvaluationDomain<F>> {
        self.circuit_specific_states
            .values()
            .flat_map(|s| [s.non_zero_a_domain, s.non_zero_b_domain, s.non_zero_c_domain])
            .collect()
    }
}
