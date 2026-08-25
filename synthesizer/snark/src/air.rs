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

pub use snarkvm_circuit::air::{
    Air,
    AirBuilder,
    BaseAir,
    OpcodeColumn,
    OpcodeR1csAir,
    PoseidonAir,
    R1csAir,
    R1csGateAir,
    Trace,
    TransitionLink,
    debug_constraints,
};

use snarkvm_circuit::environment::{Assignment, prelude::PrimeField};

/// Compiles an R1CS assignment into a complete witness-column AIR and its trace.
pub fn r1cs_air_from_assignment<F: PrimeField>(assignment: &Assignment<F>) -> (R1csAir<F>, Trace<F>) {
    R1csAir::from_assignment(assignment)
}

/// Compiles an R1CS assignment into a uniform one-row-per-constraint gate AIR.
pub fn r1cs_gate_air_from_assignment<F: PrimeField>(assignment: &Assignment<F>) -> (R1csGateAir, Trace<F>) {
    R1csGateAir::from_assignment(assignment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit::{
        Circuit,
        Environment,
        Field,
        Inject,
        Mode,
        Poseidon2,
        air::SymbolicAirBuilder,
        traits::Hash,
    };

    type BaseField = <Circuit as Environment>::BaseField;

    #[test]
    fn test_poseidon_hash_air_size_report() -> Result<(), Box<dyn std::error::Error>> {
        Circuit::reset();
        let native = <<Poseidon2<Circuit> as Inject>::Primitive>::setup("PoseidonCircuit0")?;
        let poseidon = Poseidon2::<Circuit>::constant(native);
        let inputs = [
            Field::<Circuit>::new(Mode::Private, Default::default()),
            Field::<Circuit>::new(Mode::Private, Default::default()),
        ];
        let _output = poseidon.hash(&inputs);
        let assignment = Circuit::eject_assignment_and_reset();

        let (opcode_air, opcode_trace) = OpcodeR1csAir::from_assignments(&[assignment], &[])?;
        let opcode_constraints = SymbolicAirBuilder::constraints_of(&opcode_air);
        let opcode_degree = opcode_constraints.iter().map(|constraint| constraint.degree()).max().unwrap_or(0);

        let native_air = PoseidonAir::<BaseField, 2>::setup()?;
        let native_trace = native_air.generate_trace(&[BaseField::default(); 3])?;
        let native_constraints = SymbolicAirBuilder::constraints_of(&native_air);
        let native_degree = native_constraints.iter().map(|constraint| constraint.degree()).max().unwrap_or(0);

        println!(
            "opcode_r1cs: width={}, height={}, cells={}, r1cs_constraints={}, air_constraints={}, max_degree={}",
            opcode_trace.width(),
            opcode_trace.height(),
            opcode_trace.width() * opcode_trace.height(),
            opcode_air.num_constraints_per_row(),
            opcode_constraints.len(),
            opcode_degree,
        );
        println!(
            "native_round: main_width={}, preprocessed_width={}, height={}, main_cells={}, preprocessed_cells={}, \
             air_constraints={}, max_degree={}",
            native_trace.width(),
            native_air.preprocessed_width(),
            native_trace.height(),
            native_trace.width() * native_trace.height(),
            native_air.preprocessed_width() * native_trace.height(),
            native_constraints.len(),
            native_degree,
        );

        assert_eq!(273, opcode_trace.width());
        assert_eq!(1, opcode_trace.height());
        assert_eq!(270, opcode_air.num_constraints_per_row());
        assert_eq!(271, opcode_constraints.len());
        assert_eq!(2, opcode_degree);

        assert_eq!(3, native_trace.width());
        assert_eq!(4, native_air.preprocessed_width());
        assert_eq!(40, native_trace.height());
        assert_eq!(3, native_constraints.len());
        assert_eq!(19, native_degree);
        Ok(())
    }
}
