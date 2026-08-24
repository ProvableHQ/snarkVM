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

use crate::{Air, AirBuilder, BaseAir, Trace, exp_u64};
use snarkvm_fields::{One, PoseidonDefaultField, PoseidonParameters, PrimeField, Zero};

use anyhow::{Result, ensure};
use itertools::Itertools;
use std::sync::Arc;

/// Capacity of the Poseidon sponge used throughout snarkVM.
const CAPACITY: usize = 1;

/// Native Poseidon permutation AIR: one round per transition.
///
/// Main columns are the `RATE + 1` state words. Preprocessed columns are the
/// round-key vector followed by an `is_full_round` selector. The last row is
/// the state after the final round and has no transition constraint.
#[derive(Clone, Debug)]
pub struct PoseidonAir<F: PrimeField, const RATE: usize> {
    parameters: Arc<PoseidonParameters<F, RATE, CAPACITY>>,
}

impl<F: PrimeField, const RATE: usize> PoseidonAir<F, RATE> {
    /// Width of the Poseidon state (`RATE + CAPACITY`).
    pub const WIDTH: usize = RATE + CAPACITY;

    /// Constructs an AIR from Poseidon parameters.
    pub fn new(parameters: PoseidonParameters<F, RATE, CAPACITY>) -> Self {
        Self { parameters: Arc::new(parameters) }
    }

    /// Constructs an AIR from the field's default Poseidon parameters.
    pub fn setup() -> Result<Self>
    where
        F: PoseidonDefaultField,
    {
        Ok(Self::new(F::default_poseidon_parameters::<RATE>()?))
    }

    /// Returns the underlying Poseidon parameters.
    pub fn parameters(&self) -> &PoseidonParameters<F, RATE, CAPACITY> {
        &self.parameters
    }

    /// Returns the number of permutation rounds.
    pub fn num_rounds(&self) -> usize {
        self.parameters.full_rounds + self.parameters.partial_rounds
    }

    /// Applies the Poseidon permutation to `state` in place.
    pub fn permute(&self, state: &mut [F]) {
        debug_assert_eq!(state.len(), Self::WIDTH, "Poseidon state width must be RATE + 1");
        for round in 0..self.num_rounds() {
            apply_round(state, round, &self.parameters);
        }
    }

    /// Builds a main trace whose first row is `initial` and whose last row is the
    /// fully permuted state.
    pub fn generate_trace(&self, initial: &[F]) -> Result<Trace<F>> {
        ensure!(initial.len() == Self::WIDTH, "initial state length {} != width {}", initial.len(), Self::WIDTH);
        let height = self.num_rounds() + 1;
        let mut state = initial.to_vec();
        let mut values = Vec::with_capacity(height.saturating_mul(Self::WIDTH));
        values.extend_from_slice(&state);
        for round in 0..self.num_rounds() {
            apply_round(&mut state, round, &self.parameters);
            values.extend_from_slice(&state);
        }
        Trace::new(Self::WIDTH, height, values)
    }
}

impl<F: PrimeField, const RATE: usize> BaseAir<F> for PoseidonAir<F, RATE> {
    fn width(&self) -> usize {
        Self::WIDTH
    }

    fn preprocessed_width(&self) -> usize {
        Self::WIDTH + 1
    }

    fn preprocessed_trace(&self) -> Option<Trace<F>> {
        let height = self.num_rounds() + 1;
        let prep_width = self.preprocessed_width();
        let mut values = vec![F::zero(); height.saturating_mul(prep_width)];
        let partial_start = self.parameters.full_rounds / 2;
        let partial_end = partial_start + self.parameters.partial_rounds;
        for round in 0..self.num_rounds() {
            let row = &mut values[round * prep_width..(round + 1) * prep_width];
            for (cell, ark) in row.iter_mut().take(Self::WIDTH).zip_eq(&self.parameters.ark[round]) {
                *cell = *ark;
            }
            row[Self::WIDTH] = if (partial_start..partial_end).contains(&round) { F::zero() } else { F::one() };
        }
        Some(Trace::new(prep_width, height, values).expect("preprocessed Poseidon trace dimensions are consistent"))
    }
}

impl<AB: AirBuilder, const RATE: usize> Air<AB> for PoseidonAir<AB::F, RATE> {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        // `BaseAir::preprocessed_trace` always returns `Some` for this AIR.
        let prep = builder.preprocessed().expect("PoseidonAir requires a preprocessed trace");
        let local = main.local();
        let next = main.next();
        let prep_local = prep.local();

        let is_full: AB::Expr = prep_local[Self::WIDTH].into();
        let one = AB::Expr::one();
        let alpha = self.parameters.alpha;

        let mut sboxed = Vec::with_capacity(Self::WIDTH);
        for i in 0..Self::WIDTH {
            let y = local[i].into() + prep_local[i].into();
            let y_alpha = exp_u64(y.clone(), alpha);
            if i == 0 {
                sboxed.push(y_alpha);
            } else {
                sboxed.push(is_full.clone() * y_alpha + (one.clone() - is_full.clone()) * y);
            }
        }

        let mut transition = builder.when_transition();
        for (i, mds_row) in self.parameters.mds.iter().enumerate() {
            let mut acc = AB::Expr::zero();
            for (sbox_j, mds_ij) in sboxed.iter().zip_eq(mds_row) {
                acc = acc + AB::Expr::from(*mds_ij) * sbox_j.clone();
            }
            transition.assert_eq(next[i], acc);
        }
    }
}

fn apply_round<F: PrimeField, const RATE: usize>(
    state: &mut [F],
    round: usize,
    parameters: &PoseidonParameters<F, RATE, CAPACITY>,
) {
    for (state_elem, ark_elem) in state.iter_mut().zip_eq(&parameters.ark[round]) {
        *state_elem += *ark_elem;
    }

    let partial_start = parameters.full_rounds / 2;
    let is_full_round = !(partial_start..partial_start + parameters.partial_rounds).contains(&round);
    if is_full_round {
        for state_elem in state.iter_mut() {
            *state_elem = state_elem.pow([parameters.alpha]);
        }
    } else {
        state[0] = state[0].pow([parameters.alpha]);
    }

    let current = state.to_vec();
    for (state_elem, mds_row) in state.iter_mut().zip_eq(&parameters.mds) {
        *state_elem = F::sum_of_products(&current, mds_row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SymbolicAirBuilder, debug_constraints};
    use snarkvm_curves::bls12_377::Fr;
    use snarkvm_fields::One;
    use snarkvm_utilities::{TestRng, Uniform};

    const RATE: usize = 2;

    fn random_state(rng: &mut TestRng) -> [Fr; PoseidonAir::<Fr, RATE>::WIDTH] {
        core::array::from_fn(|_| Uniform::rand(rng))
    }

    #[test]
    fn test_poseidon_air_matches_cpu_permutation() {
        let air = PoseidonAir::<Fr, RATE>::setup().unwrap();
        let mut rng = TestRng::default();
        let initial = random_state(&mut rng);

        let mut expected = initial;
        air.permute(&mut expected);

        let trace = air.generate_trace(&initial).unwrap();
        assert_eq!(air.num_rounds() + 1, trace.height());
        assert_eq!(initial.as_slice(), trace.row(0));
        assert_eq!(expected.as_slice(), trace.row(trace.height() - 1));
        debug_constraints(&air, &trace).unwrap();

        let symbolic = SymbolicAirBuilder::constraints_of(&air);
        assert_eq!(PoseidonAir::<Fr, RATE>::WIDTH, symbolic.len());
    }

    #[test]
    fn test_poseidon_air_rejects_a_mutated_trace() {
        let air = PoseidonAir::<Fr, RATE>::setup().unwrap();
        let mut rng = TestRng::default();
        let initial = random_state(&mut rng);
        let mut trace = air.generate_trace(&initial).unwrap();
        *trace.get_mut(trace.height() / 2, 0) += Fr::one();
        assert!(debug_constraints(&air, &trace).is_err());
    }
}
