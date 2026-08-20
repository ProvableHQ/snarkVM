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

use crate::snark::{
    provekit::{PoseidonPermutationCircuit, ProvekitSNARK, adapter::synthesize, common::PublicInputs, proof_size},
    varuna::TestCircuit,
};
use snarkvm_curves::bls12_377::Fr as SnarkFr;
use snarkvm_fields::{One, Zero};
use snarkvm_utilities::TestRng;

fn tiny_circuit() -> crate::snark::provekit::SynthesizedCircuit {
    let rng = &mut TestRng::default();
    let (circuit, _) = TestCircuit::<SnarkFr>::gen_rand(1, 8, 8, rng);
    synthesize(&circuit).expect("test circuit should synthesize")
}

#[test]
fn prove_verify_tiny_circuit() {
    let synthesized = tiny_circuit();
    let scheme = ProvekitSNARK::setup(&synthesized.r1cs);
    let proof =
        ProvekitSNARK::prove(&scheme, &synthesized.r1cs, synthesized.witness.clone(), &synthesized.public_inputs)
            .expect("proving should succeed");
    assert!(proof_size(&proof) > 0);
    assert!(
        ProvekitSNARK::verify(&scheme, &synthesized.r1cs, &synthesized.public_inputs, &proof)
            .expect("verify should return")
    );
}

#[test]
fn tampered_proof_fails_verify() {
    let synthesized = tiny_circuit();
    let scheme = ProvekitSNARK::setup(&synthesized.r1cs);
    let mut proof =
        ProvekitSNARK::prove(&scheme, &synthesized.r1cs, synthesized.witness.clone(), &synthesized.public_inputs)
            .expect("proving should succeed");
    if proof.narg_string.is_empty() {
        proof.hints.push(0);
    } else {
        proof.narg_string[0] ^= 0xff;
    }
    assert!(
        !ProvekitSNARK::verify(&scheme, &synthesized.r1cs, &synthesized.public_inputs, &proof)
            .expect("verify should return")
    );
}

#[test]
fn wrong_public_inputs_fail_verify() {
    let synthesized = tiny_circuit();
    let scheme = ProvekitSNARK::setup(&synthesized.r1cs);
    let proof =
        ProvekitSNARK::prove(&scheme, &synthesized.r1cs, synthesized.witness.clone(), &synthesized.public_inputs)
            .expect("proving should succeed");

    let mut wrong = synthesized.public_inputs.0.clone();
    if let Some(first) = wrong.first_mut() {
        *first += <SnarkFr as One>::one();
    } else {
        wrong.push(<SnarkFr as One>::one());
    }
    let wrong_inputs = PublicInputs::from_vec(wrong);
    assert!(!ProvekitSNARK::verify(&scheme, &synthesized.r1cs, &wrong_inputs, &proof).expect("verify should return"));
}

#[test]
fn prove_verify_poseidon_permutations() {
    let circuit = PoseidonPermutationCircuit::new(SnarkFr::from(7u64), 2);
    let synthesized = synthesize(&circuit).expect("poseidon circuit should synthesize");
    let scheme = ProvekitSNARK::setup(&synthesized.r1cs);
    let proof =
        ProvekitSNARK::prove(&scheme, &synthesized.r1cs, synthesized.witness.clone(), &synthesized.public_inputs)
            .expect("proving should succeed");
    assert!(
        ProvekitSNARK::verify(&scheme, &synthesized.r1cs, &synthesized.public_inputs, &proof)
            .expect("verify should return")
    );
}

#[test]
fn fr_byte_roundtrip() {
    use snarkvm_utilities::{FromBytes, ToBytes};
    let values = [<SnarkFr as Zero>::zero(), <SnarkFr as One>::one(), -<SnarkFr as One>::one()];
    for value in values {
        let mut bytes = [0u8; 32];
        value.write_le(&mut bytes[..]).expect("Fr is 32 bytes");
        let back = SnarkFr::read_le(&bytes[..]).expect("Fr roundtrip");
        assert_eq!(value, back);
    }
}
