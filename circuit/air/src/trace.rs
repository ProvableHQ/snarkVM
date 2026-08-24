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

use snarkvm_fields::PrimeField;

use anyhow::{Result, ensure};

/// A row-major execution trace (main or preprocessed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trace<F: PrimeField> {
    width: usize,
    height: usize,
    values: Vec<F>,
}

impl<F: PrimeField> Trace<F> {
    /// Constructs a trace from row-major values of length `width * height`.
    pub fn new(width: usize, height: usize, values: Vec<F>) -> Result<Self> {
        ensure!(width > 0, "Trace width must be positive");
        ensure!(height > 0, "Trace height must be positive");
        ensure!(
            width.checked_mul(height) == Some(values.len()),
            "Trace values length {} does not match width {width} * height {height}",
            values.len()
        );
        Ok(Self { width, height, values })
    }

    /// Returns the number of columns.
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Returns the number of rows.
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Returns the row-major values.
    pub fn values(&self) -> &[F] {
        &self.values
    }

    /// Returns row `row` as a slice of length `width`.
    pub fn row(&self, row: usize) -> &[F] {
        let start = row * self.width;
        &self.values[start..start + self.width]
    }

    /// Returns a mutable row `row`.
    pub fn row_mut(&mut self, row: usize) -> &mut [F] {
        let start = row * self.width;
        &mut self.values[start..start + self.width]
    }

    /// Returns the entry at `(row, column)`.
    pub fn get(&self, row: usize, column: usize) -> F {
        self.values[row * self.width + column]
    }

    /// Returns a mutable entry at `(row, column)`.
    pub fn get_mut(&mut self, row: usize, column: usize) -> &mut F {
        &mut self.values[row * self.width + column]
    }
}

/// A local/next window over one pair of adjacent rows.
#[derive(Clone, Debug)]
pub struct Window<T> {
    /// Values on the current row.
    pub local: Vec<T>,
    /// Values on the next row (zeros when there is no transition).
    pub next: Vec<T>,
}

impl<T> Window<T> {
    /// Returns the current row.
    pub fn local(&self) -> &[T] {
        &self.local
    }

    /// Returns the next row.
    pub fn next(&self) -> &[T] {
        &self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_curves::bls12_377::Fr;

    #[test]
    fn test_trace_new_rejects_invalid_dimensions() {
        assert!(Trace::<Fr>::new(0, 1, vec![]).is_err());
        assert!(Trace::<Fr>::new(1, 0, vec![]).is_err());
        assert!(Trace::<Fr>::new(2, 2, vec![Fr::default()]).is_err());
        assert!(Trace::<Fr>::new(1, 1, vec![Fr::default()]).is_ok());
    }
}
