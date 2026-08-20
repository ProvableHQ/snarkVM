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

//! ProveKit (Spartan + WHIR) transparent R1CS SNARK, vendored and instantiated
//! over BLS12-377.

pub mod adapter;
pub mod bls12_377;
pub mod common;
pub mod poseidon_circuit;
pub mod prover;
pub mod snark;
pub mod verifier;

pub use adapter::{SynthesizedCircuit, ark_fr_to_snarkvm, snarkvm_fr_to_ark, synthesize};
pub use bls12_377::{Bls12_377Field, register};
pub use common::{HashConfig, PublicInputs, R1CS, WhirR1CSProof, WhirR1CSScheme};
pub use poseidon_circuit::PoseidonPermutationCircuit;
pub use snark::{ProvekitSNARK, proof_size};

#[cfg(test)]
mod tests;
