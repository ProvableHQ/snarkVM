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

//! ProveKit SNARK facade over BLS12-377.

use super::{
    bls12_377::{Bls12_377Field, register},
    common::{FieldHash, HashConfig, PublicInputs, PublicInputsHash, R1CS, WhirR1CSProof, WhirR1CSScheme},
    prover::WhirR1CSProver,
    verifier::WhirR1CSVerifier,
};
use crate::snark::provekit::whir::transcript::ProverState;
use anyhow::Result;
use snarkvm_curves::bls12_377::Fr;

/// ProveKit (Spartan + WHIR) instantiated over BLS12-377 `Fr` with Blake3.
pub struct ProvekitSNARK;

impl ProvekitSNARK {
    /// Derive WHIR/Spartan parameters from `r1cs`. Setup is transparent.
    pub fn setup(r1cs: &R1CS<Fr>) -> WhirR1CSScheme<Bls12_377Field> {
        register();
        let w1_size = r1cs.num_witnesses();
        let has_public_inputs = r1cs.num_public_inputs > 0;
        WhirR1CSScheme::<Bls12_377Field>::new_for_r1cs(
            r1cs,
            w1_size,
            0,
            Vec::new(),
            has_public_inputs,
            HashConfig::Blake3,
        )
    }

    /// Prove satisfaction of `r1cs` at `witness`.
    pub fn prove(
        scheme: &WhirR1CSScheme<Bls12_377Field>,
        r1cs: &R1CS<Fr>,
        witness: Vec<Fr>,
        public_inputs: &PublicInputs<Fr>,
    ) -> Result<WhirR1CSProof> {
        register();
        let instance = public_inputs.hash_bytes::<Bls12_377Field>(scheme.hash_config);
        let ds = scheme.create_domain_separator().instance(&instance);
        let mut merlin = ProverState::new(&ds, Bls12_377Field::transcript_sponge(scheme.hash_config));
        let num_witnesses = r1cs.num_witnesses();
        let num_constraints = r1cs.num_constraints();
        let commitment = scheme.commit(&mut merlin, num_witnesses, num_constraints, witness.clone(), true)?;
        scheme.prove_noir(merlin, r1cs, vec![commitment], witness, public_inputs)
    }

    /// Verify `proof` against `r1cs` and `public_inputs`.
    pub fn verify(
        scheme: &WhirR1CSScheme<Bls12_377Field>,
        r1cs: &R1CS<Fr>,
        public_inputs: &PublicInputs<Fr>,
        proof: &WhirR1CSProof,
    ) -> Result<bool> {
        register();
        Ok(scheme.verify(proof, public_inputs, r1cs).is_ok())
    }
}

/// Serialized proof size in bytes (`narg_string` plus `hints`).
pub fn proof_size(proof: &WhirR1CSProof) -> usize {
    proof.narg_string.len() + proof.hints.len()
}
