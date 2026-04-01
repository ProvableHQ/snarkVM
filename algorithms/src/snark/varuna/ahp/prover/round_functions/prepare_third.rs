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

use crate::{
    fft::DensePolynomial,
    snark::varuna::{
        AHPError,
        SNARKMode,
        ahp::{AHPForR1CS, indexer::CircuitId, verifier},
        prover::{self, MatrixSums, ThirdMessage},
    },
};
use snarkvm_fields::PrimeField;
use snarkvm_utilities::ExecutionPool;

use anyhow::Result;
use itertools::Itertools;
use rand::Rng;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

struct LinevalPrepInstance<F: PrimeField> {
    z_m_at_alpha: DensePolynomial<F>,
    sum: F,
}

impl<F: PrimeField, SM: SNARKMode> AHPForR1CS<F, SM> {
    /// Output the preparation third round message and the next state.
    pub fn prover_prepare_third_round<'a, R: Rng>(
        verifier_message: &verifier::FirstMessage<F>,
        verifier_second_message: &verifier::SecondMessage<F>,
        mut state: prover::State<'a, F, SM>,
        _r: &mut R,
    ) -> Result<(prover::ThirdMessage<F>, prover::State<'a, F, SM>), AHPError> {
        let round_time = start_timer!(|| "AHP::Prover::ThirdRound");

        let verifier::FirstMessage { first_round_batch_combiners } = verifier_message;
        let verifier::SecondMessage { alpha, eta_b, eta_c } = verifier_second_message;

        if eta_b.is_some() || eta_c.is_some() {
            return Err(AHPError::AnyhowError(anyhow::anyhow!(
                "Did not expect eta_b,c in SecondMessage in VarunaVersion::V2"
            )));
        }

        let assignments = Self::calculate_assignments(&mut state)?;

        let msg = Self::calculate_prep_lineval_sumcheck_witness(
            &mut state,
            first_round_batch_combiners,
            assignments,
            alpha,
        )?;

        end_timer!(round_time);

        Ok((msg, state))
    }

    fn calculate_prep_lineval_sumcheck_witness(
        state: &mut prover::State<F, SM>,
        first_round_batch_combiners: &BTreeMap<CircuitId, verifier::BatchCombiners<F>>,
        assignments: BTreeMap<CircuitId, Vec<DensePolynomial<F>>>,
        alpha: &F,
    ) -> Result<ThirdMessage<F>> {
        let num_instances = first_round_batch_combiners.values().map(|c| c.instance_combiners.len()).collect_vec();
        let total_instances = num_instances.iter().sum::<usize>();
        let matrix_labels = ["a", "b", "c"];

        // Compute lineval sumcheck witnesses
        let mut job_pool = ExecutionPool::with_capacity(total_instances * 3);
        // Iterate for each circuit in the batch.
        anyhow::ensure!(
            state.circuit_specific_states.len() == first_round_batch_combiners.len(),
            "[calculate Prep Lineval Sumcheck Witness] Expected {} circuit specific states, but {} were provided.",
            first_round_batch_combiners.len(),
            state.circuit_specific_states.len()
        );
        anyhow::ensure!(
            state.circuit_specific_states.len() == assignments.len(),
            "[calculate Prep Lineval Sumcheck Witness] Expected {} assignments, but {} were provided.",
            assignments.len(),
            state.circuit_specific_states.len()
        );
        for ((&circuit, circuit_specific_state), assignments_i) in
            state.circuit_specific_states.iter().zip_eq(assignments.values())
        {
            // Precompute l_at_alpha once per circuit (alpha is fixed for the entire round;
            // all instances and matrices of this circuit share the same Lagrange coefficients).
            let l_at_alpha =
                Arc::new(circuit_specific_state.constraint_domain.evaluate_all_lagrange_coefficients(*alpha));

            // Precompute col_reindex table once per circuit to replace per-entry
            // reindex_by_subdomain calls with an O(1) table lookup.
            let col_reindex = Arc::new(
                (0..circuit_specific_state.variable_domain.size())
                    .map(|i| {
                        circuit_specific_state
                            .variable_domain
                            .reindex_by_subdomain(&circuit_specific_state.input_domain, i)
                            .unwrap()
                    })
                    .collect::<Vec<usize>>(),
            );

            // Iterate for each instance in the batch.
            for assignment in assignments_i {
                // Iterate for each R1CS matrix corresponding to the circuit and instance.
                for label in matrix_labels {
                    let matrix = match label {
                        "a" => &circuit.a,
                        "b" => &circuit.b,
                        _ => &circuit.c,
                    };
                    let l_at_alpha_clone = Arc::clone(&l_at_alpha);
                    let col_reindex_clone = Arc::clone(&col_reindex);
                    job_pool.add_job(move || {
                        let z_m_at_alpha = Self::calculate_lineval_sumcheck_instance_witness(
                            label,
                            &circuit_specific_state.variable_domain,
                            &circuit.fft_precomputation,
                            &circuit.ifft_precomputation,
                            assignment,
                            matrix,
                            &l_at_alpha_clone,
                            &col_reindex_clone,
                        )?;

                        // O(1) sum formula: Σ_{x∈H} p(x) = n·(c_0 + c_n) for degree < 2n
                        // polynomial over multiplicative domain H of size n.
                        let n = circuit_specific_state.variable_domain.size_as_field_element;
                        let c_0 = z_m_at_alpha.coeffs.first().copied().unwrap_or_else(snarkvm_fields::Zero::zero);
                        let c_n = z_m_at_alpha
                            .coeffs
                            .get(circuit_specific_state.variable_domain.size())
                            .copied()
                            .unwrap_or_else(snarkvm_fields::Zero::zero);
                        let sum = n * (c_0 + c_n);
                        Ok((circuit, LinevalPrepInstance { z_m_at_alpha, sum }))
                    });
                }
            }
        }

        let mut sums = num_instances.iter().map(|n| Vec::with_capacity(*n)).collect_vec();
        let mut circuit_index = 0;
        let mut instances_seen = 0;
        for (i, ((circuit_a, lineval_a), (circuit_b, lineval_b), (circuit_c, lineval_c))) in
            job_pool.execute_all().into_iter().collect::<Result<Vec<_>>>()?.into_iter().tuples().enumerate()
        {
            // Sanity check that we're collecting data from the same circuits.
            assert_eq!(circuit_a, circuit_b);
            assert_eq!(circuit_a, circuit_c);
            sums[circuit_index].push(MatrixSums { sum_a: lineval_a.sum, sum_b: lineval_b.sum, sum_c: lineval_c.sum });
            if 1 + i - instances_seen == num_instances[circuit_index] {
                instances_seen += num_instances[circuit_index];
                circuit_index += 1;
            }

            // Store the z_m_at_alpha polynomials for use in the next round.
            match &mut state.circuit_specific_states.get_mut(circuit_a).unwrap().z_m_at_alpha_polys {
                None => {
                    let mut z_m_at_alpha_polys = VecDeque::new();
                    z_m_at_alpha_polys.push_back([
                        lineval_a.z_m_at_alpha,
                        lineval_b.z_m_at_alpha,
                        lineval_c.z_m_at_alpha,
                    ]);
                    state.circuit_specific_states.get_mut(circuit_a).unwrap().z_m_at_alpha_polys =
                        Some(z_m_at_alpha_polys);
                }
                Some(z_m_at_alpha_polys) => {
                    z_m_at_alpha_polys.push_back([
                        lineval_a.z_m_at_alpha,
                        lineval_b.z_m_at_alpha,
                        lineval_c.z_m_at_alpha,
                    ]);
                }
            }
        }

        let msg = ThirdMessage { sums };

        Ok(msg)
    }
}
