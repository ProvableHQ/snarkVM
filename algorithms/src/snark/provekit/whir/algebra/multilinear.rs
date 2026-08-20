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

use snarkvm_fields::{Field, One, Zero};

use crate::snark::provekit::whir::algebra::embedding::{Embedding, Identity};

/// Evaluate the multi-linear extension of `evals` in `point`.
pub fn multilinear_extend<F: Field>(evals: &[F], point: &[F]) -> F {
    mixed_multilinear_extend(&Identity::<F>::new(), evals, point)
}

/// Evaluate the multi-linear extension of `evals` in `point`.
///
/// Supports implicit zero-padding: when `evals.len() < 1 << point.len()`,
/// the missing tail entries are treated as zeros.
#[allow(clippy::too_many_lines)]
pub fn mixed_multilinear_extend<M: Embedding>(embedding: &M, evals: &[M::Source], point: &[M::Target]) -> M::Target {
    #[inline]
    fn eval_exact<M: Embedding>(embedding: &M, evals: &[M::Source], point: &[M::Target]) -> M::Target {
        debug_assert_eq!(evals.len(), 1 << point.len());

        // Helper to compute (a + (b - a) * c) efficiently with a, b in source field.
        let mixed = |a, b, c| embedding.mixed_add(embedding.mixed_mul(c, b - a), a);

        match point {
            [] => embedding.map(evals[0]),
            [x] => mixed(evals[0], evals[1], *x),
            [x0, x1] => {
                let a0 = mixed(evals[0], evals[1], *x1);
                let a1 = mixed(evals[2], evals[3], *x1);
                a0 + (a1 - a0) * *x0
            }
            [x0, x1, x2] => {
                let a00 = mixed(evals[0], evals[1], *x2);
                let a01 = mixed(evals[2], evals[3], *x2);
                let a10 = mixed(evals[4], evals[5], *x2);
                let a11 = mixed(evals[6], evals[7], *x2);
                let a0 = a00 + (a01 - a00) * *x1;
                let a1 = a10 + (a11 - a10) * *x1;
                a0 + (a1 - a0) * *x0
            }
            [x0, x1, x2, x3] => {
                let a000 = mixed(evals[0], evals[1], *x3);
                let a001 = mixed(evals[2], evals[3], *x3);
                let a010 = mixed(evals[4], evals[5], *x3);
                let a011 = mixed(evals[6], evals[7], *x3);
                let a100 = mixed(evals[8], evals[9], *x3);
                let a101 = mixed(evals[10], evals[11], *x3);
                let a110 = mixed(evals[12], evals[13], *x3);
                let a111 = mixed(evals[14], evals[15], *x3);
                let a00 = a000 + (a001 - a000) * *x2;
                let a01 = a010 + (a011 - a010) * *x2;
                let a10 = a100 + (a101 - a100) * *x2;
                let a11 = a110 + (a111 - a110) * *x2;
                let a0 = a00 + (a01 - a00) * *x1;
                let a1 = a10 + (a11 - a10) * *x1;
                a0 + (a1 - a0) * *x0
            }
            [x, tail @ ..] => {
                let (f0, f1) = evals.split_at(evals.len() / 2);
                #[cfg(feature = "serial")]
                let (f0, f1) = (eval_exact(embedding, f0, tail), eval_exact(embedding, f1, tail));

                #[cfg(not(feature = "serial"))]
                let (f0, f1) = {
                    use crate::snark::provekit::whir::utils::workload_size;
                    if evals.len() > workload_size::<M::Source>() {
                        rayon::join(|| eval_exact(embedding, f0, tail), || eval_exact(embedding, f1, tail))
                    } else {
                        (eval_exact(embedding, f0, tail), eval_exact(embedding, f1, tail))
                    }
                };

                f0 + (f1 - f0) * *x
            }
        }
    }

    #[inline]
    fn eval_partial<M: Embedding>(embedding: &M, evals: &[M::Source], point: &[M::Target]) -> M::Target {
        let size = 1 << point.len();
        debug_assert!(evals.len() <= size);
        if evals.is_empty() {
            return M::Target::zero();
        }
        if evals.len() == size {
            return eval_exact(embedding, evals, point);
        }

        match point {
            [] => embedding.map(evals[0]),
            [x, tail @ ..] => {
                let half = size / 2;

                // Only low half has data; high half is all implicit zeros.
                if evals.len() <= half {
                    let f0 = eval_partial(embedding, evals, tail);
                    return f0 * (M::Target::one() - *x);
                }

                // Low subtree is exact/full, high subtree is partial.
                let (low, high) = evals.split_at(half);

                #[cfg(feature = "serial")]
                let (f0, f1) = (eval_exact(embedding, low, tail), eval_partial(embedding, high, tail));

                #[cfg(not(feature = "serial"))]
                let (f0, f1) = {
                    use crate::snark::provekit::whir::utils::workload_size;
                    if evals.len() > workload_size::<M::Source>() {
                        rayon::join(|| eval_exact(embedding, low, tail), || eval_partial(embedding, high, tail))
                    } else {
                        (eval_exact(embedding, low, tail), eval_partial(embedding, high, tail))
                    }
                };

                f0 + (f1 - f0) * *x
            }
        }
    }

    eval_partial(embedding, evals, point)
}

/// Computes eq(points, p) on the hypercube for all p ∈ {0,1}^k.
pub fn eq_weights<F: Field>(point: &[F]) -> Vec<F> {
    let mut result = vec![F::zero(); 1 << point.len()];
    eval_eq(&mut result, point, F::one());
    result
}

/// Accumulates a scaled evaluation of the equality function.
///
/// Given an evaluation point `point`, the function computes
/// the equality polynomial recursively using the formula:
///
/// ```text
/// eq(X) = ∏ (1 - X_i + 2X_i z_i)
/// ```
///
/// where `z_i` are the points.
pub fn eval_eq<F: Field>(accumulator: &mut [F], point: &[F], scalar: F) {
    assert_eq!(accumulator.len(), 1 << point.len());
    if let [x0, xs @ ..] = point {
        let (acc_0, acc_1) = accumulator.split_at_mut(1 << xs.len());
        let s1 = scalar * x0; // Contribution when `X_i = 1`
        let s0 = scalar - s1; // Contribution when `X_i = 0`

        #[cfg(not(feature = "serial"))]
        {
            use crate::snark::provekit::whir::utils::workload_size;
            if acc_0.len() > workload_size::<F>() {
                rayon::join(|| eval_eq(acc_0, xs, s0), || eval_eq(acc_1, xs, s1));
                return;
            }
        }
        eval_eq(acc_0, xs, s0);
        eval_eq(acc_1, xs, s1);
    } else {
        accumulator[0] += scalar;
    }
}

#[cfg(any())]
mod tests {
    use proptest::proptest;
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;
    use crate::snark::provekit::whir::algebra::{random_vector, sumcheck::tests::zero_pad};

    pub type F = crate::snark::provekit::whir::algebra::fields::Field64;

    #[test]
    fn test_multilinear_zero_extend() {
        proptest!(|(seed:u64, length in 0_usize..(1 << 14))| {
            let mut rng = StdRng::seed_from_u64(seed);
            let vector: Vec<F> = random_vector(&mut rng, length);
            let extended_vector = zero_pad(&vector);
            let num_variables = length.next_power_of_two().trailing_zeros();
            let point = random_vector(&mut rng, num_variables as usize);
            assert_eq!(
                multilinear_extend(&vector, &point),
                multilinear_extend(&extended_vector, &point)
            );
        });
    }

    #[test]
    fn test_multilinear_extra_variables() {
        proptest!(|(seed:u64, length in 0_usize..(1 << 10), excess_variables in 0_usize..3)| {
            let mut rng = StdRng::seed_from_u64(seed);
            let vector: Vec<F> = random_vector(&mut rng, length);
            let num_variables = length.next_power_of_two().trailing_zeros() as usize + excess_variables;
            let point = random_vector(&mut rng, num_variables);
            let mut extended_vector = vector.clone();
            extended_vector.resize(1 << num_variables, F::zero());
            assert_eq!(
                multilinear_extend(&vector, &point),
                multilinear_extend(&extended_vector, &point)
            );
        });
    }
}
