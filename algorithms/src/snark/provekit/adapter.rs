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

//! Convert a snarkVM [`ConstraintSynthesizer`] into a ProveKit R1CS instance.

use crate::{
    r1cs::{ConstraintSynthesizer, ConstraintSystem, Index, LinearCombination, Variable, errors::SynthesisError},
    snark::provekit::common::{PublicInputs, R1CS},
};
use snarkvm_curves::bls12_377::Fr;
use snarkvm_fields::One;

/// An R1CS instance plus witness, ready for ProveKit setup/prove/verify.
pub struct SynthesizedCircuit {
    /// ProveKit R1CS matrices over BLS12-377 `Fr`.
    pub r1cs: R1CS<Fr>,
    /// Full witness, including the constant-one at index 0.
    pub witness: Vec<Fr>,
    /// Public inputs excluding the constant-one (Fiat-Shamir binding vector).
    pub public_inputs: PublicInputs<Fr>,
}

/// Synthesize `circuit` into a ProveKit R1CS instance over BLS12-377 `Fr`.
pub fn synthesize<C: ConstraintSynthesizer<Fr>>(circuit: &C) -> Result<SynthesizedCircuit, SynthesisError> {
    let mut cs = CollectingConstraintSystem::new();
    circuit.generate_constraints(&mut cs)?;
    Ok(cs.into_synthesized())
}

struct CollectingConstraintSystem {
    public_variables: Vec<Fr>,
    private_variables: Vec<Fr>,
    constraints: Vec<(LinearCombination<Fr>, LinearCombination<Fr>, LinearCombination<Fr>)>,
}

impl CollectingConstraintSystem {
    fn new() -> Self {
        Self { public_variables: vec![<Fr as One>::one()], private_variables: Vec::new(), constraints: Vec::new() }
    }

    fn into_synthesized(self) -> SynthesizedCircuit {
        let num_public = self.public_variables.len();
        let num_private = self.private_variables.len();
        let num_witnesses = num_public + num_private;

        let mut r1cs = R1CS::new();
        r1cs.add_witnesses(num_witnesses);
        r1cs.num_public_inputs = num_public.saturating_sub(1);
        r1cs.reserve_constraints(self.constraints.len(), self.constraints.len() * 3);

        for (a, b, c) in &self.constraints {
            r1cs.add_constraint(&lc_to_terms(a, num_public), &lc_to_terms(b, num_public), &lc_to_terms(c, num_public));
        }

        let mut witness = Vec::with_capacity(num_witnesses);
        witness.extend(self.public_variables);
        witness.extend(self.private_variables);

        let public_inputs =
            if num_public > 1 { PublicInputs::from_vec(witness[1..num_public].to_vec()) } else { PublicInputs::new() };

        SynthesizedCircuit { r1cs, witness, public_inputs }
    }
}

fn lc_to_terms(lc: &LinearCombination<Fr>, num_public: usize) -> Vec<(Fr, usize)> {
    lc.as_ref().iter().map(|(var, coeff)| (*coeff, var_to_column(*var, num_public))).collect()
}

fn var_to_column(var: Variable, num_public: usize) -> usize {
    match var.get_unchecked() {
        Index::Public(index) => index,
        Index::Private(index) => num_public + index,
    }
}

impl ConstraintSystem<Fr> for CollectingConstraintSystem {
    type Root = Self;

    fn alloc<FN, A, AR>(&mut self, _: A, f: FN) -> Result<Variable, SynthesisError>
    where
        FN: FnOnce() -> Result<Fr, SynthesisError>,
        A: FnOnce() -> AR,
        AR: AsRef<str>,
    {
        let index = self.private_variables.len();
        self.private_variables.push(f()?);
        Ok(Variable::new_unchecked(Index::Private(index)))
    }

    fn alloc_input<FN, A, AR>(&mut self, _: A, f: FN) -> Result<Variable, SynthesisError>
    where
        FN: FnOnce() -> Result<Fr, SynthesisError>,
        A: FnOnce() -> AR,
        AR: AsRef<str>,
    {
        let index = self.public_variables.len();
        self.public_variables.push(f()?);
        Ok(Variable::new_unchecked(Index::Public(index)))
    }

    fn enforce<A, AR, LA, LB, LC>(&mut self, _: A, a: LA, b: LB, c: LC)
    where
        A: FnOnce() -> AR,
        AR: AsRef<str>,
        LA: FnOnce(LinearCombination<Fr>) -> LinearCombination<Fr>,
        LB: FnOnce(LinearCombination<Fr>) -> LinearCombination<Fr>,
        LC: FnOnce(LinearCombination<Fr>) -> LinearCombination<Fr>,
    {
        self.constraints.push((
            a(LinearCombination::zero()),
            b(LinearCombination::zero()),
            c(LinearCombination::zero()),
        ));
    }

    fn push_namespace<NR, N>(&mut self, _: N)
    where
        NR: AsRef<str>,
        N: FnOnce() -> NR,
    {
    }

    fn pop_namespace(&mut self) {}

    fn get_root(&mut self) -> &mut Self::Root {
        self
    }

    fn num_constraints(&self) -> usize {
        self.constraints.len()
    }

    fn num_public_variables(&self) -> usize {
        self.public_variables.len()
    }

    fn num_private_variables(&self) -> usize {
        self.private_variables.len()
    }

    fn is_in_setup_mode(&self) -> bool {
        false
    }
}
