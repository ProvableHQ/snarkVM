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

//! Minimal matrix class that supports strided access.
//! This abstracts over the unsafe pointer arithmetic required for
//! transpose-like algorithms.

#![allow(unsafe_code)]

use std::{
    marker::PhantomData,
    ops::{Index, IndexMut},
    ptr,
    slice,
};

/// Mutable reference to a matrix.
///
/// The invariant this data structure maintains is that `data` has lifetime
/// `'a` and points to a collection of `rows` rowws, at intervals `row_stride`,
/// each of length `cols`.
pub struct MatrixMut<'a, T> {
    data: *mut T,
    rows: usize,
    cols: usize,
    row_stride: usize,
    _lifetime: PhantomData<&'a mut T>,
}

unsafe impl<T: Send> Send for MatrixMut<'_, T> {}

unsafe impl<T: Sync> Sync for MatrixMut<'_, T> {}

impl<'a, T> MatrixMut<'a, T> {
    /// creates a MatrixMut from `slice`, where slice is the concatenations of
    /// `rows` rows, each consisting of `cols` many entries.
    pub fn from_mut_slice(slice: &'a mut [T], rows: usize, cols: usize) -> Self {
        assert_eq!(slice.len(), rows * cols);
        // Safety: The input slice is valid for the lifetime `'a` and has
        // `rows` contiguous rows of length `cols`.
        Self { data: slice.as_mut_ptr(), rows, cols, row_stride: cols, _lifetime: PhantomData }
    }

    /// returns the number of rows
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// returns the number of columns
    pub const fn cols(&self) -> usize {
        self.cols
    }

    /// checks whether the matrix is a square matrix
    pub const fn is_square(&self) -> bool {
        self.rows == self.cols
    }

    /// returns a mutable reference to the `row`'th row of the MatrixMut
    #[allow(dead_code)]
    pub fn row(&mut self, row: usize) -> &mut [T] {
        assert!(row < self.rows);
        // Safety: The structure invariant guarantees that at offset `row *
        // self.row_stride` there is valid data of length `self.cols`.
        unsafe { slice::from_raw_parts_mut(self.data.add(row * self.row_stride), self.cols) }
    }

    /// Split the matrix into two vertically at the `row`'th row (meaning that
    /// in the returned pair (A,B), the matrix A has `row` rows).
    ///
    /// [A]
    /// [ ] = self
    /// [B]
    pub fn split_vertical(self, row: usize) -> (Self, Self) {
        assert!(row <= self.rows);
        (
            Self { data: self.data, rows: row, cols: self.cols, row_stride: self.row_stride, _lifetime: PhantomData },
            Self {
                data: unsafe { self.data.add(row * self.row_stride) },
                rows: self.rows - row,
                cols: self.cols,
                row_stride: self.row_stride,
                _lifetime: PhantomData,
            },
        )
    }

    /// Split the matrix into two horizontally at the `col`th column (meaning
    /// that in the returned pair (A,B), the matrix A has `col` columns).
    ///
    /// [A B] = self
    pub fn split_horizontal(self, col: usize) -> (Self, Self) {
        assert!(col <= self.cols);
        (
            // Safety: This reduces the number of cols, keeping all else the same.
            Self { data: self.data, rows: self.rows, cols: col, row_stride: self.row_stride, _lifetime: PhantomData },
            // Safety: This reduces the number of cols and offsets and, keeping all else the same.
            Self {
                data: unsafe { self.data.add(col) },
                rows: self.rows,
                cols: self.cols - col,
                row_stride: self.row_stride,
                _lifetime: PhantomData,
            },
        )
    }

    /// Split the matrix into four quadrants at the indicated `row` and `col`
    /// (meaning that in the returned 4-tuple (A,B,C,D), the matrix A is a
    /// `row`x`col` matrix)
    ///
    /// self = [A B]
    ///        [C D]
    pub fn split_quadrants(self, row: usize, col: usize) -> (Self, Self, Self, Self) {
        let (u, l) = self.split_vertical(row); // split into upper and lower parts
        let (a, b) = u.split_horizontal(col);
        let (c, d) = l.split_horizontal(col);
        (a, b, c, d)
    }

    /// Swap two elements `a` and `b` in the matrix.
    /// Each of `a`, `b` is given as (row,column)-pair.
    /// If the given coordinates are out-of-bounds, the behaviour is undefined.
    pub unsafe fn swap(&mut self, a: (usize, usize), b: (usize, usize)) {
        if a != b {
            unsafe {
                let a = self.ptr_at_mut(a.0, a.1);
                let b = self.ptr_at_mut(b.0, b.1);
                ptr::swap_nonoverlapping(a, b, 1);
            }
        }
    }

    /// returns an immutable pointer to the element at (`row`, `col`). This
    /// performs no bounds checking and provining indices out-of-bounds is UB.
    pub(crate) const unsafe fn ptr_at(&self, row: usize, col: usize) -> *const T {
        // Safe to call under the following assertion (checked by caller)
        // assert!(row < self.rows);
        // assert!(col < self.cols);

        // Safety: The structure invariant guarantees that at offset `row *
        // self.row_stride + col` there is valid data.
        unsafe { self.data.add(row * self.row_stride + col) }
    }

    /// returns a mutable pointer to the element at (`row`, `col`). This
    /// performs no bounds checking and provining indices out-of-bounds is UB.
    pub(crate) const unsafe fn ptr_at_mut(&mut self, row: usize, col: usize) -> *mut T {
        // Safe to call under the following assertion (checked by caller)
        //
        // assert!(row < self.rows);
        // assert!(col < self.cols);

        // Safety: The structure invariant guarantees that at offset `row *
        // self.row_stride + col` there is valid data.
        unsafe { self.data.add(row * self.row_stride + col) }
    }
}

// Use MatrixMut::ptr_at and MatrixMut::ptr_at_mut to implement Index and
// IndexMut. The latter are not unsafe, since they contain bounds-checks.

impl<T> Index<(usize, usize)> for MatrixMut<'_, T> {
    type Output = T;

    fn index(&self, (row, col): (usize, usize)) -> &T {
        assert!(row < self.rows);
        assert!(col < self.cols);
        // Safety: The structure invariant guarantees that at offset `row *
        // self.row_stride + col` there is valid data.
        unsafe { &*self.ptr_at(row, col) }
    }
}

impl<T> IndexMut<(usize, usize)> for MatrixMut<'_, T> {
    fn index_mut(&mut self, (row, col): (usize, usize)) -> &mut T {
        assert!(row < self.rows);
        assert!(col < self.cols);
        // Safety: The structure invariant guarantees that at offset `row *
        // self.row_stride + col` there is valid data.
        unsafe { &mut *self.ptr_at_mut(row, col) }
    }
}

#[cfg(any())]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_creation() {
        let mut data = vec![1, 2, 3, 4, 5, 6];
        let matrix = MatrixMut::from_mut_slice(&mut data, 2, 3);
        assert_eq!(matrix.rows(), 2);
        assert_eq!(matrix.cols(), 3);
        assert!(!matrix.is_square());
        assert_eq!(matrix[(0, 0)], 1);
        assert_eq!(matrix[(1, 0)], 4);

        assert_eq!(matrix[(0, 1)], 2);
        assert_eq!(matrix[(1, 1)], 5);

        assert_eq!(matrix[(0, 2)], 3);
        assert_eq!(matrix[(1, 2)], 6);
    }

    #[test]
    fn test_row_access() {
        let mut data = vec![1, 2, 3, 4, 5, 6];
        let mut matrix = MatrixMut::from_mut_slice(&mut data, 2, 3);
        assert_eq!(matrix.row(0), &[1, 2, 3]);
        assert_eq!(matrix.row(1), &[4, 5, 6]);
    }

    #[test]
    fn test_split_vertical() {
        let mut data = vec![1, 2, 3, 4, 5, 6];
        let matrix = MatrixMut::from_mut_slice(&mut data, 2, 3);
        let (top, bottom) = matrix.split_vertical(1);
        assert_eq!(top.rows(), 1);
        assert_eq!(top[(0, 0)], 1);
        assert_eq!(top[(0, 1)], 2);
        assert_eq!(top[(0, 2)], 3);

        assert_eq!(bottom.rows(), 1);
        assert_eq!(bottom[(0, 0)], 4);
        assert_eq!(bottom[(0, 1)], 5);
        assert_eq!(bottom[(0, 2)], 6);
    }

    #[test]
    fn test_split_horizontal() {
        let mut data = vec![1, 2, 3, 4, 5, 6];
        let matrix = MatrixMut::from_mut_slice(&mut data, 2, 3);
        let (left, right) = matrix.split_horizontal(1);
        assert_eq!(left.cols(), 1);
        assert_eq!(left[(0, 0)], 1);
        assert_eq!(left[(1, 0)], 4);

        assert_eq!(right.cols(), 2);
        assert_eq!(right[(0, 0)], 2);
        assert_eq!(right[(0, 1)], 3);
        assert_eq!(right[(1, 0)], 5);
        assert_eq!(right[(1, 1)], 6);
    }

    #[test]
    fn test_element_access() {
        let mut data = vec![1, 2, 3, 4, 5, 6];
        let matrix = MatrixMut::from_mut_slice(&mut data, 2, 3);
        assert_eq!(matrix[(0, 1)], 2);
    }

    #[test]
    fn test_swap() {
        let mut data = vec![1, 2, 3, 4, 5, 6];
        let mut matrix = MatrixMut::from_mut_slice(&mut data, 2, 3);
        unsafe {
            matrix.swap((0, 0), (1, 1));
        }
        assert_eq!(matrix[(0, 0)], 5);
        assert_eq!(matrix[(1, 1)], 1);
    }

    #[test]
    fn test_split_quadrants_even() {
        let mut data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let matrix = MatrixMut::from_mut_slice(&mut data, 4, 4);

        let (a, b, c, d) = matrix.split_quadrants(2, 2);

        // Check dimensions
        assert_eq!(a.rows(), 2);
        assert_eq!(a.cols(), 2);
        assert_eq!(b.rows(), 2);
        assert_eq!(b.cols(), 2);
        assert_eq!(c.rows(), 2);
        assert_eq!(c.cols(), 2);
        assert_eq!(d.rows(), 2);
        assert_eq!(d.cols(), 2);

        // Check values in quadrants
        assert_eq!(a[(0, 0)], 1);
        assert_eq!(a[(0, 1)], 2);
        assert_eq!(a[(1, 0)], 5);
        assert_eq!(a[(1, 1)], 6);

        assert_eq!(b[(0, 0)], 3);
        assert_eq!(b[(0, 1)], 4);
        assert_eq!(b[(1, 0)], 7);
        assert_eq!(b[(1, 1)], 8);

        assert_eq!(c[(0, 0)], 9);
        assert_eq!(c[(0, 1)], 10);
        assert_eq!(c[(1, 0)], 13);
        assert_eq!(c[(1, 1)], 14);

        assert_eq!(d[(0, 0)], 11);
        assert_eq!(d[(0, 1)], 12);
        assert_eq!(d[(1, 0)], 15);
        assert_eq!(d[(1, 1)], 16);
    }

    #[test]
    fn test_split_quadrants_odd_rows() {
        let mut data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        let matrix = MatrixMut::from_mut_slice(&mut data, 3, 3);

        let (a, b, c, d) = matrix.split_quadrants(1, 1);

        // Check dimensions
        assert_eq!(a.rows(), 1);
        assert_eq!(a.cols(), 1);
        assert_eq!(b.rows(), 1);
        assert_eq!(b.cols(), 2);
        assert_eq!(c.rows(), 2);
        assert_eq!(c.cols(), 1);
        assert_eq!(d.rows(), 2);
        assert_eq!(d.cols(), 2);

        // Check values in quadrants
        assert_eq!(a[(0, 0)], 1);

        assert_eq!(b[(0, 0)], 2);
        assert_eq!(b[(0, 1)], 3);

        assert_eq!(c[(0, 0)], 4);
        assert_eq!(c[(1, 0)], 7);

        assert_eq!(d[(0, 0)], 5);
        assert_eq!(d[(0, 1)], 6);
        assert_eq!(d[(1, 0)], 8);
        assert_eq!(d[(1, 1)], 9);
    }

    #[test]
    fn test_split_quadrants_odd_cols() {
        let mut data = vec![1, 2, 3, 4, 5, 6];
        let matrix = MatrixMut::from_mut_slice(&mut data, 3, 2);

        let (a, b, c, d) = matrix.split_quadrants(1, 1);

        // Check dimensions
        assert_eq!(a.rows(), 1);
        assert_eq!(a.cols(), 1);
        assert_eq!(b.rows(), 1);
        assert_eq!(b.cols(), 1);
        assert_eq!(c.rows(), 2);
        assert_eq!(c.cols(), 1);
        assert_eq!(d.rows(), 2);
        assert_eq!(d.cols(), 1);

        // Check values in quadrants
        assert_eq!(a[(0, 0)], 1);

        assert_eq!(b[(0, 0)], 2);

        assert_eq!(c[(0, 0)], 3);
        assert_eq!(c[(1, 0)], 5);

        assert_eq!(d[(0, 0)], 4);
        assert_eq!(d[(1, 0)], 6);
    }
}
