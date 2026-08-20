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

//! Chained Poseidon permutations as an R1CS circuit.
//!
//! Mirrors the ProofBench ProveKit workload (`poseidon2` in `../proofbench`):
//! start from `[seed, 0, …]`, apply `N` permutations, and expose the first
//! state element as a public output. `N` is chosen so the native R1CS count
//! lands near a target (typically `2^14` or `2^16`).
//!
//! This uses snarkVM Poseidon-2 over BLS12-377 `Fr` (rate 2, width 3, `α =
//! 17`), not Noir Poseidon2-width-4 over BN254.

use crate::r1cs::{ConstraintSynthesizer, ConstraintSystem, Index, LinearCombination, SynthesisError, Variable};
use snarkvm_curves::bls12_377::Fr;
use snarkvm_fields::{PoseidonDefaultField, PoseidonParameters, Zero};

/// Poseidon rate matching snarkVM's `Poseidon2` hash.
const RATE: usize = 2;
/// Sponge width (`RATE + CAPACITY`).
const WIDTH: usize = RATE + 1;

/// `N` chained Poseidon permutations of a width-`WIDTH` state.
pub struct PoseidonPermutationCircuit {
    seed: Fr,
    num_permutations: usize,
    params: PoseidonParameters<Fr, RATE, 1>,
}

impl PoseidonPermutationCircuit {
    /// Builds a circuit that applies `num_permutations` Poseidon permutations
    /// to `[seed, 0, 0]`.
    pub fn new(seed: Fr, num_permutations: usize) -> Self {
        Self {
            seed,
            num_permutations,
            // RATE=2 is in the default BLS12-377 Fr parameter table.
            params: Fr::default_poseidon_parameters::<RATE>().expect("Poseidon parameters exist for rate 2"),
        }
    }

    /// Number of chained permutations.
    pub fn num_permutations(&self) -> usize {
        self.num_permutations
    }
}

fn one() -> Variable {
    Variable::new_unchecked(Index::Public(0))
}

struct Assigned {
    lc: LinearCombination<Fr>,
    value: Fr,
}

impl Assigned {
    fn constant(value: Fr) -> Self {
        Self { lc: LinearCombination::zero() + (value, one()), value }
    }

    fn from_variable(var: crate::r1cs::Variable, value: Fr) -> Self {
        Self { lc: var.into(), value }
    }
}

fn mul<CS: ConstraintSystem<Fr>>(cs: &mut CS, a: &Assigned, b: &Assigned) -> Result<Assigned, SynthesisError> {
    let value = a.value * b.value;
    let var = cs.alloc(|| "mul", || Ok(value))?;
    cs.enforce(|| "mul", |_| a.lc.clone(), |_| b.lc.clone(), |lc| lc + var);
    Ok(Assigned::from_variable(var, value))
}

/// `x^17` via four squarings then a multiply (`α = 17` for BLS12-377 Fr rate
/// 2).
fn pow_17<CS: ConstraintSystem<Fr>>(cs: &mut CS, x: &Assigned) -> Result<Assigned, SynthesisError> {
    let x2 = mul(cs, x, x)?;
    let x4 = mul(cs, &x2, &x2)?;
    let x8 = mul(cs, &x4, &x4)?;
    let x16 = mul(cs, &x8, &x8)?;
    mul(cs, &x16, x)
}

fn apply_ark(state: &mut [Assigned], ark: &[Fr]) {
    for (slot, ark_elem) in state.iter_mut().zip(ark.iter()) {
        slot.lc = slot.lc.clone() + (*ark_elem, one());
        slot.value += ark_elem;
    }
}

fn apply_s_box<CS: ConstraintSystem<Fr>>(
    cs: &mut CS,
    state: &mut [Assigned],
    is_full_round: bool,
) -> Result<(), SynthesisError> {
    if is_full_round {
        for slot in state.iter_mut() {
            *slot = pow_17(cs, slot)?;
        }
    } else {
        state[0] = pow_17(cs, &state[0])?;
    }
    Ok(())
}

fn apply_mds(state: &[Assigned], mds: &[Vec<Fr>]) -> Vec<Assigned> {
    mds.iter()
        .map(|row| {
            let mut lc = LinearCombination::zero();
            let mut value = Fr::zero();
            for (coeff, slot) in row.iter().zip(state.iter()) {
                lc = lc + (*coeff, &slot.lc);
                value += *coeff * slot.value;
            }
            Assigned { lc, value }
        })
        .collect()
}

fn permute<CS: ConstraintSystem<Fr>>(
    cs: &mut CS,
    mut state: Vec<Assigned>,
    params: &PoseidonParameters<Fr, RATE, 1>,
) -> Result<Vec<Assigned>, SynthesisError> {
    let partial_rounds = params.partial_rounds;
    let full_rounds = params.full_rounds;
    let full_rounds_over_2 = full_rounds / 2;
    let partial_round_range = full_rounds_over_2..(full_rounds_over_2 + partial_rounds);

    for round in 0..(partial_rounds + full_rounds) {
        apply_ark(&mut state, &params.ark[round]);
        apply_s_box(cs, &mut state, !partial_round_range.contains(&round))?;
        state = apply_mds(&state, &params.mds);
    }
    Ok(state)
}

impl ConstraintSynthesizer<Fr> for PoseidonPermutationCircuit {
    fn generate_constraints<CS: ConstraintSystem<Fr>>(&self, cs: &mut CS) -> Result<(), SynthesisError> {
        debug_assert_eq!(self.params.alpha, 17);

        let seed_var = cs.alloc(|| "seed", || Ok(self.seed))?;
        let mut state = vec![Assigned::from_variable(seed_var, self.seed)];
        for _ in 1..WIDTH {
            state.push(Assigned::constant(Fr::zero()));
        }

        for _ in 0..self.num_permutations {
            state = permute(cs, state, &self.params)?;
        }

        let output = cs.alloc_input(|| "output", || Ok(state[0].value))?;
        cs.enforce(|| "output", |lc| lc + CS::one(), |_| state[0].lc.clone(), |lc| lc + output);

        Ok(())
    }
}
