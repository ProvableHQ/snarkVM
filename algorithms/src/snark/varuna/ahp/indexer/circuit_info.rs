// Copyright (C) 2019-2023 Aleo Systems Inc.
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

use crate::{
    fft::EvaluationDomain,
    snark::varuna::{ahp::AHPForR1CS, SNARKMode},
};
use snarkvm_fields::PrimeField;
use snarkvm_utilities::{serialize::*, ToBytes};

/// Information about the circuit, including the field of definition, the number of
/// variables, the number of constraints, and the maximum number of non-zero
/// entries in any of the constraint matrices.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, CanonicalSerialize, CanonicalDeserialize)]
pub struct CircuitInfo {
    /// The number of public inputs after padding.
    pub num_public_inputs: usize,
    /// The total number of variables in the constraint system.
    pub num_variables: usize,
    /// The number of constraints.
    pub num_constraints: usize,
    /// The number of non-zero entries in the A matrix.
    pub num_non_zero_a: usize,
    /// The number of non-zero entries in the B matrix.
    pub num_non_zero_b: usize,
    /// The number of non-zero entries in the C matrix.
    pub num_non_zero_c: usize,
}

impl CircuitInfo {
    /// The maximum degree of polynomial required to represent this index in the AHP.
    pub fn max_degree<F: PrimeField, MM: SNARKMode>(&self) -> usize {
        let max_non_zero = self.num_non_zero_a.max(self.num_non_zero_b).max(self.num_non_zero_c);
        AHPForR1CS::<F, MM>::max_degree(self.num_constraints, self.num_variables, max_non_zero).unwrap()
    }

    /// Get all the strict degree bounds enforced in the AHP.
    pub fn get_degree_bounds<F: PrimeField>(&self) -> [usize; 4] {
        let num_variables = self.num_variables;
        let num_non_zero_a = self.num_non_zero_a;
        let num_non_zero_b = self.num_non_zero_b;
        let num_non_zero_c = self.num_non_zero_c;
        [
            // The degree bound for g_1
            EvaluationDomain::<F>::compute_size_of_domain(num_variables).unwrap() - 2,
            // The degree bound for g_A
            EvaluationDomain::<F>::compute_size_of_domain(num_non_zero_a).unwrap() - 2,
            // The degree bound for g_B
            EvaluationDomain::<F>::compute_size_of_domain(num_non_zero_b).unwrap() - 2,
            // The degree bound for g_C
            EvaluationDomain::<F>::compute_size_of_domain(num_non_zero_c).unwrap() - 2,
        ]
    }
}

impl ToBytes for CircuitInfo {
    fn write_le<W: Write>(&self, mut w: W) -> Result<(), io::Error> {
        (self.num_public_inputs as u64).write_le(&mut w)?;
        (self.num_variables as u64).write_le(&mut w)?;
        (self.num_constraints as u64).write_le(&mut w)?;
        (self.num_non_zero_a as u64).write_le(&mut w)?;
        (self.num_non_zero_b as u64).write_le(&mut w)?;
        (self.num_non_zero_c as u64).write_le(&mut w)
    }
}