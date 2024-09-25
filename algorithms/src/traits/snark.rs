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

use crate::{r1cs::ConstraintSynthesizer, AlgebraicSponge};
use snarkvm_fields::PrimeField;
use snarkvm_utilities::{CanonicalDeserialize, CanonicalSerialize, FromBytes, ToBytes};

use anyhow::Result;
use rand::{CryptoRng, Rng};
use std::{borrow::Borrow, collections::BTreeMap, fmt::Debug};

/// Defines trait that describes preparing from an unprepared version to a prepare version.
pub trait Prepare {
    type Prepared;
    fn prepare(&self) -> Self::Prepared;
}

pub trait SNARK {
    type ScalarField: Clone + PrimeField;
    type BaseField: Clone + PrimeField;

    /// A certificate that the indexing was performed correctly.
    type Certificate: CanonicalSerialize
        + CanonicalDeserialize
        + Clone
        + Debug
        + ToBytes
        + FromBytes
        + PartialEq
        + Eq
        + Send
        + Sync;
    type Proof: Clone + Debug + ToBytes + FromBytes + PartialEq + Eq + Send + Sync;
    type ProvingKey: Clone + ToBytes + FromBytes + Send + Sync + Ord;

    type UniversalSRS: Clone;
    type UniversalProver;
    type UniversalVerifier;

    type VerifierInput: ?Sized;
    type VerifyingKey: Clone + Send + Sync + ToBytes + FromBytes + Ord;

    type FiatShamirRng: AlgebraicSponge<Self::BaseField, 2, Parameters = Self::FSParameters>;
    type FSParameters;

    fn universal_setup(config: usize) -> Result<Self::UniversalSRS>;

    fn circuit_setup<C: ConstraintSynthesizer<Self::ScalarField>>(
        srs: &Self::UniversalSRS,
        circuit: &C,
    ) -> Result<(Self::ProvingKey, Self::VerifyingKey)>;

    fn prove_vk(
        universal_prover: &Self::UniversalProver,
        fs_parameters: &Self::FSParameters,
        verifying_key: &Self::VerifyingKey,
        proving_key: &Self::ProvingKey,
    ) -> Result<Self::Certificate>;

    fn prove<C: ConstraintSynthesizer<Self::ScalarField>, R: Rng + CryptoRng>(
        universal_prover: &Self::UniversalProver,
        fs_parameters: &Self::FSParameters,
        proving_key: &Self::ProvingKey,
        constraints: &C,
        rng: &mut R,
    ) -> Result<Self::Proof> {
        let mut keys_to_constraints = BTreeMap::new();
        keys_to_constraints.insert(proving_key, std::slice::from_ref(constraints));
        Self::prove_batch(universal_prover, fs_parameters, &keys_to_constraints, rng)
    }

    fn prove_batch<C: ConstraintSynthesizer<Self::ScalarField>, R: Rng + CryptoRng>(
        universal_prover: &Self::UniversalProver,
        fs_parameters: &Self::FSParameters,
        keys_to_constraints: &BTreeMap<&Self::ProvingKey, &[C]>,
        rng: &mut R,
    ) -> Result<Self::Proof>;

    fn verify_vk<C: ConstraintSynthesizer<Self::ScalarField>>(
        universal_verifier: &Self::UniversalVerifier,
        fs_parameters: &Self::FSParameters,
        circuit: &C,
        verifying_key: &Self::VerifyingKey,
        certificate: &Self::Certificate,
    ) -> Result<bool>;

    fn verify<B: Borrow<Self::VerifierInput>>(
        universal_verifier: &Self::UniversalVerifier,
        fs_parameters: &Self::FSParameters,
        verifying_key: &Self::VerifyingKey,
        input: B,
        proof: &Self::Proof,
    ) -> Result<bool> {
        let mut keys_to_inputs = BTreeMap::new();
        let inputs = [input];
        keys_to_inputs.insert(verifying_key, &inputs[..]);
        Self::verify_batch(universal_verifier, fs_parameters, &keys_to_inputs, proof)
    }

    fn verify_batch<B: Borrow<Self::VerifierInput>>(
        universal_verifier: &Self::UniversalVerifier,
        fs_parameters: &Self::FSParameters,
        keys_to_inputs: &BTreeMap<&Self::VerifyingKey, &[B]>,
        proof: &Self::Proof,
    ) -> Result<bool>;
}
