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

// Originally derived from ProveKit, Copyright 2026 World Foundation (MIT).

use crate::snark::provekit::common::{HydratedSparseMatrix, Interner, SparseMatrix, interner::InternedFieldElement};
use ark_ff::Field;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::HashMap;

fn has_duplicate_witnesses<F>(terms: &[(F, usize)]) -> bool {
    for i in 0..terms.len() {
        for j in (i + 1)..terms.len() {
            if terms[i].1 == terms[j].1 {
                return true;
            }
        }
    }
    false
}

/// Merge duplicate witness indices and drop zero-coefficient entries.
fn canonicalize_terms<F: Field>(terms: &[(F, usize)]) -> Vec<(F, usize)> {
    if !has_duplicate_witnesses(terms) {
        return terms.iter().filter(|(c, _)| !c.is_zero()).copied().collect();
    }

    let mut sorted: Vec<(F, usize)> = terms.to_vec();
    sorted.sort_unstable_by_key(|&(_c, w)| w);

    let mut result: Vec<(F, usize)> = Vec::with_capacity(sorted.len());
    let mut acc_coeff = sorted[0].0;
    let mut acc_witness = sorted[0].1;

    for &(coeff, witness) in &sorted[1..] {
        if witness == acc_witness {
            acc_coeff += coeff;
        } else {
            if !acc_coeff.is_zero() {
                result.push((acc_coeff, acc_witness));
            }
            acc_coeff = coeff;
            acc_witness = witness;
        }
    }

    if !acc_coeff.is_zero() {
        result.push((acc_coeff, acc_witness));
    }

    result
}

/// Represents a R1CS constraint system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct R1CS<F: Field> {
    pub num_public_inputs: usize,
    pub interner: Interner<F>,
    pub a: SparseMatrix,
    pub b: SparseMatrix,
    pub c: SparseMatrix,
    /// Virtual witnesses: computation-only columns excluded from matrices/WHIR
    /// commitment but needed by witness builders for intermediate calculations.
    /// The prover allocates `num_witnesses() + num_virtual` for solving.
    #[serde(default)]
    pub num_virtual: usize,
}

impl<F: Field> Default for R1CS<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: Field> R1CS<F> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            num_public_inputs: 0,
            interner: Interner::new(),
            a: SparseMatrix::new(0, 0),
            b: SparseMatrix::new(0, 0),
            c: SparseMatrix::new(0, 0),
            num_virtual: 0,
        }
    }

    #[must_use]
    pub const fn a(&self) -> HydratedSparseMatrix<'_, F> {
        self.a.hydrate(&self.interner)
    }

    #[must_use]
    pub const fn b(&self) -> HydratedSparseMatrix<'_, F> {
        self.b.hydrate(&self.interner)
    }

    #[must_use]
    pub const fn c(&self) -> HydratedSparseMatrix<'_, F> {
        self.c.hydrate(&self.interner)
    }

    /// The number of constraints in the R1CS instance.
    pub const fn num_constraints(&self) -> usize {
        self.a.num_rows
    }

    /// The number of *real* witnesses in the R1CS instance (matrix columns,
    /// including the constant-one witness at index 0).
    ///
    /// **Does NOT include virtual witnesses.** Virtual witnesses are
    /// computation-only columns excluded from the A/B/C matrices and the WHIR
    /// commitment, but still needed by witness builders for intermediate
    /// values. For total allocation needed by the witness solver, use
    /// [`Self::num_witnesses_for_solving`] (or compute
    /// `num_witnesses() + num_virtual`).
    pub const fn num_witnesses(&self) -> usize {
        self.a.num_cols
    }

    /// Total witnesses needed for solving: real witnesses (matrix columns)
    /// plus virtual witnesses (computation-only).
    ///
    /// Use this when allocating the witness vector for the solver. Use
    /// [`Self::num_witnesses`] when reasoning about matrix dimensions or
    /// the WHIR-committed witness.
    pub const fn num_witnesses_for_solving(&self) -> usize {
        self.a.num_cols + self.num_virtual
    }

    /// Compute a SHA3-256 hash of the serialized R1CS matrices.
    ///
    /// Panics if postcard serialization fails, which should not happen for a
    /// well-formed `R1CS` (all fields implement `Serialize`).
    #[must_use]
    pub fn hash(&self) -> crate::snark::provekit::common::R1csHash {
        let bytes = postcard::to_allocvec(self).expect("R1CS serialization should not fail");
        crate::snark::provekit::common::R1csHash::new(Sha3_256::digest(&bytes).into())
    }

    // Increase the size of the R1CS matrices to the specified dimensions.
    pub fn grow_matrices(&mut self, num_rows: usize, num_cols: usize) {
        self.a.grow(num_rows, num_cols);
        self.b.grow(num_rows, num_cols);
        self.c.grow(num_rows, num_cols);
    }

    /// Add a new witnesses to the R1CS instance.
    pub fn add_witnesses(&mut self, count: usize) {
        self.grow_matrices(self.num_constraints(), self.num_witnesses() + count);
    }

    /// Add an R1CS constraint. Duplicate witness indices within each linear
    /// combination are merged (coefficients summed) and zeros are dropped.
    pub fn add_constraint(&mut self, a: &[(F, usize)], b: &[(F, usize)], c: &[(F, usize)]) {
        let a = canonicalize_terms(a);
        let b = canonicalize_terms(b);
        let c = canonicalize_terms(c);

        let next_constraint_idx = self.num_constraints();
        self.grow_matrices(self.num_constraints() + 1, self.num_witnesses());

        for (coeff, witness_idx) in &a {
            self.a.set(next_constraint_idx, *witness_idx, self.interner.intern(*coeff));
        }
        for (coeff, witness_idx) in &b {
            self.b.set(next_constraint_idx, *witness_idx, self.interner.intern(*coeff));
        }
        for (coeff, witness_idx) in &c {
            self.c.set(next_constraint_idx, *witness_idx, self.interner.intern(*coeff));
        }
    }

    pub fn reserve_constraints(&mut self, num_constraints: usize, total_entries: usize) {
        let entries_per_matrix = total_entries / 3 + 1;
        self.a.reserve(num_constraints, entries_per_matrix);
        self.b.reserve(num_constraints, entries_per_matrix);
        self.c.reserve(num_constraints, entries_per_matrix);
    }

    #[inline]
    pub fn push_constraint(
        &mut self,
        a: impl Iterator<Item = (u32, InternedFieldElement)>,
        b: impl Iterator<Item = (u32, InternedFieldElement)>,
        c: impl Iterator<Item = (u32, InternedFieldElement)>,
    ) {
        self.a.push_row(a);
        self.b.push_row(b);
        self.c.push_row(c);
    }

    #[inline]
    pub fn intern(&mut self, value: F) -> InternedFieldElement {
        self.interner.intern(value)
    }

    /// Check if constraint `row` is linear, meaning it can be reduced to
    /// a linear equation over witness variables.
    ///
    /// An R1CS constraint `A·w * B·w = C·w` is linear when at least one of
    /// A or B evaluates to a known constant (only references column 0, the
    /// constant-one witness). This covers:
    /// - Both A and B empty: `0 * 0 = C·w` → `C·w = 0`
    /// - B only references w0: `A·w * const = C·w` → `const*A·w - C·w = 0`
    /// - A only references w0: `const * B·w = C·w` → `const*B·w - C·w = 0`
    pub fn is_linear_constraint(&self, row: usize) -> bool {
        let a_is_const = self.row_is_constant(&self.a, row);
        let b_is_const = self.row_is_constant(&self.b, row);
        a_is_const || b_is_const
    }

    /// Check if a matrix row is "constant" — either empty or only references
    /// column 0 (the constant-one witness).
    fn row_is_constant(&self, matrix: &SparseMatrix, row: usize) -> bool {
        let (cols, _) = matrix.row_slices(row);
        cols.is_empty() || (cols.len() == 1 && cols[0] == 0)
    }

    /// Get the constant value of a "constant" matrix row.
    /// Returns 0 if the row is empty, or the coefficient of w0 if present.
    fn row_constant_value(&self, matrix: &SparseMatrix, row: usize) -> F {
        match matrix.get(row, 0) {
            Some(interned) => self.interner.get(interned).expect("interned value missing"),
            None => F::zero(),
        }
    }

    /// Extract the linear expression from a linear constraint.
    ///
    /// Returns a list of (coefficient, witness_index) pairs such that
    /// sum(coeff_i * w_i) = 0.
    pub fn extract_linear_expression(&self, row: usize) -> Vec<(F, usize)> {
        let a_is_const = self.row_is_constant(&self.a, row);
        let b_is_const = self.row_is_constant(&self.b, row);

        let mut terms: HashMap<usize, F> = HashMap::new();

        if a_is_const && b_is_const {
            let const_a = self.row_constant_value(&self.a, row);
            let const_b = self.row_constant_value(&self.b, row);
            let product = const_a * const_b;
            if !product.is_zero() {
                *terms.entry(0).or_insert_with(F::zero) += product;
            }
            for (col, interned_val) in self.c.iter_row(row) {
                let val = self.interner.get(interned_val).expect("interned value missing");
                *terms.entry(col).or_insert_with(F::zero) -= val;
            }
        } else if a_is_const {
            let const_a = self.row_constant_value(&self.a, row);
            for (col, interned_val) in self.b.iter_row(row) {
                let val = self.interner.get(interned_val).expect("interned value missing");
                *terms.entry(col).or_insert_with(F::zero) += const_a * val;
            }
            for (col, interned_val) in self.c.iter_row(row) {
                let val = self.interner.get(interned_val).expect("interned value missing");
                *terms.entry(col).or_insert_with(F::zero) -= val;
            }
        } else {
            let const_b = self.row_constant_value(&self.b, row);
            for (col, interned_val) in self.a.iter_row(row) {
                let val = self.interner.get(interned_val).expect("interned value missing");
                *terms.entry(col).or_insert_with(F::zero) += const_b * val;
            }
            for (col, interned_val) in self.c.iter_row(row) {
                let val = self.interner.get(interned_val).expect("interned value missing");
                *terms.entry(col).or_insert_with(F::zero) -= val;
            }
        }

        let mut result: Vec<_> = terms.into_iter().filter(|(_, v)| !v.is_zero()).map(|(col, val)| (val, col)).collect();
        result.sort_by_key(|(_, col)| *col);
        result
    }

    /// Remove constraints at the given row indices from all three matrices.
    pub fn remove_constraints(&mut self, rows_to_remove: &[usize]) {
        self.a = self.a.remove_rows(rows_to_remove);
        self.b = self.b.remove_rows(rows_to_remove);
        self.c = self.c.remove_rows(rows_to_remove);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ff::Zero;
    use ark_std::One;
    use whir::algebra::fields::Field64 as FieldElement;

    /// Duplicate witness coefficients are summed, not overwritten.
    #[test]
    fn duplicate_witnesses_are_merged() {
        let mut r1cs = R1CS::new();
        r1cs.add_witnesses(3);

        let a = vec![(FieldElement::from(3u64), 1), (FieldElement::from(5u64), 1)];
        let b = vec![(FieldElement::one(), 0)];
        let c = vec![(FieldElement::from(8u64), 1)];

        r1cs.add_constraint(&a, &b, &c);

        let a_entries: Vec<_> = r1cs.a().iter_row(0).collect();
        assert_eq!(a_entries.len(), 1);
        assert_eq!(a_entries[0], (1, FieldElement::from(8u64)));
    }

    /// Opposite-sign duplicates cancel to zero and produce no entry.
    #[test]
    fn cancelling_duplicates_produce_no_entry() {
        let mut r1cs = R1CS::new();
        r1cs.add_witnesses(3);

        let five = FieldElement::from(5u64);
        let neg_five = FieldElement::zero() - five;
        let a = vec![(five, 1), (neg_five, 1)];
        let b = vec![(FieldElement::one(), 0)];
        let c: Vec<(FieldElement, usize)> = vec![];

        r1cs.add_constraint(&a, &b, &c);

        let a_entries: Vec<_> = r1cs.a().iter_row(0).collect();
        assert!(a_entries.is_empty());
    }

    /// Only duplicate witnesses are merged; distinct witnesses are preserved.
    #[test]
    fn mixed_unique_and_duplicate_witnesses() {
        let mut r1cs = R1CS::new();
        r1cs.add_witnesses(4);

        let a = vec![
            (FieldElement::from(2u64), 1),
            (FieldElement::from(7u64), 2),
            (FieldElement::from(3u64), 1),
            (FieldElement::from(11u64), 3),
        ];
        let b = vec![(FieldElement::one(), 0)];
        let c = vec![];

        r1cs.add_constraint(&a, &b, &c);

        let mut a_entries: Vec<_> = r1cs.a().iter_row(0).collect();
        a_entries.sort_by_key(|(col, _)| *col);
        assert_eq!(a_entries.len(), 3);
        assert_eq!(a_entries[0], (1, FieldElement::from(5u64)));
        assert_eq!(a_entries[1], (2, FieldElement::from(7u64)));
        assert_eq!(a_entries[2], (3, FieldElement::from(11u64)));
    }

    /// Duplicates are merged independently in all three matrices.
    #[test]
    fn duplicates_in_all_matrices() {
        let mut r1cs = R1CS::new();
        r1cs.add_witnesses(3);

        let a = vec![(FieldElement::from(1u64), 1), (FieldElement::from(2u64), 1)];
        let b = vec![(FieldElement::from(3u64), 2), (FieldElement::from(4u64), 2)];
        let c = vec![(FieldElement::from(5u64), 1), (FieldElement::from(6u64), 1)];

        r1cs.add_constraint(&a, &b, &c);

        let a_entries: Vec<_> = r1cs.a().iter_row(0).collect();
        assert_eq!(a_entries, vec![(1, FieldElement::from(3u64))]);

        let b_entries: Vec<_> = r1cs.b().iter_row(0).collect();
        assert_eq!(b_entries, vec![(2, FieldElement::from(7u64))]);

        let c_entries: Vec<_> = r1cs.c().iter_row(0).collect();
        assert_eq!(c_entries, vec![(1, FieldElement::from(11u64))]);
    }

    #[test]
    fn canonicalize_terms_basics() {
        assert!(canonicalize_terms::<FieldElement>(&[]).is_empty());
        assert!(canonicalize_terms(&[(FieldElement::zero(), 0)]).is_empty());

        let result = canonicalize_terms(&[(FieldElement::from(42u64), 5)]);
        assert_eq!(result, vec![(FieldElement::from(42u64), 5)]);

        // 1 + 2 + 3 = 6
        let result = canonicalize_terms(&[
            (FieldElement::from(1u64), 7),
            (FieldElement::from(2u64), 7),
            (FieldElement::from(3u64), 7),
        ]);
        assert_eq!(result, vec![(FieldElement::from(6u64), 7)]);
    }
}
