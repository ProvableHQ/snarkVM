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

use core::marker::PhantomData;

use crate::{
    fft::{
        EvaluationDomain,
        domain::{FFTPrecomputation, IFFTPrecomputation},
    },
    polycommit::sonic_pc::LabeledPolynomial,
    snark::varuna::{
        AHPForR1CS,
        CircuitInfo,
        Matrix,
        MatrixParametersAll,
        SNARKMode,
        ahp::matrices::MatrixEvals,
        generate_matrix_parameters,
        matrices::{MatrixArithmetization, transpose},
    },
};
use anyhow::{Result, anyhow};
use blake2::Digest;
use hex::FromHex;
use snarkvm_fields::PrimeField;
use snarkvm_utilities::{SerializationError, serialize::*};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, CanonicalSerialize, CanonicalDeserialize)]
pub struct CircuitId(pub [u8; 32]);

impl std::fmt::Display for CircuitId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl CircuitId {
    pub fn from_witness_label(witness_label: &str) -> Self {
        CircuitId(
            <[u8; 32]>::from_hex(witness_label.split('_').collect::<Vec<&str>>()[1])
                .expect("Decoding circuit_id failed"),
        )
    }
}

/// The indexed version of the constraint system.
/// This struct contains three kinds of objects:
/// 1) `index_info` is information about the index, such as the size of the
///    public input
/// 2) `{a,b,c}` are the matrices defining the R1CS instance
/// 3) `{a,b,c}_arith` are structs containing information about the arithmetized
///    matrices
/// 4) `{a,b,c}_trans_parameters` are precomputed parameters for efficient GPU
///    sparse matrix-vector multiplication
#[derive(Debug)]
pub struct Circuit<F: PrimeField, SM: SNARKMode> {
    /// Information about the indexed circuit.
    pub index_info: CircuitInfo,

    /// The A matrix for the R1CS instance
    pub a: Matrix<F>,
    /// The B matrix for the R1CS instance
    pub b: Matrix<F>,
    /// The C matrix for the R1CS instance
    pub c: Matrix<F>,

    /// Joint arithmetization of the A, B, and C matrices.
    pub a_arith: MatrixEvals<F>,
    pub b_arith: MatrixEvals<F>,
    pub c_arith: MatrixEvals<F>,

    /// Precomputed transpose parameters for GPU sparse matrix-vector
    /// multiplication. Columns are categorized into 4 buckets by density
    /// for different CUDA kernels.
    pub a_trans_parameters: MatrixParametersAll<F>,
    pub b_trans_parameters: MatrixParametersAll<F>,
    pub c_trans_parameters: MatrixParametersAll<F>,

    pub fft_precomputation: FFTPrecomputation<F>,
    pub ifft_precomputation: IFFTPrecomputation<F>,
    pub(crate) _mode: PhantomData<SM>,
    pub(crate) id: CircuitId,
}

impl<F: PrimeField, SM: SNARKMode> Eq for Circuit<F, SM> {}
impl<F: PrimeField, SM: SNARKMode> PartialEq for Circuit<F, SM> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<F: PrimeField, SM: SNARKMode> Ord for Circuit<F, SM> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl<F: PrimeField, SM: SNARKMode> PartialOrd for Circuit<F, SM> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<F: PrimeField, SM: SNARKMode> Circuit<F, SM> {
    pub fn hash(
        index_info: &CircuitInfo,
        a: &Matrix<F>,
        b: &Matrix<F>,
        c: &Matrix<F>,
    ) -> Result<CircuitId, SerializationError> {
        let mut blake2 = blake2::Blake2s256::new();
        index_info.serialize_uncompressed(&mut blake2)?;
        a.serialize_uncompressed(&mut blake2)?;
        b.serialize_uncompressed(&mut blake2)?;
        c.serialize_uncompressed(&mut blake2)?;
        Ok(CircuitId(blake2.finalize().into()))
    }

    /// The maximum degree required to represent polynomials of this index.
    pub fn max_degree(&self) -> Result<usize> {
        self.index_info.max_degree::<F, SM>()
    }

    /// The size of the constraint (i. e. row) domain in this R1CS instance.
    pub fn constraint_domain_size(&self) -> Result<usize> {
        Ok(crate::fft::EvaluationDomain::<F>::new(self.index_info.num_constraints)
            .ok_or(anyhow!("Cannot create EvaluationDomain"))?
            .size())
    }

    /// The size of the variable (i. e. column) domain in this R1CS instance.
    pub fn variable_domain_size(&self) -> Result<usize> {
        Ok(crate::fft::EvaluationDomain::<F>::new(self.index_info.num_public_and_private_variables)
            .ok_or(anyhow!("Cannot create EvaluationDomain"))?
            .size())
    }

    /// Compute the row, col, rowcol and rowcolval polynomials of the three
    /// matrices in this R1CS instance.
    pub fn interpolate_matrix_evals(&self) -> Result<impl Iterator<Item = LabeledPolynomial<F>>> {
        let mut iters = Vec::with_capacity(3);
        for (label, evals) in [("a", &self.a_arith), ("b", &self.b_arith), ("c", &self.c_arith)] {
            iters.push(MatrixArithmetization::new::<SM>(&self.id, label, evals)?.into_iter());
        }
        Ok(iters.into_iter().flatten())
    }

    /// After indexing, we drop these evaluations to save space in the
    /// ProvingKey.
    pub fn prune_row_col_evals(&mut self) {
        self.a_arith.row_col = None;
        self.b_arith.row_col = None;
        self.c_arith.row_col = None;
    }
}

impl<F: PrimeField, SM: SNARKMode> CanonicalSerialize for Circuit<F, SM> {
    fn serialize_with_mode<W: Write>(&self, mut writer: W, compress: Compress) -> Result<(), SerializationError> {
        // Note: *_trans_parameters are NOT serialized - they are computed on-the-fly
        // during deserialization from the a, b, c matrices to maintain backward
        // compatibility
        self.index_info.serialize_with_mode(&mut writer, compress)?;
        self.a.serialize_with_mode(&mut writer, compress)?;
        self.b.serialize_with_mode(&mut writer, compress)?;
        self.c.serialize_with_mode(&mut writer, compress)?;
        self.a_arith.serialize_with_mode(&mut writer, compress)?;
        self.b_arith.serialize_with_mode(&mut writer, compress)?;
        self.c_arith.serialize_with_mode(&mut writer, compress)?;
        Ok(())
    }

    fn serialized_size(&self, mode: Compress) -> usize {
        self.index_info
            .serialized_size(mode)
            .saturating_add(self.a.serialized_size(mode))
            .saturating_add(self.b.serialized_size(mode))
            .saturating_add(self.c.serialized_size(mode))
            .saturating_add(self.a_arith.serialized_size(mode))
            .saturating_add(self.b_arith.serialized_size(mode))
            .saturating_add(self.c_arith.serialized_size(mode))
    }
}

impl<F: PrimeField, SM: SNARKMode> snarkvm_utilities::Valid for Circuit<F, SM> {
    fn check(&self) -> Result<(), SerializationError> {
        Ok(())
    }

    fn batch_check<'a>(_batch: impl Iterator<Item = &'a Self> + Send) -> Result<(), SerializationError> {
        Ok(())
    }
}

impl<F: PrimeField, SM: SNARKMode> CanonicalDeserialize for Circuit<F, SM> {
    fn deserialize_with_mode<R: Read>(
        mut reader: R,
        compress: Compress,
        validate: Validate,
    ) -> Result<Self, SerializationError> {
        let index_info: CircuitInfo = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let constraint_domain_size = EvaluationDomain::<F>::compute_size_of_domain(index_info.num_constraints)
            .ok_or(SerializationError::InvalidData)?;
        let variable_domain_size =
            EvaluationDomain::<F>::compute_size_of_domain(index_info.num_public_and_private_variables)
                .ok_or(SerializationError::InvalidData)?;
        let non_zero_a_domain_size = EvaluationDomain::<F>::compute_size_of_domain(index_info.num_non_zero_a)
            .ok_or(SerializationError::InvalidData)?;
        let non_zero_b_domain_size = EvaluationDomain::<F>::compute_size_of_domain(index_info.num_non_zero_b)
            .ok_or(SerializationError::InvalidData)?;
        let non_zero_c_domain_size = EvaluationDomain::<F>::compute_size_of_domain(index_info.num_non_zero_c)
            .ok_or(SerializationError::InvalidData)?;

        let (fft_precomputation, ifft_precomputation) = AHPForR1CS::<F, SM>::fft_precomputation(
            variable_domain_size,
            constraint_domain_size,
            non_zero_a_domain_size,
            non_zero_b_domain_size,
            non_zero_c_domain_size,
        )
        .ok_or(SerializationError::InvalidData)?;
        let a: Matrix<F> = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let b: Matrix<F> = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let c: Matrix<F> = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let id = Self::hash(&index_info, &a, &b, &c)?;

        // Compute trans_parameters on-the-fly (not serialized for backward
        // compatibility)
        let variable_domain = EvaluationDomain::<F>::new(index_info.num_public_and_private_variables)
            .ok_or(SerializationError::InvalidData)?;
        let input_domain =
            EvaluationDomain::<F>::new(index_info.num_public_inputs).ok_or(SerializationError::InvalidData)?;

        let a_trans = transpose(&a, &variable_domain, &input_domain).map_err(|_| SerializationError::InvalidData)?;
        let b_trans = transpose(&b, &variable_domain, &input_domain).map_err(|_| SerializationError::InvalidData)?;
        let c_trans = transpose(&c, &variable_domain, &input_domain).map_err(|_| SerializationError::InvalidData)?;

        let a_trans_parameters = generate_matrix_parameters(&a_trans);
        let b_trans_parameters = generate_matrix_parameters(&b_trans);
        let c_trans_parameters = generate_matrix_parameters(&c_trans);

        Ok(Circuit {
            index_info,
            a,
            b,
            c,
            a_arith: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            b_arith: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            c_arith: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            a_trans_parameters,
            b_trans_parameters,
            c_trans_parameters,
            fft_precomputation,
            ifft_precomputation,
            _mode: PhantomData,
            id,
        })
    }
}
