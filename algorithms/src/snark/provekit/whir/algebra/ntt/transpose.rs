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

#![allow(unsafe_code)]

use std::mem::swap;

use super::MatrixMut;
use crate::snark::provekit::whir::utils::workload_size;

// NOTE: The assumption that rows and cols are a power of two are actually only
// relevant for the square matrix case. (This is because the algorithm recurses
// into 4 sub-matrices of half dimension; we assume those to be square matrices
// as well, which only works for powers of two).

/// Compute the row-major index permutation for a transposition.
#[cfg(any())]
pub fn transpose_permute(index: usize, rows: usize, cols: usize) -> usize {
    debug_assert!(index < rows * cols);
    let (row, col) = (index / cols, index % cols);
    row + col * rows
}

/// Transposes a matrix in-place.
///
/// This function processes a batch of matrices if the slice length is a
/// multiple of `rows * cols`. Assumes that both `rows` and `cols` are powers of
/// two.
pub fn transpose<F: Sized + Copy + Send>(matrix: &mut [F], rows: usize, cols: usize) {
    assert!(matrix.len().is_multiple_of(rows * cols));
    if !rows.is_power_of_two() || !cols.is_power_of_two() {
        // Fall back to non-recursive.
        if matrix.is_empty() {
            return;
        }
        let mut buffer = vec![matrix[0]; rows * cols];
        for matrix in matrix.chunks_exact_mut(rows * cols) {
            transpose_copy(
                MatrixMut::from_mut_slice(matrix, rows, cols),
                MatrixMut::from_mut_slice(buffer.as_mut_slice(), cols, rows),
            );
            matrix.copy_from_slice(&buffer);
        }
        return;
    }

    debug_assert!(rows.is_power_of_two());
    debug_assert!(cols.is_power_of_two());
    if rows == cols {
        for matrix in matrix.chunks_exact_mut(rows * cols) {
            let matrix = MatrixMut::from_mut_slice(matrix, rows, cols);
            transpose_square(matrix);
        }
    } else {
        // TODO: Special case for rows = 2 * cols and cols = 2 * rows.
        // TODO: Special case for very wide matrices (e.g. n x 16).
        let mut scratch = Vec::with_capacity(rows * cols);
        for matrix in matrix.chunks_exact_mut(rows * cols) {
            scratch.clear();
            scratch.extend_from_slice(matrix);
            let src = MatrixMut::from_mut_slice(scratch.as_mut_slice(), rows, cols);
            let dst = MatrixMut::from_mut_slice(matrix, cols, rows);
            transpose_copy(src, dst);
        }
    }
}

/// Transposes a rectangular matrix into another matrix.
fn transpose_copy<F: Sized + Copy + Send>(src: MatrixMut<'_, F>, mut dst: MatrixMut<'_, F>) {
    assert_eq!(src.rows(), dst.cols());
    assert_eq!(src.cols(), dst.rows());

    let (rows, cols) = (src.rows(), src.cols());

    // Direct element-wise transposition for small matrices (avoids recursion
    // overhead)
    if rows * cols * 2 <= workload_size::<F>() {
        unsafe {
            for i in 0..rows {
                for j in 0..cols {
                    *dst.ptr_at_mut(j, i) = *src.ptr_at(i, j);
                }
            }
        }
        return;
    }

    // Determine optimal split axis
    let (src_a, src_b, dst_a, dst_b) = if rows > cols {
        let split_size = rows / 2;
        let (s1, s2) = src.split_vertical(split_size);
        let (d1, d2) = dst.split_horizontal(split_size);
        (s1, s2, d1, d2)
    } else {
        let split_size = cols / 2;
        let (s1, s2) = src.split_horizontal(split_size);
        let (d1, d2) = dst.split_vertical(split_size);
        (s1, s2, d1, d2)
    };

    #[cfg(not(feature = "serial"))]
    rayon::join(|| transpose_copy(src_a, dst_a), || transpose_copy(src_b, dst_b));

    #[cfg(feature = "serial")]
    for (s, mut d) in [(src_a, dst_a), (src_b, dst_b)] {
        for i in 0..s.rows() {
            for j in 0..s.cols() {
                d[(j, i)] = s[(i, j)];
            }
        }
    }
}

/// Transposes a square matrix in-place.
fn transpose_square<F: Sized + Send>(mut m: MatrixMut<F>) {
    debug_assert!(m.is_square());
    debug_assert!(m.rows().is_power_of_two());
    let size = m.rows();

    if size * size > workload_size::<F>() {
        // Recurse into quadrants.
        // This results in a cache-oblivious algorithm.
        let n = size / 2;
        let (a, b, c, d) = m.split_quadrants(n, n);

        #[cfg(not(feature = "serial"))]
        rayon::join(|| transpose_square_swap(b, c), || rayon::join(|| transpose_square(a), || transpose_square(d)));

        #[cfg(feature = "serial")]
        {
            transpose_square(a);
            transpose_square(d);
            transpose_square_swap(b, c);
        }
    } else {
        for i in 0..size {
            for j in (i + 1)..size {
                unsafe {
                    m.swap((i, j), (j, i));
                }
            }
        }
    }
}

/// Swaps two square sub-matrices in-place, transposing them simultaneously.
fn transpose_square_swap<F: Sized + Send>(mut a: MatrixMut<'_, F>, mut b: MatrixMut<'_, F>) {
    debug_assert!(a.is_square());
    debug_assert_eq!(a.rows(), b.cols());
    debug_assert_eq!(a.cols(), b.rows());
    debug_assert!(a.rows().is_power_of_two());
    debug_assert!(workload_size::<F>() >= 2);

    let size = a.rows();

    // Direct swaps for small matrices (≤8x8)
    // - Avoids recursion overhead
    // - Uses basic element-wise swaps
    if size <= 8 {
        for i in 0..size {
            for j in 0..size {
                swap(&mut a[(i, j)], &mut b[(j, i)]);
            }
        }
        return;
    }

    // If the matrix is large, use recursive subdivision:
    // - Improves cache efficiency by working on smaller blocks
    // - Enables parallel execution
    if 2 * size * size > workload_size::<F>() {
        let n = size / 2;
        let (a_tl, a_tr, a_bl, a_br) = a.split_quadrants(n, n);
        let (b_tl, b_tr, b_bl, b_br) = b.split_quadrants(n, n);

        #[cfg(not(feature = "serial"))]
        rayon::join(
            || rayon::join(|| transpose_square_swap(a_tl, b_tl), || transpose_square_swap(a_tr, b_bl)),
            || rayon::join(|| transpose_square_swap(a_bl, b_tr), || transpose_square_swap(a_br, b_br)),
        );

        #[cfg(feature = "serial")]
        {
            transpose_square_swap(a_tl, b_tl);
            transpose_square_swap(a_tr, b_bl);
            transpose_square_swap(a_bl, b_tr);
            transpose_square_swap(a_br, b_br);
        }
    } else {
        // Optimized 2×2 loop unrolling for larger blocks
        // - Reduces loop overhead
        // - Increases memory access efficiency
        for i in (0..size).step_by(2) {
            for j in (0..size).step_by(2) {
                swap(&mut a[(i, j)], &mut b[(j, i)]);
                swap(&mut a[(i + 1, j)], &mut b[(j, i + 1)]);
                swap(&mut a[(i, j + 1)], &mut b[(j + 1, i)]);
                swap(&mut a[(i + 1, j + 1)], &mut b[(j + 1, i + 1)]);
            }
        }
    }
}

#[cfg(any())]
mod tests {
    use proptest::prelude::*;

    use super::*;

    type Pair = (usize, usize);
    type Triple = (usize, usize, usize);

    /// Creates a `rows x columns` matrix stored as a flat vector.
    /// Each element `(i, j)` represents its row and column position.
    fn make_example_matrix(rows: usize, columns: usize) -> Vec<Pair> {
        (0..rows).flat_map(|i| (0..columns).map(move |j| (i, j))).collect()
    }

    /// Creates a sequence of `instances` matrices, each of size `rows x
    /// columns`.
    ///
    /// Each element in the `index`-th matrix is `(index, row, col)`, stored in
    /// a flat vector.
    fn make_example_matrices(rows: usize, columns: usize, instances: usize) -> Vec<Triple> {
        let mut matrices = Vec::with_capacity(rows * columns * instances);

        for index in 0..instances {
            for row in 0..rows {
                for col in 0..columns {
                    matrices.push((index, row, col));
                }
            }
        }

        matrices
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn test_transpose_copy() {
        let rows: usize = workload_size::<Pair>() + 1; // intentionally not a power of two: The function is not described as only working for powers of two.
        let columns: usize = 4;
        let mut srcarray = make_example_matrix(rows, columns);
        let mut dstarray: Vec<(usize, usize)> = vec![(0, 0); rows * columns];

        let src1 = MatrixMut::<Pair>::from_mut_slice(&mut srcarray[..], rows, columns);
        let dst1 = MatrixMut::<Pair>::from_mut_slice(&mut dstarray[..], columns, rows);

        transpose_copy(src1, dst1);
        let dst1 = MatrixMut::<Pair>::from_mut_slice(&mut dstarray[..], columns, rows);

        for i in 0..rows {
            for j in 0..columns {
                assert_eq!(dst1[(j, i)], (i, j));
            }
        }
    }

    #[test]
    fn test_transpose_square_swap() {
        // Set rows manually. We want to be sure to trigger the actual recursion.
        // (Computing this from workload_size was too much hassle.)
        let rows = 1024; // workload_size::<Triple>();
        assert!(rows * rows > 2 * workload_size::<Triple>());

        let examples: Vec<Triple> = make_example_matrices(rows, rows, 2);
        // Make copies for simplicity, because we borrow different parts.
        let mut examples1 = Vec::from(&examples[0..rows * rows]);
        let mut examples2 = Vec::from(&examples[rows * rows..2 * rows * rows]);

        let view1 = MatrixMut::from_mut_slice(&mut examples1, rows, rows);
        let view2 = MatrixMut::from_mut_slice(&mut examples2, rows, rows);
        for i in 0..rows {
            for j in 0..rows {
                assert_eq!(view1[(i, j)], (0, i, j));
                assert_eq!(view2[(i, j)], (1, i, j));
            }
        }
        transpose_square_swap(view1, view2);
        let view1 = MatrixMut::from_mut_slice(&mut examples1, rows, rows);
        let view2 = MatrixMut::from_mut_slice(&mut examples2, rows, rows);
        for i in 0..rows {
            for j in 0..rows {
                assert_eq!(view1[(i, j)], (1, j, i));
                assert_eq!(view2[(i, j)], (0, j, i));
            }
        }
    }

    #[test]
    fn test_transpose_square() {
        // Set rows manually. We want to be sure to trigger the actual recursion.
        // (Computing this from workload_size was too much hassle.)
        let size = 1024;
        assert!(size * size > 2 * workload_size::<Pair>());

        let mut example = make_example_matrix(size, size);
        let view = MatrixMut::from_mut_slice(&mut example, size, size);
        transpose_square(view);
        let view = MatrixMut::from_mut_slice(&mut example, size, size);
        for i in 0..size {
            for j in 0..size {
                assert_eq!(view[(i, j)], (j, i));
            }
        }
    }

    #[test]
    fn test_transpose() {
        let size = 1024;

        // rectangular matrix:
        let rows = size;
        let cols = 16;
        let mut example = make_example_matrix(rows, cols);
        transpose(&mut example, rows, cols);
        let view = MatrixMut::from_mut_slice(&mut example, cols, rows);
        for i in 0..cols {
            for j in 0..rows {
                assert_eq!(view[(i, j)], (j, i));
            }
        }

        // square matrix:
        let rows = size;
        let cols = size;
        let mut example = make_example_matrix(rows, cols);
        transpose(&mut example, rows, cols);
        let view = MatrixMut::from_mut_slice(&mut example, cols, rows);
        for i in 0..cols {
            for j in 0..rows {
                assert_eq!(view[(i, j)], (j, i));
            }
        }

        // 20 rectangular matrices:
        let number_of_matrices = 20;
        let rows = size;
        let cols = 16;
        let mut example = make_example_matrices(rows, cols, number_of_matrices);
        transpose(&mut example, rows, cols);
        for index in 0..number_of_matrices {
            let view =
                MatrixMut::from_mut_slice(&mut example[index * rows * cols..(index + 1) * rows * cols], cols, rows);
            for i in 0..cols {
                for j in 0..rows {
                    assert_eq!(view[(i, j)], (index, j, i));
                }
            }
        }

        // 20 square matrices:
        let number_of_matrices = 20;
        let rows = size;
        let cols = size;
        let mut example = make_example_matrices(rows, cols, number_of_matrices);
        transpose(&mut example, rows, cols);
        for index in 0..number_of_matrices {
            let view =
                MatrixMut::from_mut_slice(&mut example[index * rows * cols..(index + 1) * rows * cols], cols, rows);
            for i in 0..cols {
                for j in 0..rows {
                    assert_eq!(view[(i, j)], (index, j, i));
                }
            }
        }
    }

    fn arb_pow2() -> impl Strategy<Value = usize> {
        (0..=8).prop_map(|log2_size| 1_usize << log2_size)
    }

    /// Generates random square matrices with sizes that are powers of two.
    #[allow(clippy::cast_sign_loss)]
    fn arb_square_matrix() -> impl Strategy<Value = (Vec<usize>, usize)> {
        arb_pow2().prop_map(|size| size * size).prop_flat_map(|matrix_size| {
            prop::collection::vec(0usize..1000, matrix_size)
                .prop_map(move |matrix| (matrix, (matrix_size as f64).sqrt() as usize))
        })
    }

    /// Generates random rectangular matrices where rows and columns are powers
    /// of two.
    fn arb_rect_matrix() -> impl Strategy<Value = (Vec<usize>, usize, usize)> {
        (arb_pow2(), arb_pow2()).prop_flat_map(|(rows, cols)| {
            prop::collection::vec(0usize..1000, rows * cols).prop_map(move |matrix| (matrix, rows, cols))
        })
    }

    proptest! {
        #[test]
        fn proptest_transpose_square((mut matrix, size) in arb_square_matrix()) {
            let original = matrix.clone();
            transpose(&mut matrix, size, size);
            transpose(&mut matrix, size, size);
            prop_assert_eq!(matrix, original);
        }

        #[test]
        fn proptest_transpose_rect((mut matrix, rows, cols) in arb_rect_matrix()) {
            let original = matrix.clone();
            transpose(&mut matrix, rows, cols);

            let view = MatrixMut::from_mut_slice(&mut matrix, cols, rows);

            // Verify that each (i, j) moved to (j, i)
            for i in 0..cols {
                for j in 0..rows {
                    prop_assert_eq!(view[(i, j)], original[j * cols + i]);
                }
            }
        }
    }
}
