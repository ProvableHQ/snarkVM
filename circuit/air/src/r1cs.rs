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

use crate::{Air, AirBuilder, BaseAir, Trace};
use snarkvm_circuit_environment::{Assignment, LinearCombination, Variable};
use snarkvm_fields::{One, PrimeField};

/// A sparse linear combination over AIR main-trace columns.
#[derive(Clone, Debug)]
struct SparseLc<F: PrimeField> {
    constant: F,
    terms: Vec<(usize, F)>,
}

/// One R1CS multiplication constraint `A * B = C`.
#[derive(Clone, Debug)]
struct SparseConstraint<F: PrimeField> {
    a: SparseLc<F>,
    b: SparseLc<F>,
    c: SparseLc<F>,
}

/// Complete AIR for an R1CS `Assignment`.
///
/// Columns are the public variables followed by the private variables (the
/// witness). The trace has a single row. Each R1CS constraint becomes a
/// degree-2 polynomial `A(w) * B(w) - C(w)` in those columns. This is a
/// circuit-specific AIR: `eval` lists the instance's constraints rather than
/// a uniform gate.
#[derive(Clone, Debug)]
pub struct R1csAir<F: PrimeField> {
    num_public: usize,
    num_private: usize,
    constraints: Vec<SparseConstraint<F>>,
}

impl<F: PrimeField> R1csAir<F> {
    /// Lowers `assignment` to a witness-column AIR and a matching one-row trace.
    pub fn from_assignment(assignment: &Assignment<F>) -> (Self, Trace<F>) {
        let air = Self::from_assignment_structure(assignment);
        let trace = Self::trace(assignment);
        (air, trace)
    }

    /// Lowers the constraint structure of `assignment` without building a trace.
    pub fn from_assignment_structure(assignment: &Assignment<F>) -> Self {
        let num_public = assignment.num_public() as usize;
        let num_private = assignment.num_private() as usize;
        let constraints = assignment
            .constraints()
            .iter()
            .map(|constraint| {
                let (a, b, c) = constraint.to_terms();
                SparseConstraint {
                    a: sparse_lc(a, num_public),
                    b: sparse_lc(b, num_public),
                    c: sparse_lc(c, num_public),
                }
            })
            .collect();
        Self { num_public, num_private, constraints }
    }

    /// Builds a one-row witness trace from `assignment`.
    pub fn trace(assignment: &Assignment<F>) -> Trace<F> {
        let num_public = assignment.num_public() as usize;
        let num_private = assignment.num_private() as usize;
        let width = num_public + num_private;
        let mut values = Vec::with_capacity(width);
        values.extend(assignment.public_inputs().iter().map(Variable::value));
        values.extend(assignment.private_inputs().iter().map(Variable::value));
        // The R1CS environment always allocates the public `one` variable, so width is positive.
        Trace::new(width, 1, values).expect("R1CS assignment always contains at least the public one variable")
    }

    /// Returns the number of public (including `one`) columns.
    pub const fn num_public(&self) -> usize {
        self.num_public
    }

    /// Returns the number of private columns.
    pub const fn num_private(&self) -> usize {
        self.num_private
    }

    /// Returns the number of R1CS constraints encoded in this AIR.
    pub fn num_constraints(&self) -> usize {
        self.constraints.len()
    }
}

impl<F: PrimeField> BaseAir<F> for R1csAir<F> {
    fn width(&self) -> usize {
        self.num_public + self.num_private
    }
}

impl<AB: AirBuilder> Air<AB> for R1csAir<AB::F> {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let local = main.local();

        if self.num_public > 0 {
            builder.when_first_row().assert_eq(local[0], AB::Expr::one());
        }

        for constraint in &self.constraints {
            let a = eval_lc::<AB>(&constraint.a, local);
            let b = eval_lc::<AB>(&constraint.b, local);
            let c = eval_lc::<AB>(&constraint.c, local);
            builder.assert_zero(a * b - c);
        }
    }
}

/// Uniform AIR with one row per R1CS constraint and columns `(A, B, C)`.
///
/// The local constraint is `A * B - C = 0`. Linear-combination correctness is
/// established by filling the trace from the assignment's LC evaluations.
#[derive(Clone, Debug, Default)]
pub struct R1csGateAir;

impl R1csGateAir {
    /// Number of columns: `(A, B, C)`.
    pub const WIDTH: usize = 3;

    /// Lowers `assignment` to a one-row-per-constraint gate AIR and its trace.
    pub fn from_assignment<F: PrimeField>(assignment: &Assignment<F>) -> (Self, Trace<F>) {
        (Self, Self::trace(assignment))
    }

    /// Builds a 3-column trace whose `i`-th row is the LC values of constraint `i`.
    pub fn trace<F: PrimeField>(assignment: &Assignment<F>) -> Trace<F> {
        let height = assignment.num_constraints().max(1) as usize;
        let mut values = Vec::with_capacity(height.saturating_mul(Self::WIDTH));
        if assignment.num_constraints() == 0 {
            values.extend([F::default(), F::default(), F::default()]);
        } else {
            for constraint in assignment.constraints().iter() {
                let (a, b, c) = constraint.to_terms();
                values.push(a.value());
                values.push(b.value());
                values.push(c.value());
            }
        }
        Trace::new(Self::WIDTH, height, values).expect("gate trace width is 3 and height is at least 1")
    }

    /// Returns the number of columns.
    pub const fn width(&self) -> usize {
        Self::WIDTH
    }
}

impl<F: PrimeField> BaseAir<F> for R1csGateAir {
    fn width(&self) -> usize {
        Self::WIDTH
    }
}

impl<AB: AirBuilder> Air<AB> for R1csGateAir {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let local = main.local();
        let a: AB::Expr = local[0].into();
        let b: AB::Expr = local[1].into();
        let c: AB::Expr = local[2].into();
        builder.assert_zero(a * b - c);
    }
}

fn sparse_lc<F: PrimeField>(lc: &LinearCombination<F>, num_public: usize) -> SparseLc<F> {
    let mut constant = lc.to_constant();
    let mut terms = Vec::with_capacity(lc.to_terms().len());
    for (variable, coeff) in lc.to_terms() {
        if coeff.is_zero() {
            continue;
        }
        match variable {
            Variable::Constant(value) => {
                constant += *coeff * **value;
            }
            Variable::Public(index_value) => {
                let (index, _) = index_value.as_ref();
                // Variable indices are allocated sequentially and fit in `usize`.
                let column = usize::try_from(*index).expect("public variable index fits in usize");
                terms.push((column, *coeff));
            }
            Variable::Private(index_value) => {
                let (index, _) = index_value.as_ref();
                // Variable indices are allocated sequentially and fit in `usize`.
                let index = usize::try_from(*index).expect("private variable index fits in usize");
                terms.push((num_public + index, *coeff));
            }
        }
    }
    SparseLc { constant, terms }
}

fn eval_lc<AB: AirBuilder>(lc: &SparseLc<AB::F>, local: &[AB::Var]) -> AB::Expr {
    let mut acc = AB::Expr::from(lc.constant);
    for (column, coeff) in &lc.terms {
        acc = acc + AB::Expr::from(*coeff) * local[*column].into();
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SymbolicAirBuilder, debug_constraints};
    use snarkvm_circuit_environment::{Circuit, Environment, Inject, Mode};
    use snarkvm_circuit_types_field::Field;
    use snarkvm_console_types_field::Field as ConsoleField;
    use snarkvm_fields::One;

    use serial_test::serial;

    type ConsoleF = ConsoleField<<Circuit as Environment>::Network>;

    /// Synthesizes a single private multiplication `3 * 5` as the test circuit.
    fn mul_circuit() -> Assignment<<Circuit as Environment>::BaseField> {
        Circuit::reset();
        let a = Field::<Circuit>::new(Mode::Private, ConsoleF::from_u64(3));
        let b = Field::<Circuit>::new(Mode::Private, ConsoleF::from_u64(5));
        let _product = a * b;
        assert_eq!(1, Circuit::num_constraints());
        Circuit::eject_assignment_and_reset()
    }

    #[test]
    #[serial]
    fn test_r1cs_air_for_a_single_multiplication_circuit() {
        let assignment = mul_circuit();
        assert_eq!(1, assignment.num_constraints());
        assert_eq!(1, assignment.num_public());
        assert_eq!(3, assignment.num_private());

        let (air, mut trace) = R1csAir::from_assignment(&assignment);
        assert_eq!(assignment.num_constraints() as usize, air.num_constraints());
        debug_constraints(&air, &trace).unwrap();
        assert_eq!(2, SymbolicAirBuilder::constraints_of(&air).len());

        let (gate_air, gate_trace) = R1csGateAir::from_assignment(&assignment);
        assert_eq!(3, gate_air.width());
        assert_eq!(1, gate_trace.height());
        debug_constraints(&gate_air, &gate_trace).unwrap();

        let last = trace.width() - 1;
        *trace.get_mut(0, last) += <Circuit as Environment>::BaseField::one();
        assert!(debug_constraints(&air, &trace).is_err());
    }
}
