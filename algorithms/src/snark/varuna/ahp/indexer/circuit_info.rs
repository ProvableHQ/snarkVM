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

use crate::{
    fft::EvaluationDomain,
    polycommit::kzg10::DegreeInfo,
    snark::varuna::{SNARKMode, ahp::AHPForR1CS},
};
use anyhow::Result;
use snarkvm_fields::PrimeField;
use snarkvm_utilities::{ToBytes, serialize::*};
use std::io;

/// Information about the circuit, including the field of definition, the number
/// of variables, the number of constraints, and the maximum number of non-zero
/// entries in any of the constraint matrices.
#[derive(Copy, Clone, Debug, PartialEq, Eq, CanonicalSerialize, CanonicalDeserialize)]
pub struct CircuitInfo {
    /// The number of public inputs after padding.
    // This includes the constant 1
    pub num_public_inputs: usize,
    /// The number of public and private variables in the constraint system.
    /// Note: This does *NOT* include the number of constants in the constraint
    /// system.
    // This includes the padded instance (constant 1 and public variables) as
    // well as the witness, but is not itself padded.
    pub num_public_and_private_variables: usize,
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
    /// The max degrees and bounds required to represent polynomials of this
    /// circuit.
    pub fn degree_info<F: PrimeField, SM: SNARKMode>(&self) -> Result<DegreeInfo> {
        let max_degree = self.max_degree::<F, SM>()?;
        let max_fft_size = self.max_fft_size::<F>();
        let degree_bounds = Some(self.degree_bounds::<F>().into_iter().collect());
        let hiding_bound = AHPForR1CS::<F, SM>::zk_bound().unwrap_or(0);
        Ok(DegreeInfo::new(max_degree, max_fft_size, degree_bounds, hiding_bound, None))
    }

    /// The maximum degree of polynomial required to represent this index in the
    /// AHP.
    pub fn max_degree<F: PrimeField, SM: SNARKMode>(&self) -> Result<usize> {
        let max_non_zero = self.num_non_zero_a.max(self.num_non_zero_b).max(self.num_non_zero_c);
        AHPForR1CS::<F, SM>::max_degree(self.num_constraints, self.num_public_and_private_variables, max_non_zero)
    }

    /// The maximum size of polynomials we need to do (i)fft for
    pub fn max_fft_size<F: PrimeField>(&self) -> usize {
        let size = [
            2 * self.num_constraints,                  // zerocheck poly degree
            2 * self.num_public_and_private_variables, // lineval sumcheck poly degree
            2 * self.num_non_zero_a,                   // matrix sumcheck poly degree
            2 * self.num_non_zero_b,                   // matrix sumcheck poly degree
            2 * self.num_non_zero_c,                   // matrix sumcheck poly degree
        ]
        .into_iter()
        .max()
        .unwrap();
        crate::fft::EvaluationDomain::<F>::compute_size_of_domain(size).unwrap()
    }

    /// Get all the strict degree bounds enforced in the AHP.
    pub fn degree_bounds<F: PrimeField>(&self) -> [usize; 4] {
        [
            // The degree bound for g_1
            EvaluationDomain::<F>::compute_size_of_domain(self.num_public_and_private_variables).unwrap() - 2,
            // The degree bound for g_A
            EvaluationDomain::<F>::compute_size_of_domain(self.num_non_zero_a).unwrap() - 2,
            // The degree bound for g_B
            EvaluationDomain::<F>::compute_size_of_domain(self.num_non_zero_b).unwrap() - 2,
            // The degree bound for g_C
            EvaluationDomain::<F>::compute_size_of_domain(self.num_non_zero_c).unwrap() - 2,
        ]
    }
}

impl ToBytes for CircuitInfo {
    fn write_le<W: Write>(&self, mut w: W) -> Result<(), io::Error> {
        (self.num_public_inputs as u64).write_le(&mut w)?;
        (self.num_public_and_private_variables as u64).write_le(&mut w)?;
        (self.num_constraints as u64).write_le(&mut w)?;
        (self.num_non_zero_a as u64).write_le(&mut w)?;
        (self.num_non_zero_b as u64).write_le(&mut w)?;
        (self.num_non_zero_c as u64).write_le(&mut w)
    }
}
