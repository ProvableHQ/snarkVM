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
use ark_bls12_377::Fr as ArkFr;
use ark_ff::PrimeField as ArkPrimeField;
use snarkvm_curves::bls12_377::Fr as SnarkFr;
use snarkvm_fields::{One, PrimeField};
use snarkvm_utilities::ToBytes;

/// An R1CS instance plus witness, ready for ProveKit setup/prove/verify.
pub struct SynthesizedCircuit {
    /// ProveKit R1CS matrices over arkworks BLS12-377 `Fr`.
    pub r1cs: R1CS<ArkFr>,
    /// Full witness, including the constant-one at index 0.
    pub witness: Vec<ArkFr>,
    /// Public inputs excluding the constant-one (Fiat-Shamir binding vector).
    pub public_inputs: PublicInputs<ArkFr>,
}

/// Synthesize `circuit` into a ProveKit R1CS instance over BLS12-377 `Fr`.
pub fn synthesize<C: ConstraintSynthesizer<SnarkFr>>(circuit: &C) -> Result<SynthesizedCircuit, SynthesisError> {
    let mut cs = CollectingConstraintSystem::new();
    circuit.generate_constraints(&mut cs)?;
    Ok(cs.into_synthesized())
}

/// Convert a snarkVM BLS12-377 scalar into the arkworks representation.
pub fn snarkvm_fr_to_ark(value: SnarkFr) -> ArkFr {
    let mut bytes = [0u8; 32];
    value.write_le(&mut bytes[..]).expect("BLS12-377 Fr is 32 little-endian bytes");
    ArkFr::from_le_bytes_mod_order(&bytes)
}

/// Convert an arkworks BLS12-377 scalar into the snarkVM representation.
pub fn ark_fr_to_snarkvm(value: ArkFr) -> SnarkFr {
    let limbs = value.into_bigint().0;
    let mut bytes = [0u8; 32];
    for (i, &limb) in limbs.iter().enumerate() {
        bytes[i * 8..(i + 1) * 8].copy_from_slice(&limb.to_le_bytes());
    }
    SnarkFr::from_bytes_le_mod_order(&bytes)
}

struct CollectingConstraintSystem {
    public_variables: Vec<SnarkFr>,
    private_variables: Vec<SnarkFr>,
    constraints: Vec<(LinearCombination<SnarkFr>, LinearCombination<SnarkFr>, LinearCombination<SnarkFr>)>,
}

impl CollectingConstraintSystem {
    fn new() -> Self {
        Self { public_variables: vec![<SnarkFr as One>::one()], private_variables: Vec::new(), constraints: Vec::new() }
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
        witness.extend(self.public_variables.iter().copied().map(snarkvm_fr_to_ark));
        witness.extend(self.private_variables.iter().copied().map(snarkvm_fr_to_ark));

        let public_inputs =
            if num_public > 1 { PublicInputs::from_vec(witness[1..num_public].to_vec()) } else { PublicInputs::new() };

        SynthesizedCircuit { r1cs, witness, public_inputs }
    }
}

fn lc_to_terms(lc: &LinearCombination<SnarkFr>, num_public: usize) -> Vec<(ArkFr, usize)> {
    lc.as_ref().iter().map(|(var, coeff)| (snarkvm_fr_to_ark(*coeff), var_to_column(*var, num_public))).collect()
}

fn var_to_column(var: Variable, num_public: usize) -> usize {
    match var.get_unchecked() {
        Index::Public(index) => index,
        Index::Private(index) => num_public + index,
    }
}

impl ConstraintSystem<SnarkFr> for CollectingConstraintSystem {
    type Root = Self;

    fn alloc<FN, A, AR>(&mut self, _: A, f: FN) -> Result<Variable, SynthesisError>
    where
        FN: FnOnce() -> Result<SnarkFr, SynthesisError>,
        A: FnOnce() -> AR,
        AR: AsRef<str>,
    {
        let index = self.private_variables.len();
        self.private_variables.push(f()?);
        Ok(Variable::new_unchecked(Index::Private(index)))
    }

    fn alloc_input<FN, A, AR>(&mut self, _: A, f: FN) -> Result<Variable, SynthesisError>
    where
        FN: FnOnce() -> Result<SnarkFr, SynthesisError>,
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
        LA: FnOnce(LinearCombination<SnarkFr>) -> LinearCombination<SnarkFr>,
        LB: FnOnce(LinearCombination<SnarkFr>) -> LinearCombination<SnarkFr>,
        LC: FnOnce(LinearCombination<SnarkFr>) -> LinearCombination<SnarkFr>,
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
