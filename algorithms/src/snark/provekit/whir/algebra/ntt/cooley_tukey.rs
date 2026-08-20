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

//! Number-theoretic transforms (NTTs) over fields with high two-adicity.
//!
//! Implements the √N Cooley-Tukey six-step algorithm to achieve parallelism
//! with good locality. A global cache is used for twiddle factors.

use std::sync::{RwLock, RwLockReadGuard};

use snarkvm_fields::{FftField, FftParameters, Field};
#[cfg(any())]
use tracing::instrument;
#[cfg(not(feature = "serial"))]
use {crate::snark::provekit::whir::utils::workload_size, rayon::prelude::*, std::cmp::max};

use super::{
    ReedSolomon,
    transpose,
    utils::{lcm, sqrt_factor},
};
#[cfg(any())]
use crate::snark::provekit::whir::algebra::ntt::transpose::transpose_permute;
use crate::snark::provekit::whir::{
    algebra::ntt::utils::divisors,
    utils::{chunks_exact_or_empty, zip_strict},
};

// Supported primes
const PRIMES: [usize; 2] = [2, 3];

/// Engine for computing NTTs over arbitrary fields.
/// Assumes the field has large two-adicity.
#[derive(Debug)]
pub struct NttEngine<F: Field> {
    order: usize,         // order of omega_orger
    divisors: Vec<usize>, // divisors of the order.
    omega_order: F,       // primitive order'th root.

    // Roots of small order (zero if unavailable). The naming convention is that omega_foo has order foo.
    half_omega_3_1_plus_2: F, // ½(ω₃ + ω₃²)
    half_omega_3_1_min_2: F,  // ½(ω₃ - ω₃²)
    omega_4_1: F,
    omega_8_1: F,
    omega_8_3: F,
    omega_16_1: F,
    omega_16_3: F,
    omega_16_9: F,

    // Root lookup table (extended on demand)
    roots: RwLock<Vec<F>>,
}

impl<F: FftField> NttEngine<F> {
    /// Construct a new engine from the field's `FftField` trait.
    pub fn new_from_fftfield() -> Self {
        let (mut omega, mut order) = if let (Some(mut omega), Some(b), Some(k)) = (
            F::large_subgroup_root_of_unity(),
            F::FftParameters::SMALL_SUBGROUP_BASE,
            F::FftParameters::SMALL_SUBGROUP_BASE_ADICITY,
        ) {
            // Extract supported subgroup q from small group b.
            let mut order = 1;
            let mut remaining = (b as usize).checked_pow(k).expect("Small group too large.");
            for p in PRIMES {
                while remaining.is_multiple_of(p) {
                    order *= p;
                    remaining /= p;
                }
            }
            omega = omega.pow([remaining as u64]);
            (omega, order)
        } else {
            (F::two_adic_root_of_unity(), 1)
        };
        let twos = F::FftParameters::TWO_ADICITY.min(order.leading_zeros()) as usize;
        for _ in 0..(F::FftParameters::TWO_ADICITY as usize - twos) {
            omega.square_in_place();
        }
        order <<= twos;
        Self::new(order, omega)
    }
}

/// Creates a new NttEngine. `omega_order` must be a primitive root of unity of
/// even order `omega`.
impl<F: Field> NttEngine<F> {
    pub fn new(order: usize, omega_order: F) -> Self {
        // Make sure `omega_order` is a primitive root of unity.
        assert_eq!(omega_order.pow([order as u64]), F::one());
        for prime in PRIMES {
            if order.is_multiple_of(prime) {
                assert_ne!(omega_order.pow([(order / prime) as u64]), F::one());
            }
        }

        let mut res = Self {
            order,
            divisors: divisors(order, &PRIMES),
            omega_order,
            half_omega_3_1_plus_2: F::zero(),
            half_omega_3_1_min_2: F::zero(),
            omega_4_1: F::zero(),
            omega_8_1: F::zero(),
            omega_8_3: F::zero(),
            omega_16_1: F::zero(),
            omega_16_3: F::zero(),
            omega_16_9: F::zero(),
            roots: RwLock::new(Vec::new()),
        };
        if order.is_multiple_of(3) {
            let omega_3_1 = res.root(3);
            let omega_3_2 = omega_3_1 * omega_3_1;
            // Note: char F cannot be 2 and so division by 2 works, because primitive roots
            // of unity with even order exist.
            res.half_omega_3_1_min_2 = (omega_3_1 - omega_3_2) / F::from(2u64);
            res.half_omega_3_1_plus_2 = (omega_3_1 + omega_3_2) / F::from(2u64);
        }
        if order.is_multiple_of(4) {
            res.omega_4_1 = res.root(4);
        }
        if order.is_multiple_of(8) {
            res.omega_8_1 = res.root(8);
            res.omega_8_3 = res.omega_8_1.pow([3]);
        }
        if order.is_multiple_of(16) {
            res.omega_16_1 = res.root(16);
            res.omega_16_3 = res.omega_16_1.pow([3]);
            res.omega_16_9 = res.omega_16_1.pow([9]);
        }
        res
    }

    pub fn ntt(&self, values: &mut [F]) {
        self.ntt_batch(values, values.len());
    }

    pub fn ntt_batch(&self, values: &mut [F], size: usize) {
        assert!(values.len().is_multiple_of(size));
        if size <= 1 {
            return;
        }
        let roots = self.roots_table(size);
        self.ntt_dispatch(values, &roots, size);
    }

    /// Inverse NTT. Does not apply 1/n scaling factor.
    pub fn intt(&self, values: &mut [F]) {
        if values.len() <= 1 {
            return;
        }
        values[1..].reverse();
        self.ntt(values);
    }

    /// Inverse batch NTT. Does not apply 1/n scaling factor.
    pub fn intt_batch(&self, values: &mut [F], size: usize) {
        assert!(values.len().is_multiple_of(size));
        if size <= 1 {
            return;
        }

        #[cfg(feature = "serial")]
        values.chunks_exact_mut(size).for_each(|values| {
            values[1..].reverse();
        });

        #[cfg(not(feature = "serial"))]
        values.par_chunks_exact_mut(size).for_each(|values| {
            values[1..].reverse();
        });

        self.ntt_batch(values, size);
    }

    pub fn checked_root(&self, order: usize) -> Option<F> {
        if order == 0 {
            return Some(F::one());
        }
        self.order.is_multiple_of(order).then(|| self.omega_order.pow([(self.order / order) as u64]))
    }

    pub fn root(&self, order: usize) -> F {
        self.checked_root(order).expect("Subgroup of requested order does not exist.")
    }

    /// Returns a cached table of roots of unity of the given order.
    fn roots_table(&self, order: usize) -> RwLockReadGuard<'_, Vec<F>> {
        assert!(self.order.is_multiple_of(order), "No subgroup of order {order}.");

        // Precompute more roots of unity if requested.
        let roots = self.roots.read().unwrap();
        if roots.is_empty() || !roots.len().is_multiple_of(order) {
            // Obtain write lock to update the cache.
            drop(roots);
            let mut roots = self.roots.write().unwrap();
            // Race condition: check if another thread updated the cache.
            if roots.is_empty() || !roots.len().is_multiple_of(order) {
                // Compute minimal size to support all sizes seen so far.
                // TODO: Do we really need all of these? Can we leverage omege_2 = -1?
                let size = if roots.is_empty() { order } else { lcm(roots.len(), order) };
                roots.clear();
                roots.reserve_exact(size);

                // Compute powers of roots of unity.
                let root = self.root(size);
                #[cfg(feature = "serial")]
                {
                    let mut root_i = F::one();
                    for _ in 0..size {
                        roots.push(root_i);
                        root_i *= root;
                    }
                }
                #[cfg(not(feature = "serial"))]
                roots.par_extend((0..size).into_par_iter().map_with(F::zero(), |root_i, i| {
                    if root_i.is_zero() {
                        *root_i = root.pow([i as u64]);
                    } else {
                        *root_i *= root;
                    }
                    *root_i
                }));
            }
            // Back to read lock.
            drop(roots);
            self.roots.read().unwrap()
        } else {
            roots
        }
    }

    /// Compute NTTs in place by splititng into two factors.
    /// Recurses using the sqrt(N) Cooley-Tukey Six step NTT algorithm.
    fn ntt_recurse(&self, values: &mut [F], roots: &[F], size: usize) {
        debug_assert_eq!(values.len() % size, 0);
        let n1 = sqrt_factor(size); // TODO: Replace with divisors search.
        let n2 = size / n1;

        transpose(values, n1, n2);
        self.ntt_dispatch(values, roots, n1);
        transpose(values, n2, n1);
        // TODO: When (n1, n2) are coprime we can use the
        // Good-Thomas NTT algorithm and avoid the twiddle loop.
        apply_twiddles(values, roots, n1, n2);
        self.ntt_dispatch(values, roots, n2);
        transpose(values, n1, n2);
    }

    fn ntt_dispatch(&self, values: &mut [F], roots: &[F], size: usize) {
        debug_assert_eq!(values.len() % size, 0);
        debug_assert_eq!(roots.len() % size, 0);
        #[cfg(not(feature = "serial"))]
        if values.len() > workload_size::<F>() && values.len() != size {
            // Multiple NTTs, compute in parallel.
            // Work size is largest multiple of `size` smaller than `WORKLOAD_SIZE`.
            let workload_size = size * max(1, workload_size::<F>() / size);
            return values.par_chunks_mut(workload_size).for_each(|values| {
                self.ntt_dispatch(values, roots, size);
            });
        }
        match size {
            0 | 1 => {}
            2 => {
                for v in values.chunks_exact_mut(2) {
                    (v[0], v[1]) = (v[0] + v[1], v[0] - v[1]);
                }
            }
            3 => {
                for v in values.chunks_exact_mut(3) {
                    // Rader NTT to reduce 3 to 2.
                    let v0 = v[0];
                    (v[1], v[2]) = (v[1] + v[2], v[1] - v[2]);
                    v[0] += v[1];
                    v[1] *= self.half_omega_3_1_plus_2; // ½(ω₃ + ω₃²)
                    v[2] *= self.half_omega_3_1_min_2; // ½(ω₃ - ω₃²)
                    v[1] += v0;
                    (v[1], v[2]) = (v[1] + v[2], v[1] - v[2]);
                }
            }
            4 => {
                for v in values.chunks_exact_mut(4) {
                    (v[0], v[2]) = (v[0] + v[2], v[0] - v[2]);
                    (v[1], v[3]) = (v[1] + v[3], v[1] - v[3]);
                    v[3] *= self.omega_4_1;
                    (v[0], v[1]) = (v[0] + v[1], v[0] - v[1]);
                    (v[2], v[3]) = (v[2] + v[3], v[2] - v[3]);
                    (v[1], v[2]) = (v[2], v[1]);
                }
            }
            8 => {
                for v in values.chunks_exact_mut(8) {
                    // Cooley-Tukey with v as 2x4 matrix.
                    (v[0], v[4]) = (v[0] + v[4], v[0] - v[4]);
                    (v[1], v[5]) = (v[1] + v[5], v[1] - v[5]);
                    (v[2], v[6]) = (v[2] + v[6], v[2] - v[6]);
                    (v[3], v[7]) = (v[3] + v[7], v[3] - v[7]);
                    v[5] *= self.omega_8_1;
                    v[6] *= self.omega_4_1; // == omega_8_2
                    v[7] *= self.omega_8_3;
                    (v[0], v[2]) = (v[0] + v[2], v[0] - v[2]);
                    (v[1], v[3]) = (v[1] + v[3], v[1] - v[3]);
                    v[3] *= self.omega_4_1;
                    (v[0], v[1]) = (v[0] + v[1], v[0] - v[1]);
                    (v[2], v[3]) = (v[2] + v[3], v[2] - v[3]);
                    (v[4], v[6]) = (v[4] + v[6], v[4] - v[6]);
                    (v[5], v[7]) = (v[5] + v[7], v[5] - v[7]);
                    v[7] *= self.omega_4_1;
                    (v[4], v[5]) = (v[4] + v[5], v[4] - v[5]);
                    (v[6], v[7]) = (v[6] + v[7], v[6] - v[7]);
                    (v[1], v[4]) = (v[4], v[1]);
                    (v[3], v[6]) = (v[6], v[3]);
                }
            }
            16 => {
                for v in values.chunks_exact_mut(16) {
                    // Cooley-Tukey with v as 4x4 matrix.
                    for i in 0..4 {
                        let v = &mut v[i..];
                        (v[0], v[8]) = (v[0] + v[8], v[0] - v[8]);
                        (v[4], v[12]) = (v[4] + v[12], v[4] - v[12]);
                        v[12] *= self.omega_4_1;
                        (v[0], v[4]) = (v[0] + v[4], v[0] - v[4]);
                        (v[8], v[12]) = (v[8] + v[12], v[8] - v[12]);
                        (v[4], v[8]) = (v[8], v[4]);
                    }
                    v[5] *= self.omega_16_1;
                    v[6] *= self.omega_8_1;
                    v[7] *= self.omega_16_3;
                    v[9] *= self.omega_8_1;
                    v[10] *= self.omega_4_1;
                    v[11] *= self.omega_8_3;
                    v[13] *= self.omega_16_3;
                    v[14] *= self.omega_8_3;
                    v[15] *= self.omega_16_9;
                    for i in 0..4 {
                        let v = &mut v[i * 4..];
                        (v[0], v[2]) = (v[0] + v[2], v[0] - v[2]);
                        (v[1], v[3]) = (v[1] + v[3], v[1] - v[3]);
                        v[3] *= self.omega_4_1;
                        (v[0], v[1]) = (v[0] + v[1], v[0] - v[1]);
                        (v[2], v[3]) = (v[2] + v[3], v[2] - v[3]);
                        (v[1], v[2]) = (v[2], v[1]);
                    }
                    (v[1], v[4]) = (v[4], v[1]);
                    (v[2], v[8]) = (v[8], v[2]);
                    (v[3], v[12]) = (v[12], v[3]);
                    (v[6], v[9]) = (v[9], v[6]);
                    (v[7], v[13]) = (v[13], v[7]);
                    (v[11], v[14]) = (v[14], v[11]);
                }
            }
            size => self.ntt_recurse(values, roots, size),
        }
    }
}

impl<F: Field> ReedSolomon<F> for NttEngine<F> {
    fn next_order(&self, size: usize) -> Option<usize> {
        match self.divisors.binary_search(&size) {
            Ok(index) | Err(index) => self.divisors.get(index).copied(),
        }
    }

    fn generator(&self, codeword_length: usize) -> F {
        self.omega_order.pow([(self.order / codeword_length) as u64])
    }

    fn evaluation_points(&self, masked_message_length: usize, codeword_length: usize, indices: &[usize]) -> Vec<F> {
        assert!(masked_message_length <= codeword_length);
        assert!(self.order.is_multiple_of(codeword_length));
        let mut result = Vec::new();
        let generator = self.generator(codeword_length);

        // Coset transformation
        let mut coset_size = self.next_order(masked_message_length).unwrap();
        while !codeword_length.is_multiple_of(coset_size) {
            coset_size = self.next_order(coset_size + 1).unwrap();
        }
        #[cfg(any())]
        let num_cosets = codeword_length / coset_size;

        for &index in indices {
            assert!(index < codeword_length);

            #[cfg(any())]
            let index = transpose_permute(index, num_cosets, coset_size);
            result.push(generator.pow([index as u64]));
        }
        result
    }

    #[cfg_attr(any(), instrument(skip(self, messages, masks), fields(
        num_messages = messages.len(),
        message_len = messages.first().map(|c| c.len()),
        codeword_length = codeword_length,
        mask_len = masks.len().checked_div(messages.len())

    )))]
    fn interleaved_encode(&self, messages: &[&[F]], masks: &[F], codeword_length: usize) -> Vec<F> {
        assert!(self.order.is_multiple_of(codeword_length));
        if messages.is_empty() {
            assert!(masks.is_empty());
            return Vec::new();
        }
        let num_messages = messages.len();
        let message_len = messages[0].len();
        assert!(messages.iter().all(|m| m.len() == message_len));
        assert!(masks.len().is_multiple_of(num_messages));
        let mask_length = masks.len() / num_messages;
        let masked_message_length = message_len + mask_length;
        assert!(masked_message_length <= codeword_length);

        // Coset-NTT: instead of doing one codeword-length NTT on mostly zeros,
        // do `num_cosets` many `coset_size`-point NTTs on twisted coefficient
        // vectors. For coset `c`, we evaluate on points
        //
        //     ω_N^{c + j * num_cosets} = ω_N^c · (ω_N^{num_cosets})^j
        //
        // so the coefficient of X^i must be multiplied by (ω_N^c)^i.
        //
        // You can also see this as applying a first round of Cooley-Tukey with
        // N = coset_size × num_cosets, and solving it directly by observing that
        // only the first coset is non-zero.
        let mut coset_size = self.next_order(masked_message_length).unwrap();
        while !codeword_length.is_multiple_of(coset_size) {
            coset_size = self.next_order(coset_size + 1).unwrap();
        }
        let num_cosets = codeword_length / coset_size;
        let coset_padding = coset_size - masked_message_length;

        // Lay out twisted coefficients in contiguous coset blocks of length
        // `coset_size`, zero-padding each block as needed.
        let mut result = Vec::with_capacity(num_messages * codeword_length);
        for (message, mask) in zip_strict(messages, chunks_exact_or_empty(masks, mask_length, num_messages)) {
            // FFT[a 0 0 0] = [a a a a], so just replicate input in coset dimension.
            for _ in 0..num_cosets {
                result.extend_from_slice(message);
                result.extend_from_slice(mask);
                result.resize(result.len() + coset_padding, F::zero());
            }
        }
        assert_eq!(result.len(), num_messages * codeword_length);

        // NTT each coset block, then transpose each codeword block from
        // coset-major `(num_cosets × coset_size)` layout into standard codeword
        // order `(coset_size × num_cosets)`, where global index is
        // `c + j * num_cosets`.
        apply_twiddles(&mut result, self.roots_table(codeword_length).as_slice(), num_cosets, coset_size);
        self.ntt_batch(&mut result, coset_size);

        transpose(&mut result, num_cosets, coset_size);

        // Transpose to row-major order with vectors stacked horizontally.
        transpose(&mut result, num_messages, codeword_length);
        result
    }
}

/// Applies twiddle factors to a slice of field elements in-place.
///
/// This is part of the six-step Cooley-Tukey NTT algorithm,
/// where after transposing and partially transforming a 2D matrix,
/// we multiply each non-zero row and column entry by a scalar "twiddle factor"
/// derived from powers of a root of unity.
///
/// Given:
/// - `values`: a flattened set of NTT matrices, each with shape `[rows × cols]`
/// - `roots`: the root-of-unity table (should have length divisible by `rows *
///   cols`)
/// - `rows`, `cols`: the dimensions of each matrix
///
/// This function mutates `values` in-place, applying twiddle factors like so:
///
/// ```text
/// values[i][j] *= roots[(i * step + j * step) % roots.len()]
/// ```
///
/// More specifically:
/// - The first row and column are left untouched (twiddle factor is 1).
/// - For each row `i > 0`, each element `values[i][j > 0]` is multiplied by a
///   twiddle factor.
/// - The factor is taken as `roots[index]`, where:
///   - `index` starts at `step = (i * roots.len()) / (rows * cols)`
///   - `index` increments by `step` for each column.
///
/// ### Parallelism
/// - If `parallel` is enabled and `values.len()` exceeds a threshold:
///   - Large matrices are split into workloads and processed in parallel.
///   - If a single matrix is present, its rows are parallelized directly.
///
/// ### Panics
/// - If `values.len() % (rows * cols) != 0`
/// - If `roots.len()` is not divisible by `rows * cols`
///
/// ### Example
/// Suppose you have a `2×4` matrix:
/// ```text
/// [ a0 a1 a2 a3 ]
/// [ b0 b1 b2 b3 ]
/// ```
/// and `roots = [r0, r1, ..., rN]`, then the transformed matrix becomes:
/// ```text
/// [ a0  a1       a2       a3       ]
/// [ b0  b1*rX1   b2*rX2   b3*rX3   ]
/// ```
/// where `rX1`, `rX2`, etc., are powers of root-of-unity determined by row/col
/// indices.
pub fn apply_twiddles<F: Field>(values: &mut [F], roots: &[F], rows: usize, cols: usize) {
    let size = rows * cols;
    debug_assert_eq!(values.len() % size, 0);
    let step = roots.len() / size;

    #[cfg(not(feature = "serial"))]
    {
        if values.len() > workload_size::<F>() {
            if values.len() == size {
                // Only one matrix → parallelize rows directly
                values.par_chunks_exact_mut(cols).enumerate().skip(1).for_each(|(i, row)| {
                    let step = (i * step) % roots.len();
                    let mut index = step;
                    for value in row.iter_mut().skip(1) {
                        index %= roots.len();
                        *value *= roots[index];
                        index += step;
                    }
                });
                return;
            }
            // Multiple matrices → chunk and recurse
            let workload_size = size * max(1, workload_size::<F>() / size);
            values.par_chunks_mut(workload_size).for_each(|chunk| apply_twiddles(chunk, roots, rows, cols));
            return;
        }
    }

    // Fallback (non-parallel or small workload)
    for matrix in values.chunks_exact_mut(size) {
        for (i, row) in matrix.chunks_exact_mut(cols).enumerate().skip(1) {
            let step = (i * step) % roots.len();
            let mut index = step;
            for value in row.iter_mut().skip(1) {
                index %= roots.len();
                *value *= roots[index];
                index += step;
            }
        }
    }
}

#[cfg(any())]
#[allow(clippy::significant_drop_tightening)]
mod tests {
    use snarkvm_fields::{Field, One, PrimeField, Zero};

    use super::*;
    use crate::snark::provekit::whir::algebra::fields::Field64;

    #[test]
    fn test_new_from_fftfield_basic() {
        // Ensure that an engine is created correctly from FFT field properties
        let engine = NttEngine::<Field64>::new_from_fftfield();

        // Verify that the order of the engine is correctly set
        assert_eq!(engine.order, 3 << 32);

        // Verify that the root of unity is correctly initialized
        assert_eq!(engine.omega_order, Field64::GENERATOR.pow([(18_446_744_069_414_584_320 / engine.order) as u64]));
    }

    #[test]
    fn test_root_computation() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        // Ensure the root exponentiates correctly
        assert_eq!(engine.root(8).pow([8]), Field64::ONE);
        assert_eq!(engine.root(4).pow([4]), Field64::ONE);
        assert_eq!(engine.root(2).pow([2]), Field64::ONE);

        // Ensure it's not a lower-order root
        assert_ne!(engine.root(8).pow([4]), Field64::ONE);
        assert_ne!(engine.root(4).pow([2]), Field64::ONE);
    }

    #[test]
    fn test_root_of_unity_multiplication() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        let root = engine.root(16);

        // Multiply root by itself repeatedly and verify expected outcomes
        assert_eq!(root.pow([2]), engine.root(8));
        assert_eq!(root.pow([4]), engine.root(4));
        assert_eq!(root.pow([8]), engine.root(2));
    }

    #[test]
    fn test_root_of_unity_inversion() {
        let engine = NttEngine::<Field64>::new_from_fftfield();
        let root = engine.root(16);

        // The inverse of ω is ω^{-1}, computed as ω^(p-2) in Field64.
        let p: u64 = u64::from_be_bytes(Field64::MODULUS.to_bytes_be().try_into().unwrap());
        let inverse_root = root.pow([p - 2]);
        assert_eq!(root * inverse_root, Field64::ONE);
    }

    #[test]
    fn test_precomputed_small_roots() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        // Check that precomputed values are correctly initialized
        assert_eq!(engine.omega_8_1.pow([8]), Field64::ONE);
        assert_eq!(engine.omega_8_3.pow([8]), Field64::ONE);
        assert_eq!(engine.omega_16_1.pow([16]), Field64::ONE);
        assert_eq!(engine.omega_16_3.pow([16]), Field64::ONE);
        assert_eq!(engine.omega_16_9.pow([16]), Field64::ONE);
    }

    #[test]
    fn test_consistency_across_multiple_instances() {
        let engine1 = NttEngine::<Field64>::new_from_fftfield();
        let engine2 = NttEngine::<Field64>::new_from_fftfield();

        // Ensure that multiple instances yield the same results
        assert_eq!(engine1.root(8), engine2.root(8));
        assert_eq!(engine1.root(4), engine2.root(4));
        assert_eq!(engine1.root(2), engine2.root(2));
    }

    #[test]
    fn test_roots_table_basic() {
        let engine = NttEngine::<Field64>::new_from_fftfield();
        let roots_4 = engine.roots_table(4);

        // Check hardcoded expected values (ω^i)
        assert_eq!(roots_4[0], Field::ONE);
        assert_eq!(roots_4[1], engine.root(4));
        assert_eq!(roots_4[2], engine.root(4).pow([2]));
        assert_eq!(roots_4[3], engine.root(4).pow([3]));
    }

    #[test]
    fn test_roots_table_minimal_order() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        let roots_2 = engine.roots_table(2);

        // Must contain only ω^0 and ω^1
        assert_eq!(roots_2.len(), 2);
        assert_eq!(roots_2[0], Field64::ONE);
        assert_eq!(roots_2[1], engine.root(2));
    }

    #[test]
    fn test_roots_table_progression() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        let roots_4 = engine.roots_table(4);

        // Ensure the sequence follows expected powers of the root of unity
        for i in 0..4 {
            assert_eq!(roots_4[i], engine.root(4).pow([i as u64]));
        }
    }

    #[test]
    fn test_roots_table_cached_results() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        let first_access = engine.roots_table(4);
        let second_access = engine.roots_table(4);

        // The memory location should be the same, meaning it's cached
        assert!(std::ptr::eq(first_access.as_ptr(), second_access.as_ptr()));
    }

    #[test]
    fn test_roots_table_recompute_factor_order() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        let roots_4 = engine.roots_table(4);
        let roots_2 = engine.roots_table(2);

        // Ensure first two elements of roots_4 match the first two elements of roots_2
        assert_eq!(&roots_4[..2], &roots_2[..2]);
    }

    #[test]
    fn test_apply_twiddles_basic() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        let mut values = vec![
            Field64::from(1),
            Field64::from(2),
            Field64::from(3),
            Field64::from(4),
            Field64::from(5),
            Field64::from(6),
            Field64::from(7),
            Field64::from(8),
        ];

        // Mock roots
        let r1 = Field64::from(33);
        let roots = vec![r1];

        // Ensure the root of unity is correct
        assert_eq!(engine.root(4).pow([4]), Field64::ONE);

        apply_twiddles(&mut values, &roots, 2, 4);

        // The first row should remain unchanged
        assert_eq!(values[0], Field64::from(1));
        assert_eq!(values[1], Field64::from(2));
        assert_eq!(values[2], Field64::from(3));
        assert_eq!(values[3], Field64::from(4));

        // The second row should be multiplied by the correct twiddle factors
        assert_eq!(values[4], Field64::from(5)); // No change for first column
        assert_eq!(values[5], Field64::from(6) * r1);
        assert_eq!(values[6], Field64::from(7) * r1);
        assert_eq!(values[7], Field64::from(8) * r1);
    }

    #[test]
    fn test_apply_twiddles_single_row() {
        let mut values = vec![Field64::from(1), Field64::from(2)];

        // Mock roots
        let r1 = Field64::from(12);
        let roots = vec![r1];

        apply_twiddles(&mut values, &roots, 1, 2);

        // Everything should remain unchanged
        assert_eq!(values[0], Field64::from(1));
        assert_eq!(values[1], Field64::from(2));
    }

    #[test]
    fn test_apply_twiddles_varying_rows() {
        let mut values = vec![
            Field64::from(1),
            Field64::from(2),
            Field64::from(3),
            Field64::from(4),
            Field64::from(5),
            Field64::from(6),
            Field64::from(7),
            Field64::from(8),
            Field64::from(9),
        ];

        // Mock roots
        let roots = (2..100).map(Field64::from).collect::<Vec<_>>();

        apply_twiddles(&mut values, &roots, 3, 3);

        // First row remains unchanged
        assert_eq!(values[0], Field64::from(1));
        assert_eq!(values[1], Field64::from(2));
        assert_eq!(values[2], Field64::from(3));

        // Second row multiplied by twiddle factors
        assert_eq!(values[3], Field64::from(4));
        assert_eq!(values[4], Field64::from(5) * roots[10]);
        assert_eq!(values[5], Field64::from(6) * roots[20]);

        // Third row multiplied by twiddle factors
        assert_eq!(values[6], Field64::from(7));
        assert_eq!(values[7], Field64::from(8) * roots[20]);
        assert_eq!(values[8], Field64::from(9) * roots[40]);
    }

    #[test]
    fn test_apply_twiddles_large_table() {
        let rows = 320;
        let cols = 320;
        let size = rows * cols;

        let mut values: Vec<Field64> = (0..size as u64).map(Field64::from).collect();

        // Generate a large set of twiddle factors
        let roots: Vec<Field64> = (0..(size * 2) as u64).map(Field64::from).collect();

        apply_twiddles(&mut values, &roots, rows, cols);

        // Verify the first row remains unchanged
        for (i, &col) in values.iter().enumerate().take(cols) {
            assert_eq!(col, Field64::from(i as u64));
        }

        // Verify the first column remains unchanged
        for row in 1..rows {
            let index = row * cols;
            assert_eq!(values[index], Field64::from(index as u64), "Mismatch in first column at row={row}");
        }

        // Verify that other rows have been modified using the twiddle factors
        for row in 1..rows {
            let mut idx = row * 2;
            for col in 1..cols {
                let index = row * cols + col;
                let expected = Field64::from(index as u64) * roots[idx];
                assert_eq!(values[index], expected, "Mismatch at row={row}, col={col}");
                idx += 2 * row;
            }
        }
    }

    #[test]
    fn test_ntt_batch_size_2() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        // Input values: f(x) = [1, 2]
        let f0 = Field64::from(1);
        let f1 = Field64::from(2);
        let mut values = vec![f0, f1];

        // Compute the expected NTT manually:
        //
        //   F(0)  =  f0 + f1
        //   F(1)  =  f0 - f1
        //
        // ω is the 2nd root of unity: ω² = 1.

        let expected_f0 = f0 + f1;
        let expected_f1 = f0 - f1;

        let expected_values = vec![expected_f0, expected_f1];

        engine.ntt_batch(&mut values, 2);

        assert_eq!(values, expected_values);
    }

    #[test]
    fn test_ntt_batch_size_4() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        // Input values: f(x) = [1, 2, 3, 4]
        let f0 = Field64::from(1);
        let f1 = Field64::from(2);
        let f2 = Field64::from(3);
        let f3 = Field64::from(4);
        let mut values = vec![f0, f1, f2, f3];

        // Compute the expected NTT manually:
        //
        //   F(0)  =  f0 + f1 + f2 + f3
        //   F(1)  =  f0 + f1 * ω + f2 * ω² + f3 * ω³
        //   F(2)  =  f0 + f1 * ω² + f2 * ω⁴ + f3 * ω⁶
        //   F(3)  =  f0 + f1 * ω³ + f2 * ω⁶ + f3 * ω⁹
        //
        // ω is the 4th root of unity: ω⁴ = 1, ω² = -1.

        let omega = engine.omega_4_1;
        let omega1 = omega; // ω
        let omega2 = omega * omega; // ω² = -1
        let omega3 = omega * omega2; // ω³ = -ω
        let omega4 = omega * omega3; // ω⁴ = 1

        let expected_f0 = f0 + f1 + f2 + f3;
        let expected_f1 = f0 + f1 * omega1 + f2 * omega2 + f3 * omega3;
        let expected_f2 = f0 + f1 * omega2 + f2 * omega4 + f3 * omega2;
        let expected_f3 = f0 + f1 * omega3 + f2 * omega2 + f3 * omega1;

        let expected_values = vec![expected_f0, expected_f1, expected_f2, expected_f3];

        engine.ntt_batch(&mut values, 4);

        assert_eq!(values, expected_values);
    }

    #[test]
    fn test_ntt_batch_size_8() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        // Input values: f(x) = [1, 2, 3, 4, 5, 6, 7, 8]
        let f0 = Field64::from(1);
        let f1 = Field64::from(2);
        let f2 = Field64::from(3);
        let f3 = Field64::from(4);
        let f4 = Field64::from(5);
        let f5 = Field64::from(6);
        let f6 = Field64::from(7);
        let f7 = Field64::from(8);
        let mut values = vec![f0, f1, f2, f3, f4, f5, f6, f7];

        // Compute the expected NTT manually:
        //
        //   F(k) = ∑ f_j * ω^(j*k)  for k ∈ {0, ..., 7}
        //
        // ω is the 8th root of unity: ω⁸ = 1.

        let omega = engine.omega_8_1; // ω
        let omega1 = omega; // ω
        let omega2 = omega * omega; // ω²
        let omega3 = omega * omega2; // ω³
        let omega4 = omega * omega3; // ω⁴
        let omega5 = omega * omega4; // ω⁵
        let omega6 = omega * omega5; // ω⁶
        let omega7 = omega * omega6; // ω⁷

        let expected_f0 = f0 + f1 + f2 + f3 + f4 + f5 + f6 + f7;
        let expected_f1 =
            f0 + f1 * omega1 + f2 * omega2 + f3 * omega3 + f4 * omega4 + f5 * omega5 + f6 * omega6 + f7 * omega7;
        let expected_f2 =
            f0 + f1 * omega2 + f2 * omega4 + f3 * omega6 + f4 * Field64::ONE + f5 * omega2 + f6 * omega4 + f7 * omega6;
        let expected_f3 =
            f0 + f1 * omega3 + f2 * omega6 + f3 * omega1 + f4 * omega4 + f5 * omega7 + f6 * omega2 + f7 * omega5;
        let expected_f4 = f0
            + f1 * omega4
            + f2 * Field64::ONE
            + f3 * omega4
            + f4 * Field64::ONE
            + f5 * omega4
            + f6 * Field64::ONE
            + f7 * omega4;
        let expected_f5 =
            f0 + f1 * omega5 + f2 * omega2 + f3 * omega7 + f4 * omega4 + f5 * omega1 + f6 * omega6 + f7 * omega3;
        let expected_f6 =
            f0 + f1 * omega6 + f2 * omega4 + f3 * omega2 + f4 * Field64::ONE + f5 * omega6 + f6 * omega4 + f7 * omega2;
        let expected_f7 =
            f0 + f1 * omega7 + f2 * omega6 + f3 * omega5 + f4 * omega4 + f5 * omega3 + f6 * omega2 + f7 * omega1;

        let expected_values = vec![
            expected_f0,
            expected_f1,
            expected_f2,
            expected_f3,
            expected_f4,
            expected_f5,
            expected_f6,
            expected_f7,
        ];

        engine.ntt_batch(&mut values, 8);

        assert_eq!(values, expected_values);
    }

    #[test]
    fn test_ntt_batch_size_16() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        // Input values: f(x) = [1, 2, ..., 16]
        let values: Vec<_> = (1..=16).map(Field64::from).collect();
        let mut values_ntt = values.clone();

        // Compute the expected NTT manually:
        //
        //   F(k) = ∑ f_j * ω^(j*k)  for k ∈ {0, ..., 15}
        //
        // ω is the 16th root of unity: ω¹⁶ = 1.

        let omega = engine.omega_16_1; // ω
        let mut expected_values = vec![Field64::ZERO; 16];
        for (k, expected_value) in expected_values.iter_mut().enumerate().take(16) {
            let omega_k = omega.pow([k as u64]);
            *expected_value = values.iter().enumerate().map(|(j, &f_j)| f_j * omega_k.pow([j as u64])).sum();
        }

        engine.ntt_batch(&mut values_ntt, 16);

        assert_eq!(values_ntt, expected_values);
    }

    #[test]
    fn test_ntt_batch_size_32() {
        let engine = NttEngine::<Field64>::new_from_fftfield();

        // Input values: f(x) = [1, 2, ..., 32]
        let values: Vec<_> = (1..=32).map(Field64::from).collect();
        let mut values_ntt = values.clone();

        // Compute the expected NTT manually:
        //
        //   F(k) = ∑ f_j * ω^(j*k)  for k ∈ {0, ..., 31}
        //
        // ω is the 32nd root of unity: ω³² = 1.

        let omega = engine.root(32);
        let mut expected_values = vec![Field64::ZERO; 32];
        for (k, expected_value) in expected_values.iter_mut().enumerate().take(32) {
            let omega_k = omega.pow([k as u64]);
            *expected_value = values.iter().enumerate().map(|(j, &f_j)| f_j * omega_k.pow([j as u64])).sum();
        }

        engine.ntt_batch(&mut values_ntt, 32);

        assert_eq!(values_ntt, expected_values);
    }
}
