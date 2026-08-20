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

use crate::snark::provekit::{
    common::{
        R1CS,
        sparse_matrix::SparseMatrix,
        utils::{unzip_double_array, workload_size},
    },
    whir::algebra::embedding::Embedding,
};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use snarkvm_fields::Field;
use std::array;

/// Compute the sum of a vector valued function over the boolean hypercube in
/// the leading variable.
pub fn sumcheck_fold_map_reduce<const N: usize, const M: usize, F: Field>(
    mles: [&mut [F]; N],
    fold: Option<F>,
    map: impl Fn([(F, F); N]) -> [F; M] + Send + Sync + Copy,
) -> [F; M] {
    let size = mles[0].len();
    assert!(size.is_power_of_two());
    assert!(size >= 2);
    assert!(mles.iter().all(|mle| mle.len() == size));

    if let Some(fold) = fold {
        assert!(size >= 4);
        let slices = mles.map(|mle| {
            let (p0, tail) = mle.split_at_mut(size / 4);
            let (p1, tail) = tail.split_at_mut(size / 4);
            let (p2, p3) = tail.split_at_mut(size / 4);
            [p0, p1, p2, p3]
        });
        sumcheck_fold_map_reduce_inner::<N, M, F>(slices, fold, map)
    } else {
        let slices = mles.map(|mle| mle.split_at(size / 2));
        sumcheck_map_reduce_inner::<N, M, F>(slices, map)
    }
}

fn sumcheck_map_reduce_inner<const N: usize, const M: usize, F: Field>(
    mles: [(&[F], &[F]); N],
    map: impl Fn([(F, F); N]) -> [F; M] + Send + Sync + Copy,
) -> [F; M] {
    let size = mles[0].0.len();
    if size * N * 2 > workload_size::<F>() {
        // Split slices
        let pairs = mles.map(|(p0, p1)| (p0.split_at(size / 2), p1.split_at(size / 2)));
        let left = pairs.map(|((l0, _), (l1, _))| (l0, l1));
        let right = pairs.map(|((_, r0), (_, r1))| (r0, r1));

        // Parallel recurse
        let (l, r) = rayon::join(|| sumcheck_map_reduce_inner(left, map), || sumcheck_map_reduce_inner(right, map));

        // Combine results
        array::from_fn(|i| l[i] + r[i])
    } else {
        let mut result = [F::zero(); M];
        for i in 0..size {
            let e = mles.map(|(p0, p1)| (p0[i], p1[i]));
            let local = map(e);
            result.iter_mut().zip(local).for_each(|(r, l)| *r += l);
        }
        result
    }
}

fn sumcheck_fold_map_reduce_inner<const N: usize, const M: usize, F: Field>(
    mut mles: [[&mut [F]; 4]; N],
    fold: F,
    map: impl Fn([(F, F); N]) -> [F; M] + Send + Sync + Copy,
) -> [F; M] {
    let size = mles[0][0].len();
    if size * N * 4 > workload_size::<F>() {
        // Split slices
        let pairs = mles.map(|mles| mles.map(|p| p.split_at_mut(size / 2)));
        let (left, right) = unzip_double_array(pairs);

        // Parallel recurse
        let (l, r) = rayon::join(
            || sumcheck_fold_map_reduce_inner(left, fold, map),
            || sumcheck_fold_map_reduce_inner(right, fold, map),
        );

        // Combine results
        array::from_fn(|i| l[i] + r[i])
    } else {
        let mut result = [F::zero(); M];
        for i in 0..size {
            let e = array::from_fn(|j| {
                let mle = &mut mles[j];
                mle[0][i] += fold * (mle[2][i] - mle[0][i]);
                mle[1][i] += fold * (mle[3][i] - mle[1][i]);
                (mle[0][i], mle[1][i])
            });
            let local = map(e);
            result.iter_mut().zip(local).for_each(|(r, l)| *r += l);
        }
        result
    }
}

/// Compute the sum of a vector valued function over the boolean hypercube in
/// the leading variable, with base-field `mles` against an extension-field
/// `ext_mle`.
pub fn mixed_sumcheck_map_reduce<const N: usize, const M: usize, F: Field, G: Field>(
    mles: [&[F]; N],
    ext_mle: &[G],
    map: impl Fn([(F, F); N], (G, G)) -> [G; M] + Send + Sync + Copy,
) -> [G; M] {
    let size = ext_mle.len();
    assert!(size.is_power_of_two());
    assert!(size >= 2);
    assert!(mles.iter().all(|mle| mle.len() == size));

    let mles = mles.map(|mle| mle.split_at(size / 2));
    mixed_map_reduce_inner::<N, M, F, G>(mles, ext_mle.split_at(size / 2), map)
}

fn mixed_map_reduce_inner<const N: usize, const M: usize, F: Field, G: Field>(
    mles: [(&[F], &[F]); N],
    ext_mle: (&[G], &[G]),
    map: impl Fn([(F, F); N], (G, G)) -> [G; M] + Send + Sync + Copy,
) -> [G; M] {
    let size = ext_mle.0.len();
    if size * (N + 1) * 2 > workload_size::<G>() {
        // Split slices
        let pairs = mles.map(|(p0, p1)| (p0.split_at(size / 2), p1.split_at(size / 2)));
        let left = pairs.map(|((l0, _), (l1, _))| (l0, l1));
        let right = pairs.map(|((_, r0), (_, r1))| (r0, r1));
        let (ext_p0, ext_p1) = (ext_mle.0.split_at(size / 2), ext_mle.1.split_at(size / 2));

        // Parallel recurse
        let (l, r) = rayon::join(
            || mixed_map_reduce_inner(left, (ext_p0.0, ext_p1.0), map),
            || mixed_map_reduce_inner(right, (ext_p0.1, ext_p1.1), map),
        );

        // Combine results
        array::from_fn(|i| l[i] + r[i])
    } else {
        let mut result = [G::zero(); M];
        for i in 0..size {
            let e = mles.map(|(p0, p1)| (p0[i], p1[i]));
            let local = map(e, (ext_mle.0[i], ext_mle.1[i]));
            result.iter_mut().zip(local).for_each(|(r, l)| *r += l);
        }
        result
    }
}

/// Fold the leading variable of a base-field mle at an extension-field point,
/// lifting it into the extension: `out[i] = mle[i] + point * (mle[i + n/2] -
/// mle[i])`.
pub fn mixed_fold<M: Embedding>(embedding: &M, mle: &[M::Source], point: M::Target) -> Vec<M::Target> {
    assert!(mle.len().is_power_of_two());
    assert!(mle.len() >= 2);
    let (p0, p1) = mle.split_at(mle.len() / 2);
    p0.par_iter()
        .zip(p1.par_iter())
        .with_min_len(workload_size::<M::Target>())
        .map(|(&e0, &e1)| embedding.mixed_add(embedding.mixed_mul(point, e1 - e0), e0))
        .collect()
}

/// List of evaluations for eq(r, x) over the boolean hypercube, truncated to
/// `num_entries` elements. When `num_entries < 2^r.len()`, avoids allocating
/// the full hypercube.
pub fn calculate_evaluations_over_boolean_hypercube_for_eq<F: Field>(r: &[F], num_entries: usize) -> Vec<F> {
    if num_entries == 0 {
        return vec![];
    }
    let full_size = 1usize << r.len();
    assert!(num_entries <= full_size);
    let mut result = vec![F::zero(); num_entries];
    eval_eq(r, &mut result, F::one(), full_size);
    result
}

/// Evaluates the equality polynomial recursively. `subtree_size` tracks the
/// logical size of this recursion level so that truncated output buffers are
/// split correctly.
fn eval_eq<F: Field>(eval: &[F], out: &mut [F], scalar: F, subtree_size: usize) {
    debug_assert!(out.len() <= subtree_size);
    if let Some((&x, tail)) = eval.split_first() {
        let half = subtree_size / 2;
        let left_len = out.len().min(half);
        let right_len = out.len().saturating_sub(half);
        let (o0, o1) = out.split_at_mut(left_len);
        let s1 = scalar * x;
        let s0 = scalar - s1;
        if right_len == 0 {
            eval_eq(tail, o0, s0, half);
        } else if subtree_size > workload_size::<F>() {
            rayon::join(|| eval_eq(tail, o0, s0, half), || eval_eq(tail, o1, s1, half));
        } else {
            eval_eq(tail, o0, s0, half);
            eval_eq(tail, o1, s1, half);
        }
    } else {
        out[0] += scalar;
    }
}

/// Evaluates a quadratic polynomial on a value
pub fn eval_quadratic_poly<F: Field>(poly: [F; 3], point: F) -> F {
    poly[0] + point * (poly[1] + point * poly[2])
}

/// Evaluates a cubic polynomial on a value
pub fn eval_cubic_poly<F: Field>(poly: [F; 4], point: F) -> F {
    poly[0] + point * (poly[1] + point * (poly[2] + point * poly[3]))
}

/// Given a path to JSON file with sparse matrices and a witness, calculates
/// matrix-vector multiplication and returns them
pub fn calculate_witness_bounds<F: Field>(r1cs: &R1CS<F>, witness: &[F]) -> (Vec<F>, Vec<F>, Vec<F>) {
    let (a, b) = rayon::join(|| r1cs.a() * witness, || r1cs.b() * witness);

    let target_len = a.len().next_power_of_two();
    let mut c = Vec::with_capacity(target_len);
    c.extend(a.iter().zip(b.iter()).map(|(a, b)| *a * *b));
    c.resize(target_len, F::zero());

    let mut a = a;
    let mut b = b;
    a.resize(target_len, F::zero());
    b.resize(target_len, F::zero());
    (a, b, c)
}

/// Calculates eq(r, alpha)
pub fn calculate_eq<F: Field>(r: &[F], alpha: &[F]) -> F {
    r.iter()
        .zip(alpha.iter())
        .fold(F::one(), |acc, (&r, &alpha)| acc * (r * alpha + (F::one() - r) * (F::one() - alpha)))
}

/// Transpose all three R1CS matrices in parallel.
///
/// This depends only on the R1CS structure (from the verifier key), not on any
/// proof-specific data, so it can run concurrently with sumcheck verification.
pub fn transpose_r1cs_matrices<F: Field>(r1cs: &R1CS<F>) -> (SparseMatrix, SparseMatrix, SparseMatrix) {
    let ((at, bt), ct) =
        rayon::join(|| rayon::join(|| r1cs.a.transpose(), || r1cs.b.transpose()), || r1cs.c.transpose());
    (at, bt, ct)
}

/// Multiply pre-transposed R1CS matrices by eq(alpha, ·) to compute the
/// external row.
pub fn multiply_transposed_by_eq_alpha<M: Embedding>(
    embedding: &M,
    at: &SparseMatrix,
    bt: &SparseMatrix,
    ct: &SparseMatrix,
    alpha: &[M::Target],
    r1cs: &R1CS<M::Source>,
) -> [Vec<M::Target>; 3] {
    let eq_alpha = calculate_evaluations_over_boolean_hypercube_for_eq(alpha, r1cs.num_constraints());
    let interner = &r1cs.interner;
    let ((a, b), c) = rayon::join(
        || {
            rayon::join(
                || at.hydrate(interner).mixed_multiply(embedding, &eq_alpha),
                || bt.hydrate(interner).mixed_multiply(embedding, &eq_alpha),
            )
        },
        || ct.hydrate(interner).mixed_multiply(embedding, &eq_alpha),
    );

    [a, b, c]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snark::provekit::whir::algebra::embedding::Identity;
    use snarkvm_curves::bls12_377::Fr as FieldElement;
    use snarkvm_fields::{One, Zero};

    fn fe(v: i64) -> FieldElement {
        if v >= 0 { FieldElement::from(v as u64) } else { FieldElement::from(0u64) - FieldElement::from((-v) as u64) }
    }

    /// Build a small 3×4 R1CS for matrix tests.
    ///
    /// A = [[1, 2, 0, 0],   B = [[0, 1, 0, 0],   C = [[0, 0, 1, 0],
    ///      [0, 0, 3, 0],        [2, 0, 0, 1],        [0, 1, 0, 3],
    ///      [1, 0, 0, 1]]        [0, 0, 4, 0]]        [2, 0, 0, 0]]
    fn make_test_r1cs() -> crate::snark::provekit::common::R1CS<FieldElement> {
        let mut r1cs = crate::snark::provekit::common::R1CS::<FieldElement>::new();
        r1cs.add_witnesses(4);
        r1cs.add_constraint(&[(fe(1), 0), (fe(2), 1)], &[(fe(1), 1)], &[(fe(1), 2)]);
        r1cs.add_constraint(&[(fe(3), 2)], &[(fe(2), 0), (fe(1), 3)], &[(fe(1), 1), (fe(3), 3)]);
        r1cs.add_constraint(&[(fe(1), 0), (fe(1), 3)], &[(fe(4), 2)], &[(fe(2), 0)]);
        r1cs
    }

    /// calculate_eq

    #[test]
    fn test_calculate_eq_non_boolean() {
        // r = [2,3,4,5], alpha = [6,7,8,9]
        // eq = 17 × 33 × 53 × 77 = 2,289,441
        let r = [fe(2), fe(3), fe(4), fe(5)];
        let alpha = [fe(6), fe(7), fe(8), fe(9)];
        assert_eq!(calculate_eq(&r, &alpha), fe(2_289_441));
    }

    #[test]
    fn test_calculate_eq_boolean_identity() {
        let r = [fe(0), fe(1), fe(1), fe(0)];
        assert_eq!(calculate_eq(&r, &[fe(0), fe(1), fe(1), fe(0)]), fe(1));
        assert_eq!(calculate_eq(&r, &[fe(1), fe(0), fe(0), fe(1)]), fe(0));
    }

    #[test]
    fn test_calculate_eq_empty() {
        assert_eq!(calculate_eq::<FieldElement>(&[], &[]), fe(1));
    }

    /// calculate_evaluations_over_boolean_hypercube_for_eq

    #[test]
    fn test_eq_hypercube_len4() {
        // r of dimension 4 → 16-entry hypercube.
        // Cross-validate every entry against calculate_eq.
        let r = [fe(2), fe(3), fe(4), fe(5)];
        let result = calculate_evaluations_over_boolean_hypercube_for_eq(&r, 16);
        assert_eq!(result.len(), 16);
        let n = r.len();
        for (i, val) in result.iter().enumerate() {
            let point: Vec<FieldElement> = (0..n).map(|j| fe(((i >> (n - 1 - j)) & 1) as i64)).collect();
            let expected = calculate_eq(&r, &point);
            assert_eq!(*val, expected, "mismatch at index {i}");
        }
    }
    #[test]
    fn test_eq_hypercube_truncated() {
        let r = [fe(2), fe(3), fe(4), fe(5)];
        let full = calculate_evaluations_over_boolean_hypercube_for_eq(&r, 16);
        let truncated = calculate_evaluations_over_boolean_hypercube_for_eq(&r, 10);
        assert_eq!(truncated.len(), 10);
        assert_eq!(&full[..10], truncated.as_slice());
    }

    #[test]
    fn test_eq_hypercube_empty_r() {
        let result = calculate_evaluations_over_boolean_hypercube_for_eq::<FieldElement>(&[], 1);
        assert_eq!(result, vec![fe(1)]);
    }

    #[test]
    fn test_eq_hypercube_zero_entries() {
        let result = calculate_evaluations_over_boolean_hypercube_for_eq(&[fe(2), fe(3), fe(5)], 0);
        assert!(result.is_empty(), "non-empty r, zero entries");

        let result = calculate_evaluations_over_boolean_hypercube_for_eq::<FieldElement>(&[], 0);
        assert!(result.is_empty(), "empty r, zero entries");
    }

    /// eval_eq

    #[test]
    fn test_eval_eq_base_case() {
        // Base case: eval is empty, so out[0] += scalar.
        let mut out = [fe(7)];
        eval_eq(&[], &mut out, fe(3), 1);
        assert_eq!(out[0], fe(10));
    }

    #[test]
    fn test_eval_eq_truncated_left_only() {
        // eval = [2,3,5], out has 1 slot (right_len = 0 each time), subtree_size = 8.
        // Expected: eq([2,3,5], [0,0,0]) = (1-2)(1-3)(1-5) = (-1)(-2)(-4) = -8
        let mut out = [FieldElement::zero()];
        eval_eq(&[fe(2), fe(3), fe(5)], &mut out, FieldElement::one(), 8);
        assert_eq!(out[0], fe(-8));
    }

    #[test]
    fn test_eval_eq_both_halves() {
        // r = [2, 3], out has 4 slots → right_len = 2, exercises both-halves path.
        let mut out = vec![FieldElement::zero(); 4];
        eval_eq(&[fe(2), fe(3)], &mut out, FieldElement::one(), 4);
        let expected = calculate_evaluations_over_boolean_hypercube_for_eq(&[fe(2), fe(3)], 4);
        assert_eq!(out, expected);
    }

    #[test]
    fn test_eq_hypercube_single_entry() {
        // num_entries == 1 with non-empty r: only the all-zeros vertex is computed.
        // eq([2,3], [0,0]) = (1-2)(1-3) = (-1)(-2) = 2
        let r = [fe(2), fe(3)];
        let result = calculate_evaluations_over_boolean_hypercube_for_eq(&r, 1);
        assert_eq!(result, vec![calculate_eq(&r, &[fe(0), fe(0)])]);
    }

    // mixed_sumcheck_map_reduce / mixed_fold

    /// The cubic sumcheck round evaluations, base mles against ext eq.
    fn mixed_round_map(
        embedding: &Identity<FieldElement>,
        [a, b, c]: [(FieldElement, FieldElement); 3],
        eq: (FieldElement, FieldElement),
    ) -> [FieldElement; 3] {
        let f0 = embedding.mixed_mul(eq.0, a.0 * b.0 - c.0);
        let f_em1 = embedding.mixed_mul(eq.0 + eq.0 - eq.1, (a.0 + a.0 - a.1) * (b.0 + b.0 - b.1) - (c.0 + c.0 - c.1));
        let f_inf = embedding.mixed_mul(eq.1 - eq.0, (a.1 - a.0) * (b.1 - b.0));
        [f0, f_em1, f_inf]
    }

    /// The same round evaluations, everything lifted to the extension.
    fn ext_round_map([a, b, c, eq]: [(FieldElement, FieldElement); 4]) -> [FieldElement; 3] {
        let f0 = eq.0 * (a.0 * b.0 - c.0);
        let f_em1 = (eq.0 + eq.0 - eq.1) * ((a.0 + a.0 - a.1) * (b.0 + b.0 - b.1) - (c.0 + c.0 - c.1));
        let f_inf = (eq.1 - eq.0) * (a.1 - a.0) * (b.1 - b.0);
        [f0, f_em1, f_inf]
    }

    fn mixed_test_mles(n: usize) -> ([Vec<FieldElement>; 3], Vec<FieldElement>) {
        let a = (0..n).map(|i| fe(i as i64 + 1)).collect();
        let b = (0..n).map(|i| fe(2 * i as i64 + 3)).collect();
        let c = (0..n).map(|i| fe(7 * i as i64 + 5)).collect();
        let eq = (0..n).map(|i| fe(i as i64 + 2)).collect();
        ([a, b, c], eq)
    }

    #[test]
    fn test_mixed_map_reduce_matches_lifted() {
        let embedding = Identity::<FieldElement>::new();
        let ([a, b, c], eq) = mixed_test_mles(16);

        let mixed =
            mixed_sumcheck_map_reduce([&a[..], &b[..], &c[..]], &eq, |mles, eq| mixed_round_map(&embedding, mles, eq));

        let (mut la, mut lb, mut lc, mut leq) = (embedding.map_vec(a), embedding.map_vec(b), embedding.map_vec(c), eq);
        let lifted = sumcheck_fold_map_reduce([&mut la, &mut lb, &mut lc, &mut leq], None, ext_round_map);

        assert_eq!(mixed, lifted);
    }

    #[test]
    fn test_mixed_fold_matches_lifted() {
        let embedding = Identity::<FieldElement>::new();
        let n = 8;
        let mle: Vec<FieldElement> = (0..n).map(|i| fe(3 * i as i64 + 2)).collect();
        let point = fe(5);

        let folded = mixed_fold(&embedding, &mle, point);

        let lifted = embedding.map_vec(mle);
        let (p0, p1) = lifted.split_at(n / 2);
        let expected: Vec<FieldElement> = p0.iter().zip(p1).map(|(&l, &h)| l + point * (h - l)).collect();
        assert_eq!(folded, expected);
    }

    #[test]
    fn test_mixed_fold_identity() {
        let n = 4;
        let mle: Vec<FieldElement> = (0..n).map(|i| fe(i as i64 + 1)).collect();
        let point = fe(9);

        let folded = mixed_fold(&Identity::new(), &mle, point);

        let expected: Vec<FieldElement> = (0..n / 2).map(|i| mle[i] + point * (mle[i + n / 2] - mle[i])).collect();
        assert_eq!(folded, expected);
    }

    /// Explicit `mixed_fold` + fold-free round must match the fused in-place
    /// fold inside `sumcheck_fold_map_reduce` — the parity the prover's
    /// base-field first round relies on.
    #[test]
    fn test_explicit_mixed_fold_matches_fused_fold() {
        let embedding = Identity::<FieldElement>::new();
        let ([a, b, c], eq) = mixed_test_mles(8);
        let alpha = fe(3);

        let (mut la, mut lb, mut lc, mut leq) =
            (embedding.map_vec(a.clone()), embedding.map_vec(b.clone()), embedding.map_vec(c.clone()), eq.clone());
        let fused = sumcheck_fold_map_reduce([&mut la, &mut lb, &mut lc, &mut leq], Some(alpha), ext_round_map);

        let (mut fa, mut fb, mut fc, mut feq) = (
            mixed_fold(&embedding, &a, alpha),
            mixed_fold(&embedding, &b, alpha),
            mixed_fold(&embedding, &c, alpha),
            mixed_fold(&Identity::new(), &eq, alpha),
        );
        let explicit = sumcheck_fold_map_reduce([&mut fa, &mut fb, &mut fc, &mut feq], None, ext_round_map);

        assert_eq!(fused, explicit);
    }

    /// transpose_r1cs_matrices

    #[test]
    fn test_transpose_r1cs_matrices() {
        let r1cs = make_test_r1cs();
        let (at, bt, ct) = transpose_r1cs_matrices(&r1cs);

        // Dimensions swapped: 3×4 → 4×3.
        assert_eq!((at.num_rows, at.num_cols), (4, 3));
        assert_eq!((bt.num_rows, bt.num_cols), (4, 3));
        assert_eq!((ct.num_rows, ct.num_cols), (4, 3));

        // Standard basis vectors e_i pick out row i of M and column i of M^T.
        // Looping over all 3 gives 3×4 = 12 equations, one per entry of each
        // 3×4 matrix — fully determined coverage.
        let expected_a_rows =
            [vec![fe(1), fe(2), fe(0), fe(0)], vec![fe(0), fe(0), fe(3), fe(0)], vec![fe(1), fe(0), fe(0), fe(1)]];
        let expected_b_rows =
            [vec![fe(0), fe(1), fe(0), fe(0)], vec![fe(2), fe(0), fe(0), fe(1)], vec![fe(0), fe(0), fe(4), fe(0)]];
        let expected_c_rows =
            [vec![fe(0), fe(0), fe(1), fe(0)], vec![fe(0), fe(1), fe(0), fe(3)], vec![fe(2), fe(0), fe(0), fe(0)]];

        // A^T · e_i extracts column i of A^T, which must equal row i of A.
        // Three basis vectors cover all 12 entries of each 3×4 matrix.
        for i in 0..r1cs.num_constraints() {
            let mut e = vec![FieldElement::zero(); r1cs.num_constraints()];
            e[i] = FieldElement::one();

            assert_eq!(at.hydrate(&r1cs.interner) * e.as_slice(), expected_a_rows[i], "AT col {i}");
            assert_eq!(bt.hydrate(&r1cs.interner) * e.as_slice(), expected_b_rows[i], "BT col {i}");
            assert_eq!(ct.hydrate(&r1cs.interner) * e.as_slice(), expected_c_rows[i], "CT col {i}");
        }
    }

    /// multiply_transposed_by_eq_alpha

    #[test]
    fn test_multiply_transposed_by_eq_alpha() {
        let r1cs = make_test_r1cs();
        let (at, bt, ct) = transpose_r1cs_matrices(&r1cs);
        // alpha length 2 → full EQ size 4, truncated to 3 constraints.
        let alpha = [fe(2), fe(3)];

        let expected_a = vec![fe(-2), fe(4), fe(-9), fe(-4)];
        let expected_b = vec![fe(-6), fe(2), fe(-16), fe(-3)];
        let expected_c = vec![fe(-8), fe(-3), fe(2), fe(-9)];

        let [actual_a, actual_b, actual_c] = multiply_transposed_by_eq_alpha(
            &crate::snark::provekit::whir::algebra::embedding::Identity::<FieldElement>::new(),
            &at,
            &bt,
            &ct,
            &alpha,
            &r1cs,
        );

        assert_eq!(actual_a.len(), r1cs.num_witnesses());
        assert_eq!(actual_a, expected_a, "A result mismatch");
        assert_eq!(actual_b, expected_b, "B result mismatch");
        assert_eq!(actual_c, expected_c, "C result mismatch");
    }
}
