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
        ahp::{
            indexer::{CircuitId, CircuitInfo},
            verifier::{BatchCombiners, FirstMessage, FourthMessage, QuerySet, SecondMessage, State, ThirdMessage},
            AHPError,
            AHPForR1CS,
        },
        verifier::CircuitSpecificState,
        SNARKMode,
    },
    AlgebraicSponge,
};
use anyhow::{ensure, Result};
use smallvec::SmallVec;
use snarkvm_fields::PrimeField;
use std::collections::BTreeMap;

impl<TargetField: PrimeField, SM: SNARKMode> AHPForR1CS<TargetField, SM> {
    /// Output the first message and next round state.
    pub fn verifier_first_round<BaseField: PrimeField, R: AlgebraicSponge<BaseField, 2>>(
        batch_sizes: &BTreeMap<CircuitId, usize>,
        circuit_infos: &BTreeMap<CircuitId, &CircuitInfo>,
        max_constraint_domain: EvaluationDomain<TargetField>,
        max_variable_domain: EvaluationDomain<TargetField>,
        max_non_zero_domain: EvaluationDomain<TargetField>,
        fs_rng: &mut R,
    ) -> Result<(FirstMessage<TargetField>, State<TargetField, SM>)> {
        let mut batch_combiners = BTreeMap::new();
        let mut circuit_specific_states = BTreeMap::new();
        let mut num_circuit_combiners = vec![1; batch_sizes.len()];
        num_circuit_combiners[0] = 0; // the first circuit_combiner is TargetField::one() and needs no random sampling

        for ((batch_size, (circuit_id, circuit_info)), num_c_combiner) in
            batch_sizes.values().zip(circuit_infos).zip(num_circuit_combiners)
        {
            let squeeze_time = start_timer!(|| format!("Squeezing challenges for {circuit_id}"));
            let elems = fs_rng.squeeze_nonnative_field_elements(*batch_size - 1 + num_c_combiner);
            end_timer!(squeeze_time);

            let (instance_combiners, circuit_combiner) = elems.split_at(*batch_size - 1);
            ensure!(circuit_combiner.len() == num_c_combiner);
            let mut combiners =
                BatchCombiners { circuit_combiner: TargetField::one(), instance_combiners: vec![TargetField::one()] };
            if num_c_combiner == 1 {
                combiners.circuit_combiner = circuit_combiner[0];
            }
            combiners.instance_combiners.extend(instance_combiners);
            batch_combiners.insert(*circuit_id, combiners);

            let constraint_domain_time = start_timer!(|| format!("Constructing constraint domain for {circuit_id}"));
            let constraint_domain =
                EvaluationDomain::new(circuit_info.num_constraints).ok_or(AHPError::PolyTooLarge)?;
            end_timer!(constraint_domain_time);

            let variable_domain_time = start_timer!(|| format!("Constructing constraint domain for {circuit_id}"));
            let variable_domain =
                EvaluationDomain::new(circuit_info.num_public_and_private_variables).ok_or(AHPError::PolyTooLarge)?;
            end_timer!(variable_domain_time);

            let non_zero_a_time = start_timer!(|| format!("Constructing non-zero-a domain for {circuit_id}"));
            let non_zero_a_domain = EvaluationDomain::new(circuit_info.num_non_zero_a).ok_or(AHPError::PolyTooLarge)?;
            end_timer!(non_zero_a_time);

            let non_zero_b_time = start_timer!(|| format!("Constructing non-zero-b domain {circuit_id}"));
            let non_zero_b_domain = EvaluationDomain::new(circuit_info.num_non_zero_b).ok_or(AHPError::PolyTooLarge)?;
            end_timer!(non_zero_b_time);

            let non_zero_c_time = start_timer!(|| format!("Constructing non-zero-c domain for {circuit_id}"));
            let non_zero_c_domain = EvaluationDomain::new(circuit_info.num_non_zero_c).ok_or(AHPError::PolyTooLarge)?;
            end_timer!(non_zero_c_time);

            let input_domain_time = start_timer!(|| format!("Constructing input domain {circuit_id}"));
            let input_domain = EvaluationDomain::new(circuit_info.num_public_inputs).ok_or(AHPError::PolyTooLarge)?;
            end_timer!(input_domain_time);

            let circuit_specific_state = CircuitSpecificState {
                input_domain,
                variable_domain,
                constraint_domain,
                non_zero_a_domain,
                non_zero_b_domain,
                non_zero_c_domain,
                batch_size: *batch_size,
            };
            circuit_specific_states.insert(*circuit_id, circuit_specific_state);
        }

        let message = FirstMessage { batch_combiners };

        let new_state = State {
            circuit_specific_states,
            max_constraint_domain,
            max_variable_domain,
            max_non_zero_domain,

            first_round_message: Some(message.clone()),
            second_round_message: None,
            third_round_message: None,
            fourth_round_message: None,

            gamma: None,
            mode: PhantomData,
        };

        Ok((message, new_state))
    }

    /// Output the second message and next round state.
    pub fn verifier_second_round<BaseField: PrimeField, R: AlgebraicSponge<BaseField, 2>>(
        mut state: State<TargetField, SM>,
        fs_rng: &mut R,
    ) -> Result<(SecondMessage<TargetField>, State<TargetField, SM>)> {
        let elems = fs_rng.squeeze_nonnative_field_elements(3);
        let (first, _) = elems.split_at(3);
        let [alpha, eta_b, eta_c]: [_; 3] = first.try_into().map_err(anyhow::Error::msg)?;

        let check_vanish_poly_time = start_timer!(|| "Evaluating vanishing polynomial");
        ensure!(!state.max_constraint_domain.evaluate_vanishing_polynomial(alpha).is_zero());
        end_timer!(check_vanish_poly_time);

        let message = SecondMessage { alpha, eta_b, eta_c };
        state.second_round_message = Some(message);

        Ok((message, state))
    }

    /// Output the third message and next round state.
    pub fn verifier_third_round<BaseField: PrimeField, R: AlgebraicSponge<BaseField, 2>>(
        mut state: State<TargetField, SM>,
        fs_rng: &mut R,
    ) -> Result<(ThirdMessage<TargetField>, State<TargetField, SM>)> {
        let elems = fs_rng.squeeze_nonnative_field_elements(1);
        let beta = elems[0];
        ensure!(!state.max_variable_domain.evaluate_vanishing_polynomial(beta).is_zero());

        let message = ThirdMessage { beta };
        state.third_round_message = Some(message);

        Ok((message, state))
    }

    /// Output the fourth message and next round state.
    pub fn verifier_fourth_round<BaseField: PrimeField, R: AlgebraicSponge<BaseField, 2>>(
        mut state: State<TargetField, SM>,
        fs_rng: &mut R,
    ) -> Result<(FourthMessage<TargetField>, State<TargetField, SM>)> {
        let num_circuits = state.circuit_specific_states.len();
        let mut delta_a = Vec::with_capacity(num_circuits);
        let mut delta_b = Vec::with_capacity(num_circuits);
        let mut delta_c = Vec::with_capacity(num_circuits);
        let first_elems = fs_rng.squeeze_nonnative_field_elements(2);
        delta_a.push(TargetField::one());
        delta_b.push(first_elems[0]);
        delta_c.push(first_elems[1]);
        for _ in 1..num_circuits {
            let elems: SmallVec<[TargetField; 10]> = fs_rng.squeeze_nonnative_field_elements(3);
            delta_a.push(elems[0]);
            delta_b.push(elems[1]);
            delta_c.push(elems[2]);
        }
        let message = FourthMessage { delta_a, delta_b, delta_c };

        state.fourth_round_message = Some(message.clone());
        Ok((message, state))
    }

    /// Output the next round state.
    pub fn verifier_fifth_round<BaseField: PrimeField, R: AlgebraicSponge<BaseField, 2>>(
        mut state: State<TargetField, SM>,
        fs_rng: &mut R,
    ) -> Result<State<TargetField, SM>> {
        let elems = fs_rng.squeeze_nonnative_field_elements(1);
        let gamma = elems[0];
        ensure!(!state.max_non_zero_domain.evaluate_vanishing_polynomial(gamma).is_zero());

        state.gamma = Some(gamma);
        Ok(state)
    }

    /// Output the query state and next round state.
    pub fn verifier_query_set(state: State<TargetField, SM>) -> (QuerySet<TargetField>, State<TargetField, SM>) {
        (QuerySet::new(&state), state)
    }
}
