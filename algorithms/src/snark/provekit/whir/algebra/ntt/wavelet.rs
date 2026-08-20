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

// Originally derived from WHIR (https://github.com/WizardOfMenlo/whir),
// licensed under Apache-2.0 OR MIT.

use snarkvm_fields::Field;
#[cfg(not(feature = "serial"))]
use {crate::snark::provekit::whir::utils::workload_size, rayon::prelude::*};

use super::transpose;

/// Fast Wavelet Transform.
///
/// The input slice must have a length that is a power of two.
/// Recursively applies the kernel
///   [1 0]
///   [1 1]
pub fn wavelet_transform<F: Field>(values: &mut [F]) {
    debug_assert!(values.len().is_power_of_two());
    wavelet_transform_batch(values, values.len());
}

pub fn inverse_wavelet_transform<F: Field>(values: &mut [F]) {
    debug_assert!(values.len().is_power_of_two());
    inverse_wavelet_transform_batch(values, values.len());
}

pub fn inverse_wavelet_transform_batch<F: Field>(values: &mut [F], size: usize) {
    debug_assert_eq!(values.len() % size, 0);
    debug_assert!(size.is_power_of_two());
    #[cfg(not(feature = "serial"))]
    if values.len() > workload_size::<F>() && values.len() != size {
        let workload_size = size * std::cmp::max(1, workload_size::<F>() / size);
        return values.par_chunks_mut(workload_size).for_each(|values| {
            inverse_wavelet_transform_batch(values, size);
        });
    }
    match size {
        0 | 1 => {}
        2 => {
            for v in values.chunks_exact_mut(2) {
                v[1] -= v[0];
            }
        }
        4 => {
            for v in values.chunks_exact_mut(4) {
                v[3] -= v[1];
                v[2] -= v[0];
                v[3] -= v[2];
                v[1] -= v[0];
            }
        }
        n => {
            let n1 = 1 << (n.trailing_zeros() / 2);
            let n2 = n / n1;
            inverse_wavelet_transform_batch(values, n1);
            transpose(values, n2, n1);
            inverse_wavelet_transform_batch(values, n2);
            transpose(values, n1, n2);
        }
    }
}

pub fn wavelet_transform_batch<F: Field>(values: &mut [F], size: usize) {
    debug_assert_eq!(values.len() % size, 0);
    debug_assert!(size.is_power_of_two());
    #[cfg(not(feature = "serial"))]
    if values.len() > workload_size::<F>() && values.len() != size {
        let workload_size = size * std::cmp::max(1, workload_size::<F>() / size);
        return values.par_chunks_mut(workload_size).for_each(|values| {
            wavelet_transform_batch(values, size);
        });
    }
    match size {
        0 | 1 => {}
        2 => {
            for v in values.chunks_exact_mut(2) {
                v[1] += v[0];
            }
        }
        4 => {
            for v in values.chunks_exact_mut(4) {
                v[1] += v[0];
                v[3] += v[2];
                v[2] += v[0];
                v[3] += v[1];
            }
        }
        8 => {
            for v in values.chunks_exact_mut(8) {
                v[1] += v[0];
                v[3] += v[2];
                v[2] += v[0];
                v[3] += v[1];
                v[5] += v[4];
                v[7] += v[6];
                v[6] += v[4];
                v[7] += v[5];
                v[4] += v[0];
                v[5] += v[1];
                v[6] += v[2];
                v[7] += v[3];
            }
        }
        16 => {
            for v in values.chunks_exact_mut(16) {
                for v in v.chunks_exact_mut(4) {
                    v[1] += v[0];
                    v[3] += v[2];
                    v[2] += v[0];
                    v[3] += v[1];
                }
                let (a, v) = v.split_at_mut(4);
                let (b, v) = v.split_at_mut(4);
                let (c, d) = v.split_at_mut(4);
                for i in 0..4 {
                    b[i] += a[i];
                    d[i] += c[i];
                    c[i] += a[i];
                    d[i] += b[i];
                }
            }
        }
        n => {
            let n1 = 1 << (n.trailing_zeros() / 2);
            let n2 = n / n1;
            wavelet_transform_batch(values, n1);
            transpose(values, n2, n1);
            wavelet_transform_batch(values, n2);
            transpose(values, n1, n2);
        }
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use crate::snark::provekit::whir::algebra::fields::Field64;

    #[test]
    fn test_wavelet_transform_single_element() {
        let mut values = vec![Field64::from(5)];
        wavelet_transform(&mut values);
        assert_eq!(values, vec![Field64::from(5)]);
    }

    #[test]
    fn test_wavelet_transform_size_2() {
        let v1 = Field64::from(3);
        let v2 = Field64::from(7);
        let mut values = vec![v1, v2];
        wavelet_transform(&mut values);
        assert_eq!(values, vec![v1, v1 + v2]);
    }

    #[test]
    fn test_wavelet_transform_size_4() {
        let v1 = Field64::from(1);
        let v2 = Field64::from(2);
        let v3 = Field64::from(3);
        let v4 = Field64::from(4);
        let mut values = vec![v1, v2, v3, v4];

        wavelet_transform(&mut values);

        assert_eq!(values, vec![v1, v1 + v2, v3 + v1, v1 + v2 + v3 + v4]);
    }

    #[test]
    fn test_wavelet_transform_size_8() {
        let mut values = (1..=8).map(Field64::from).collect::<Vec<_>>();
        let v1 = values[0];
        let v2 = values[1];
        let v3 = values[2];
        let v4 = values[3];
        let v5 = values[4];
        let v6 = values[5];
        let v7 = values[6];
        let v8 = values[7];

        wavelet_transform(&mut values);

        assert_eq!(values, vec![
            v1,
            v1 + v2,
            v3 + v1,
            v1 + v2 + v3 + v4,
            v5 + v1,
            v1 + v2 + v5 + v6,
            v3 + v1 + v5 + v7,
            v1 + v2 + v3 + v4 + v5 + v6 + v7 + v8
        ]);
    }

    #[test]
    fn test_wavelet_transform_size_16() {
        let mut values = (1..=16).map(Field64::from).collect::<Vec<_>>();
        let v1 = values[0];
        let v2 = values[1];
        let v3 = values[2];
        let v4 = values[3];
        let v5 = values[4];
        let v6 = values[5];
        let v7 = values[6];
        let v8 = values[7];
        let v9 = values[8];
        let v10 = values[9];
        let v11 = values[10];
        let v12 = values[11];
        let v13 = values[12];
        let v14 = values[13];
        let v15 = values[14];
        let v16 = values[15];

        wavelet_transform(&mut values);

        assert_eq!(values, vec![
            v1,
            v1 + v2,
            v3 + v1,
            v1 + v2 + v3 + v4,
            v5 + v1,
            v1 + v2 + v5 + v6,
            v3 + v1 + v5 + v7,
            v1 + v2 + v3 + v4 + v5 + v6 + v7 + v8,
            v9 + v1,
            v1 + v2 + v9 + v10,
            v3 + v1 + v9 + v11,
            v1 + v2 + v3 + v4 + v9 + v10 + v11 + v12,
            v5 + v1 + v9 + v13,
            v1 + v2 + v5 + v6 + v9 + v10 + v13 + v14,
            v3 + v1 + v5 + v7 + v9 + v11 + v13 + v15,
            v1 + v2 + v3 + v4 + v5 + v6 + v7 + v8 + v9 + v10 + v11 + v12 + v13 + v14 + v15 + v16
        ]);
    }

    #[test]
    fn test_wavelet_transform_large() {
        let size = 2_i32.pow(10) as u64;
        let mut values = (1..=size).map(Field64::from).collect::<Vec<_>>();
        let v1 = values[0];

        wavelet_transform(&mut values);

        // Verify the first element remains unchanged
        assert_eq!(values[0], v1);

        // Verify last element has accumulated all previous values
        let expected_last = (1..=size).sum::<u64>();
        assert_eq!(values[size as usize - 1], Field64::from(expected_last));
    }

    #[test]
    fn test_wavelet_transform_batch_parallel_chunks() {
        // Define the size for the wavelet transform batch
        let batch_size = 2_i32.pow(20) as usize;
        // Ensure values.len() > size to enter parallel execution
        let total_size = batch_size * 4;
        let mut values = (1..=total_size as u64).map(Field64::from).collect::<Vec<_>>();

        // Keep a copy to compare later
        let original_values = values.clone();

        // Run batch transform on 256-sized chunks
        wavelet_transform_batch(&mut values, batch_size);

        // Verify that the first chunk has been transformed correctly
        let mut expected_chunk = original_values[..batch_size].to_vec();
        wavelet_transform_batch(&mut expected_chunk, batch_size);
        assert_eq!(&values[..batch_size], &expected_chunk);

        // Ensure that the transformation occurred separately for each chunk
        for i in 1..4 {
            let start = i * batch_size;
            let end = start + batch_size;

            let mut expected_chunk = original_values[start..end].to_vec();
            wavelet_transform_batch(&mut expected_chunk, batch_size);

            assert_eq!(&values[start..end], &expected_chunk, "Mismatch in chunk {i}");
        }

        // Ensure the first element remains unchanged
        assert_eq!(values[0], Field64::from(1));

        // Ensure the last element has accumulated all values from its own chunk
        let expected_last_chunk_sum = (total_size as u64 - batch_size as u64 + 1..=total_size as u64).sum::<u64>();
        assert_eq!(values[total_size - 1], Field64::from(expected_last_chunk_sum), "Final element mismatch");
    }
}
