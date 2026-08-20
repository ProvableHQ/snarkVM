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

#[cfg(not(feature = "serial"))]
use rayon::join;
use snarkvm_fields::Field;

use crate::snark::provekit::whir::algebra::{dot, embedding::Embedding, scalar_mul};
#[cfg(not(feature = "serial"))]
use crate::snark::provekit::whir::utils::workload_size;

/// Computes the constant and quadratic coefficient of the sumcheck polynomial.
///
/// Vectors `a` and `b` are implicitly zero-extended to the next power of two.
pub fn compute_sumcheck_polynomial<F: Field>(a: &[F], b: &[F]) -> (F, F) {
    fn recurse<F: Field>(a0: &[F], a1: &[F], b0: &[F], b1: &[F]) -> (F, F) {
        debug_assert_eq!(a0.len(), b0.len());
        debug_assert_eq!(a1.len(), b1.len());
        debug_assert!(a0.len() == a1.len());

        #[cfg(not(feature = "serial"))]
        if a0.len() * 4 > workload_size::<F>() {
            let mid = a0.len() / 2;
            let (a0l, a0r) = a0.split_at(mid);
            let (b0l, b0r) = b0.split_at(mid);
            let (a1l, a1r) = a1.split_at(mid);
            let (b1l, b1r) = b1.split_at(mid);
            let (left, right) = join(|| recurse(a0l, a1l, b0l, b1l), || recurse(a0r, a1r, b0r, b1r));
            return (left.0 + right.0, left.1 + right.1);
        }
        let mut acc0 = F::zero();
        let mut acc2 = F::zero();
        for ((&a0, &a1), (&b0, &b1)) in a0.iter().zip(a1).zip(b0.iter().zip(b1)) {
            acc0 += a0 * b0;
            acc2 += (a1 - a0) * (b1 - b0);
        }
        (acc0, acc2)
    }

    let non_padded = a.len().min(b.len());
    let a = &a[..non_padded];
    let b = &b[..non_padded];
    if a.is_empty() {
        return (F::zero(), F::zero());
    }
    if a.len() == 1 {
        return (a[0] * b[0], F::zero());
    }

    let half = a.len().next_power_of_two() >> 1;
    let (a0, a1) = a.split_at(half);
    let (b0, b1) = b.split_at(half);
    debug_assert!(a0.len() >= a1.len());
    let (a0, a0_tail) = a0.split_at(a1.len());
    let (b0, b0_tail) = b0.split_at(a1.len());
    let (acc0, acc2) = recurse(a0, a1, b0, b1);

    // Handle the tail part where a1, b1 is implicit zero padding,
    // When a1, b1 = 0, then acc0 = acc2 = a0 * b0:
    let acc = dot(a0_tail, b0_tail);

    (acc0 + acc, acc2 + acc)
}

/// Folds evaluations by linear interpolation at the given weight, in place.
///
/// The `values` are implicitly zero-padded to the next power of two. On return,
/// the length of `values` will always be a power of two.
pub fn fold<F: Field>(values: &mut Vec<F>, weight: F) {
    fn recurse_both<F: Field>(low: &mut [F], high: &[F], weight: F) {
        #[cfg(not(feature = "serial"))]
        if low.len() > workload_size::<F>() {
            let split = low.len() / 2;
            let (ll, lr) = low.split_at_mut(split);
            let (hl, hr) = high.split_at(split);
            rayon::join(|| recurse_both(ll, hl, weight), || recurse_both(lr, hr, weight));
            return;
        }

        for (low, high) in low.iter_mut().zip(high) {
            *low += (*high - *low) * weight;
        }
    }

    if values.len() <= 1 {
        return;
    }

    let half = values.len().next_power_of_two() >> 1;
    let (low, high) = values.split_at_mut(half);
    debug_assert!(low.len() >= high.len());
    let (low, tail) = low.split_at_mut(high.len());
    recurse_both(low, high, weight);

    // Tail part where `high` is implicit zero padding
    // When high = 0 we have *low *= 1 - weight.
    scalar_mul(tail, F::one() - weight);

    values.truncate(half);
    values.shrink_to_fit();
}

pub fn fold_and_compute_polynomial<F: Field>(a: &mut Vec<F>, b: &mut Vec<F>, weight: F) -> (F, F) {
    // TODO: Replace with a single pass implementation.
    fold(a, weight);
    fold(b, weight);
    compute_sumcheck_polynomial(a, b)
}

/// Evaluate a coefficient vector at a multilinear point in the target field.
pub fn mixed_eval<M: Embedding>(
    embedding: &M,
    coeff: &[M::Source],
    eval: &[M::Target],
    scalar: M::Target,
) -> M::Target {
    debug_assert_eq!(coeff.len(), 1 << eval.len());

    if let Some((&x, tail)) = eval.split_first() {
        let (low, high) = coeff.split_at(coeff.len() / 2);

        #[cfg(not(feature = "serial"))]
        if low.len() > workload_size::<M::Source>() {
            let (a, b) =
                join(|| mixed_eval(embedding, low, tail, scalar), || mixed_eval(embedding, high, tail, scalar * x));
            return a + b;
        }

        mixed_eval(embedding, low, tail, scalar) + mixed_eval(embedding, high, tail, scalar * x)
    } else {
        embedding.mixed_mul(scalar, coeff[0])
    }
}

#[cfg(any())]
pub(crate) mod tests {
    use proptest::proptest;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    use super::*;
    use crate::snark::provekit::whir::algebra::{fields::Field64, random_vector};

    type F = Field64;

    /// Zero-pad to the next power of two.
    pub fn zero_pad<F: Field>(values: &[F]) -> Vec<F> {
        if values.is_empty() {
            return Vec::new();
        }
        let mut vec = values.to_vec();
        vec.resize(vec.len().next_power_of_two(), F::zero());
        vec
    }

    #[test]
    fn sumcheck_poly_zero_extend() {
        proptest!(|(seed:u64, length in 0_usize..(1 << 14))| {
            let mut rng = StdRng::seed_from_u64(seed);
            let vector: Vec<F> = random_vector(&mut rng, length);
            let covector: Vec<F> = random_vector(&mut rng, length);
            let extended_vector = zero_pad(&vector);
            let extended_covector = zero_pad(&covector);
            let expected = compute_sumcheck_polynomial(&extended_vector, &extended_covector);
            assert_eq!(compute_sumcheck_polynomial(&vector, &covector), expected);
            assert_eq!(compute_sumcheck_polynomial(&extended_vector, &covector), expected);
            assert_eq!(compute_sumcheck_polynomial(&vector, &extended_covector), expected);
        });
    }

    #[test]
    fn fold_zero_extend() {
        proptest!(|(seed:u64, length in 0_usize..(1 << 14))| {
            let mut rng = StdRng::seed_from_u64(seed);
            let mut vector: Vec<F> = random_vector(&mut rng, length);
            let mut extended_vector = zero_pad(&vector);
            let weight = rng.gen::<F>();

            fold(&mut vector, weight);
            assert!(vector.is_empty() || vector.len().is_power_of_two());
            fold(&mut extended_vector, weight);
            assert_eq!(vector, extended_vector);
        });
    }
}
