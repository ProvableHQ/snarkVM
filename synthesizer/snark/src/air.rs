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

//! Experimental AIR lowering of R1CS assignments.
//!
//! This module sits beside Varuna (and optional ProveKit). It does **not**
//! replace those proving systems or change `ProvingKey` / `Proof` APIs.

pub use snarkvm_circuit::air::{Air, AirBuilder, BaseAir, PoseidonAir, R1csAir, R1csGateAir, Trace, debug_constraints};

use snarkvm_circuit::environment::{Assignment, prelude::PrimeField};

/// Compiles an R1CS assignment into a complete witness-column AIR and its trace.
pub fn r1cs_air_from_assignment<F: PrimeField>(assignment: &Assignment<F>) -> (R1csAir<F>, Trace<F>) {
    R1csAir::from_assignment(assignment)
}

/// Compiles an R1CS assignment into a uniform one-row-per-constraint gate AIR.
pub fn r1cs_gate_air_from_assignment<F: PrimeField>(assignment: &Assignment<F>) -> (R1csGateAir, Trace<F>) {
    R1csGateAir::from_assignment(assignment)
}
