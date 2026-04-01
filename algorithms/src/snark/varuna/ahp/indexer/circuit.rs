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
use std::sync::Arc;

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
        SNARKMode,
        ahp::matrices::MatrixEvals,
        matrices::MatrixArithmetization,
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

    pub fft_precomputation: FFTPrecomputation<F>,
    pub ifft_precomputation: IFFTPrecomputation<F>,

    /// Precomputed FFT precomputation for the 2×variable_domain (used in
    /// prepare_third.rs to multiply m_at_alpha by assignment polynomials).
    /// Stored behind Arc so prove calls share the data with O(1) Arc::clone
    /// instead of an O(n) step_by copy. Not serialized; reconstructed from
    /// `index_info`.
    pub mul_fft_precomputation: Arc<FFTPrecomputation<F>>,
    pub mul_ifft_precomputation: Arc<IFFTPrecomputation<F>>,

    /// Precomputed column reindex table mapping variable-domain indices to
    /// their positions after reindexing by the input subdomain. Cached here
    /// (Arc) so prove calls share the data with O(1) cost. Not serialized;
    /// reconstructed from `index_info`. `None` when
    /// `variable_domain.size() == input_domain.size()` (no private variables).
    pub col_reindex: Option<Arc<Vec<usize>>>,

    /// Precomputed FFT/IFFT precomputations for each non-zero domain (K_a,
    /// K_b, K_c) and their 2× multiplication domains. Stored as Arc so that
    /// prove calls pay O(1) Arc::clone instead of an O(k) step_by extraction
    /// from `fft_precomputation` on every interpolation in the fourth round.
    /// Indexed [0=a, 1=b, 2=c]. Not serialized; reconstructed from
    /// `index_info`.
    pub non_zero_ifft_precomputation: [Arc<IFFTPrecomputation<F>>; 3],
    pub non_zero_mul_fft_precomputation: [Arc<FFTPrecomputation<F>>; 3],
    pub non_zero_mul_ifft_precomputation: [Arc<IFFTPrecomputation<F>>; 3],

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

        // Compute the 2×variable_domain FFT precomputation by extracting the
        // sub-precomputation from the full fft_precomputation. This avoids an
        // O(n) extraction on every prove call.
        let mul_domain_size = 2 * variable_domain_size;
        let mul_domain = EvaluationDomain::<F>::new(mul_domain_size).ok_or(SerializationError::InvalidData)?;
        let mul_fft_precomputation = Arc::new(
            fft_precomputation
                .precomputation_for_subdomain(&mul_domain)
                .ok_or(SerializationError::InvalidData)?
                .into_owned(),
        );
        let mul_ifft_precomputation = Arc::new(mul_fft_precomputation.to_ifft_precomputation());

        // Precompute the col_reindex table from domain sizes. Only valid when
        // variable_domain_size > input_domain_size (i.e., circuit has private vars).
        let variable_domain =
            EvaluationDomain::<F>::new(variable_domain_size).ok_or(SerializationError::InvalidData)?;
        let input_domain =
            EvaluationDomain::<F>::new(index_info.num_public_inputs).ok_or(SerializationError::InvalidData)?;
        let col_reindex = if variable_domain_size > input_domain.size() {
            Some(Arc::new(
                (0..variable_domain_size)
                    .map(|i| variable_domain.reindex_by_subdomain(&input_domain, i).unwrap())
                    .collect::<Vec<usize>>(),
            ))
        } else {
            None
        };

        // Precompute IFFT and 2× multiplication domain FFT/IFFT precomputations
        // for each non-zero domain (K_a, K_b, K_c). These are extracted from
        // fft_precomputation once here so that prove calls can use them directly
        // without O(k) step_by extraction on every interpolation in the fourth round.
        let non_zero_sizes = [non_zero_a_domain_size, non_zero_b_domain_size, non_zero_c_domain_size];
        let non_zero_ifft_precomputation: [Arc<IFFTPrecomputation<F>>; 3] = non_zero_sizes
            .iter()
            .map(|&size| {
                let domain = EvaluationDomain::<F>::new(size).ok_or(SerializationError::InvalidData)?;
                let nz_fft_pc = fft_precomputation
                    .precomputation_for_subdomain(&domain)
                    .ok_or(SerializationError::InvalidData)?
                    .into_owned();
                Ok(Arc::new(nz_fft_pc.to_ifft_precomputation()))
            })
            .collect::<Result<Vec<_>, SerializationError>>()?
            .try_into()
            .map_err(|_| SerializationError::InvalidData)?;
        let non_zero_mul_fft_precomputation: [Arc<FFTPrecomputation<F>>; 3] = non_zero_sizes
            .iter()
            .map(|&size| {
                let mul_nz_domain = EvaluationDomain::<F>::new(2 * size).ok_or(SerializationError::InvalidData)?;
                let nz_mul_fft_pc = fft_precomputation
                    .precomputation_for_subdomain(&mul_nz_domain)
                    .ok_or(SerializationError::InvalidData)?
                    .into_owned();
                Ok(Arc::new(nz_mul_fft_pc))
            })
            .collect::<Result<Vec<_>, SerializationError>>()?
            .try_into()
            .map_err(|_| SerializationError::InvalidData)?;
        let non_zero_mul_ifft_precomputation: [Arc<IFFTPrecomputation<F>>; 3] = non_zero_mul_fft_precomputation
            .iter()
            .map(|fft_pc| Arc::new(fft_pc.to_ifft_precomputation()))
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| SerializationError::InvalidData)?;

        let a = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let b = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let c = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let id = Self::hash(&index_info, &a, &b, &c)?;
        Ok(Circuit {
            index_info,
            a,
            b,
            c,
            a_arith: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            b_arith: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            c_arith: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            fft_precomputation,
            ifft_precomputation,
            mul_fft_precomputation,
            mul_ifft_precomputation,
            col_reindex,
            non_zero_ifft_precomputation,
            non_zero_mul_fft_precomputation,
            non_zero_mul_ifft_precomputation,
            _mode: PhantomData,
            id,
        })
    }
}
