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

use crate::r1cs::{ConstraintSynthesizer, ConstraintSystem, SynthesisError};
use snarkvm_fields::Field;

use rand::{CryptoRng, Rng};

#[doc(hidden)]
#[derive(Clone)]
/// This Circuit is only for testing and should not be used in production
pub struct TestCircuit<F: Field> {
    pub a: Option<F>,
    pub b: Option<F>,
    pub num_constraints: usize,
    pub num_variables: usize,
    pub mul_depth: usize,
}

impl<F: Field> core::fmt::Debug for TestCircuit<F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "TestCircuit {{ num_constraints: {}, num_variables: {}, mul_depth: {} }}",
            self.num_constraints, self.num_variables, self.mul_depth
        )
    }
}

impl<ConstraintF: Field> ConstraintSynthesizer<ConstraintF> for TestCircuit<ConstraintF> {
    fn generate_constraints<CS: ConstraintSystem<ConstraintF>>(&self, cs: &mut CS) -> Result<(), SynthesisError> {
        // Ensure the given `cs` is starting off clean.
        assert_eq!(1, cs.num_public_variables());
        assert_eq!(0, cs.num_private_variables());
        assert_eq!(0, cs.num_constraints());

        let a = cs.alloc(|| "a", || self.a.ok_or(SynthesisError::AssignmentMissing))?;
        let b = cs.alloc(|| "b", || self.b.ok_or(SynthesisError::AssignmentMissing))?;

        let mut mul_vars = Vec::with_capacity(self.mul_depth);
        for i in 0..self.mul_depth {
            let mul_var = cs.alloc_input(
                || format!("mul_var {i}"),
                || {
                    let mut a = self.a.ok_or(SynthesisError::AssignmentMissing)?;
                    let b = self.b.ok_or(SynthesisError::AssignmentMissing)?;

                    for _ in 0..(1 + i) {
                        a.mul_assign(&b);
                    }
                    Ok(a)
                },
            )?;
            mul_vars.push(mul_var);
        }

        // For this test we should allocate self.num_variables number of variables
        // 2 + mul_depth variables were already allocated above, 1 is allocated by default
        let dummy_variables = self.num_variables - 3 - self.mul_depth;
        for i in 0..dummy_variables {
            let _ = cs.alloc(|| format!("var {i}"), || self.a.ok_or(SynthesisError::AssignmentMissing))?;
        }

        let mul_constraints = self.mul_depth - 1;
        for i in 0..(self.num_constraints - mul_constraints) {
            cs.enforce(|| format!("constraint {i}"), |lc| lc + a, |lc| lc + b, |lc| lc + mul_vars[0]);
        }

        for i in 0..mul_constraints {
            cs.enforce(|| format!("constraint_mul {i}"), |lc| lc + mul_vars[i], |lc| lc + b, |lc| lc + mul_vars[i + 1]);
        }

        assert_eq!(cs.num_constraints(), self.num_constraints);
        assert_eq!(cs.num_public_variables() + cs.num_private_variables(), self.num_variables);

        Ok(())
    }
}

impl<F: Field> TestCircuit<F> {
    // Generate a test circuit with a random witness.
    pub fn gen_rand<R: Rng + CryptoRng>(
        mul_depth: usize,
        num_constraints: usize,
        num_variables: usize,
        rng: &mut R,
    ) -> (Self, Vec<F>) {
        let mut public_inputs: Vec<F> = Vec::with_capacity(1 + mul_depth);
        public_inputs.push(F::one());

        let a = F::rand(rng);
        let b = F::rand(rng);

        for j in 1..(mul_depth + 1) {
            let mut new_var = a;
            for _ in 0..j {
                new_var.mul_assign(&b);
            }
            public_inputs.push(new_var);
        }

        (TestCircuit { a: Some(a), b: Some(b), num_constraints, num_variables, mul_depth }, public_inputs)
    }

    // Generate a test circuit with a fixed witness.
    pub fn generate_circuit_with_fixed_witness(
        a: u128,
        b: u128,
        mul_depth: usize,
        num_constraints: usize,
        num_variables: usize,
    ) -> (Self, Vec<F>) {
        let mut public_inputs: Vec<F> = Vec::with_capacity(1 + mul_depth);
        public_inputs.push(F::one());
        let a = F::from(a);
        let b = F::from(b);
        for j in 1..(mul_depth + 1) {
            let mut new_var = a;
            for _ in 0..j {
                new_var.mul_assign(&b);
            }
            public_inputs.push(new_var);
        }

        (TestCircuit { a: Some(a), b: Some(b), num_constraints, num_variables, mul_depth }, public_inputs)
    }
}
