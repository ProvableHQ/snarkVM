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

#![allow(non_snake_case)]

mod circuit;
pub(crate) use circuit::*;

mod circuit_info;
pub(crate) use circuit_info::*;

mod constraint_system;
pub(crate) use constraint_system::*;

mod indexer;

use snarkvm_fields::PrimeField;
use snarkvm_utilities::serialize::*;

/// Represents a matrix.
pub(crate) type Matrix<F> = Vec<Vec<(F, usize)>>;

pub(crate) fn num_non_zero<F>(joint_matrix: &Matrix<F>) -> usize {
    joint_matrix.iter().map(|row| row.len()).sum()
}

/// Parameters for sparse matrix-vector multiplication, categorized by column
/// density. Used for efficient GPU computation with different kernel strategies
/// per density bucket.
#[derive(Debug, Clone, Default)]
pub struct MatrixParameters<F> {
    /// Column indices in the output
    pub col_indices: Vec<usize>,
    /// Row indices for looking up input values
    pub row_indices: Vec<usize>,
    /// Non-zero values
    pub row_values: Vec<F>,
    /// Number of non-zeros per column
    pub col_sizes: Vec<usize>,
    /// Starting location for each column's data in row_indices/row_values
    pub col_locations: Vec<usize>,
}

/// 4 buckets of MatrixParameters, categorized by column density:
/// - [0]: < 8 non-zeros (Thread kernel)
/// - [1]: 8-127 non-zeros (Warp kernel)
/// - [2]: 128-1023 non-zeros (Block kernel)
/// - [3]: >= 1024 non-zeros (Cooperative kernel)
pub(crate) type MatrixParametersAll<F> = Vec<MatrixParameters<F>>;

impl<F: PrimeField> CanonicalSerialize for MatrixParameters<F> {
    fn serialize_with_mode<W: std::io::Write>(
        &self,
        mut writer: W,
        compress: Compress,
    ) -> Result<(), SerializationError> {
        self.col_indices.serialize_with_mode(&mut writer, compress)?;
        self.row_indices.serialize_with_mode(&mut writer, compress)?;
        self.row_values.serialize_with_mode(&mut writer, compress)?;
        self.col_sizes.serialize_with_mode(&mut writer, compress)?;
        self.col_locations.serialize_with_mode(&mut writer, compress)?;
        Ok(())
    }

    fn serialized_size(&self, mode: Compress) -> usize {
        self.col_indices
            .serialized_size(mode)
            .saturating_add(self.row_indices.serialized_size(mode))
            .saturating_add(self.row_values.serialized_size(mode))
            .saturating_add(self.col_sizes.serialized_size(mode))
            .saturating_add(self.col_locations.serialized_size(mode))
    }
}

impl<F: PrimeField> snarkvm_utilities::Valid for MatrixParameters<F> {
    fn check(&self) -> Result<(), SerializationError> {
        Ok(())
    }
}

impl<F: PrimeField> CanonicalDeserialize for MatrixParameters<F> {
    fn deserialize_with_mode<R: std::io::Read>(
        mut reader: R,
        compress: Compress,
        validate: Validate,
    ) -> Result<Self, SerializationError> {
        Ok(MatrixParameters {
            col_indices: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            row_indices: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            row_values: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            col_sizes: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            col_locations: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
        })
    }
}

/// Generate matrix parameters from a transposed matrix, categorizing columns by
/// density. This preprocessing enables efficient GPU sparse matrix-vector
/// multiplication.
pub(crate) fn generate_matrix_parameters<F: Clone>(matrix: &Matrix<F>) -> MatrixParametersAll<F> {
    let mut matrix_parameters = vec![
        MatrixParameters {
            col_indices: Vec::new(),
            row_indices: Vec::new(),
            row_values: Vec::new(),
            col_sizes: Vec::new(),
            col_locations: Vec::new(),
        };
        4
    ];

    let mut counter_case0 = 0; // Thread (< 8)
    let mut counter_case1 = 0; // Warp (8-127)
    let mut counter_case2 = 0; // Block (128-1023)
    let mut counter_case3 = 0; // Coop (>= 1024)

    for (index, col) in matrix.iter().enumerate() {
        let case_idx = if col.len() < 8 {
            0
        } else if col.len() < 128 {
            1
        } else if col.len() < 1024 {
            2
        } else {
            3
        };

        let counter = match case_idx {
            0 => &mut counter_case0,
            1 => &mut counter_case1,
            2 => &mut counter_case2,
            _ => &mut counter_case3,
        };

        matrix_parameters[case_idx].col_indices.push(index);
        matrix_parameters[case_idx].col_locations.push(*counter);
        matrix_parameters[case_idx].col_sizes.push(col.len());

        for (value, row_index) in col {
            matrix_parameters[case_idx].row_indices.push(*row_index);
            matrix_parameters[case_idx].row_values.push(value.clone());
        }

        *counter += col.len();
    }

    matrix_parameters
}
