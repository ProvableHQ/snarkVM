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

use snarkvm_curves::{AffineCurve, ProjectiveCurve};
use snarkvm_fields::{Field, One, PrimeField, Zero};
use snarkvm_utilities::{BigInteger, BitIteratorBE, cfg_into_iter};

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

#[cfg(target_arch = "x86_64")]
use crate::{prefetch_slice, prefetch_slice_write};

// Thread-local scratch buffer for the counting sort output in `batch_add`.
// Reusing a pre-allocated Vec across calls on the same thread eliminates
// both the per-call allocation (O(n) malloc + OS page-fault) and the
// sentinel zero-initialization (O(n) memset) since the scatter loop
// overwrites every slot exactly once. With 20 windows per MSM and ~11 MSMs
// per proof, this saves ~110 MB of allocation+init work per proof.
thread_local! {
    static SORT_SCRATCH: std::cell::RefCell<Vec<BucketPosition>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[derive(Copy, Clone, Debug)]
pub struct BucketPosition {
    pub bucket_index: u32,
    pub scalar_index: u32,
}

impl Eq for BucketPosition {}

impl PartialEq for BucketPosition {
    fn eq(&self, other: &Self) -> bool {
        self.bucket_index == other.bucket_index
    }
}

impl Ord for BucketPosition {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.bucket_index.cmp(&other.bucket_index)
    }
}

impl PartialOrd for BucketPosition {
    #[allow(clippy::non_canonical_partial_ord_impl)]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.bucket_index.partial_cmp(&other.bucket_index)
    }
}

/// Returns a batch size of sufficient size to amortize the cost of an
/// inversion, while attempting to reduce strain to the CPU cache.
#[inline]
const fn batch_size(msm_size: usize) -> usize {
    // These values are determined empirically using performance benchmarks for
    // BLS12-377 on Intel, AMD, and M1 machines. These values are determined by
    // taking the L1 and L2 cache sizes and dividing them by the size of group
    // elements (i.e. 96 bytes).
    //
    // As the algorithm itself requires caching additional values beyond the group
    // elements, the ideal batch size is less than expected, to accommodate
    // those values. In general, it was found that undershooting is better than
    // overshooting this heuristic.
    if cfg!(target_arch = "x86_64") && msm_size < 500_000 {
        // Tuned for an L1D cache of 64KiB: 600 affinep points × 96 bytes +
        // scratch_space of 300 × 96 bytes ≈ 57.6KiB, which fits in a 64KiB
        // L1D cache. Machines with smaller L1 caches (e.g., 32KiB) should use
        // 300; the larger value reduces the number of batch inversions per MSM.
        600
    } else {
        // Tuned for an L2 cache of 2MiB: 6000 affine points × 96 bytes +
        // scratch_space of 3000 × 96 bytes ≈ 864KiB, which fits in a 2MiB L2
        // cache. Machines with smaller L2 caches (e.g., 1MiB) should use 3000.
        6000
    }
}

/// If `(j, k)` is the `i`-th entry in `index`, then this method sets
/// `bases[j] = bases[j] + bases[k]`. The state of `bases[k]` becomes
/// unspecified.
#[inline]
fn batch_add_in_place_same_slice<G: AffineCurve>(bases: &mut [G], index: &[(u32, u32)]) {
    let mut inversion_tmp = G::BaseField::one();
    let half = G::BaseField::half();

    #[cfg(target_arch = "x86_64")]
    let mut prefetch_iter = index.iter();
    #[cfg(target_arch = "x86_64")]
    prefetch_iter.next();

    // We run two loops over the data separated by an inversion
    for (idx, idy) in index.iter() {
        #[cfg(target_arch = "x86_64")]
        prefetch_slice!(G, bases, bases, prefetch_iter);

        let (a, b) = if idx < idy {
            let (x, y) = bases.split_at_mut(*idy as usize);
            (&mut x[*idx as usize], &mut y[0])
        } else {
            let (x, y) = bases.split_at_mut(*idx as usize);
            (&mut y[0], &mut x[*idy as usize])
        };
        G::batch_add_loop_1(a, b, &half, &mut inversion_tmp);
    }

    inversion_tmp = inversion_tmp.inverse().unwrap(); // this is always in Fp*

    #[cfg(target_arch = "x86_64")]
    let mut prefetch_iter = index.iter().rev();
    #[cfg(target_arch = "x86_64")]
    prefetch_iter.next();

    for (idx, idy) in index.iter().rev() {
        #[cfg(target_arch = "x86_64")]
        prefetch_slice!(G, bases, bases, prefetch_iter);

        let (a, b) = if idx < idy {
            let (x, y) = bases.split_at_mut(*idy as usize);
            (&mut x[*idx as usize], y[0])
        } else {
            let (x, y) = bases.split_at_mut(*idx as usize);
            (&mut y[0], x[*idy as usize])
        };
        G::batch_add_loop_2(a, b, &mut inversion_tmp);
    }
}

/// If `(j, k)` is the `i`-th entry in `index`, then this method performs one of
/// two actions:
/// * `addition_result[i] = bases[j] + bases[k]`
/// * `addition_result[i] = bases[j];
///
/// It uses `scratch_space` to store intermediate values, and clears it after
/// use.
#[inline]
fn batch_add_write<G: AffineCurve>(
    bases: &[G],
    index: &[(u32, u32)],
    addition_result: &mut Vec<G>,
    scratch_space: &mut Vec<Option<G>>,
) {
    let mut inversion_tmp = G::BaseField::one();
    let half = G::BaseField::half();

    #[cfg(target_arch = "x86_64")]
    let mut prefetch_iter = index.iter();
    #[cfg(target_arch = "x86_64")]
    prefetch_iter.next();

    // We run two loops over the data separated by an inversion
    for (idx, idy) in index.iter() {
        #[cfg(target_arch = "x86_64")]
        prefetch_slice_write!(G, bases, bases, prefetch_iter);

        if *idy == !0u32 {
            addition_result.push(bases[*idx as usize]);
            scratch_space.push(None);
        } else {
            let (mut a, mut b) = (bases[*idx as usize], bases[*idy as usize]);
            G::batch_add_loop_1(&mut a, &mut b, &half, &mut inversion_tmp);
            addition_result.push(a);
            scratch_space.push(Some(b));
        }
    }

    inversion_tmp = inversion_tmp.inverse().unwrap(); // this is always in Fp*

    for (a, op_b) in addition_result.iter_mut().rev().zip(scratch_space.iter().rev()) {
        if let Some(b) = op_b {
            G::batch_add_loop_2(a, *b, &mut inversion_tmp);
        }
    }
    scratch_space.clear();
}

#[inline]
pub(super) fn batch_add<G: AffineCurve>(
    num_buckets: usize,
    bases: &[G],
    bucket_positions: &mut Vec<BucketPosition>,
) -> (usize, Vec<G>) {
    // Returns `(num_scalars, new_bases)` where `bucket_positions[..num_scalars]`
    // contains the final sorted bucket assignments and `new_bases[i]` is the
    // accumulated affine point for assignment `i`. The caller can Horner-
    // accumulate directly from this sparse representation, avoiding the
    // `vec![Zero::zero(); num_buckets]` scatter buffer (~196 KiB for c=11).
    assert!(bases.len() >= bucket_positions.len());
    assert!(!bases.is_empty());

    // Fetch the ideal batch size for the number of bases.
    let batch_size = batch_size(bases.len());

    // Counting sort by bucket_index (bounded integer in [0, num_buckets-1], or
    // u32::MAX for "skip" entries where the scalar's window bits are zero).
    // Counting sort is O(n + num_buckets) vs O(n log n) for sort_unstable,
    // saving ~90% of sort cost at typical MSM sizes (n ≈ 65k, num_buckets ≈ 8k).
    // Skip entries (bucket_index ≥ num_buckets) sort last, matching sort_unstable.
    //
    // Optimisations:
    // 1. Fuse `counts` and `starts` into a single `starts` array (one fewer 32 KiB
    //    allocation) via the shifted-histogram trick.
    // 2. Thread-local scratch buffer (SORT_SCRATCH) reused across calls so that no
    //    allocation or zero-initialization occurs after the first call on each
    //    rayon thread. The scatter loop overwrites every slot exactly once, so
    //    leftover data from the previous call is safe to ignore.
    // 3. std::mem::swap to hand the sorted buffer to `bucket_positions` in O(1).
    {
        let n = bucket_positions.len();
        // Single counts+starts array: starts[i] accumulates count of bucket i,
        // then is converted to start position via prefix-sum.
        let mut starts = vec![0u32; num_buckets];
        for pos in bucket_positions.iter() {
            let idx = pos.bucket_index as usize;
            if idx < num_buckets {
                starts[idx] += 1;
            }
            // Skip entries (bucket_index >= num_buckets) are not counted here.
        }
        // Prefix-sum in place: starts[i] becomes the start position of bucket i.
        let mut cumsum = 0u32;
        for s in starts.iter_mut() {
            let cnt = *s;
            *s = cumsum;
            cumsum += cnt;
        }
        // cumsum == number of non-skip entries; skip entries occupy [cumsum .. n).
        let skip_start = cumsum as usize;
        // Scatter into the thread-local scratch buffer. The buffer is reused
        // across calls on the same rayon thread, so no allocation or
        // zero-initialization occurs once the buffer has been warmed up.
        // resize() is O(1) when len >= n (no write), O(n-len) on first call.
        // After the first window per thread the buffer stays at capacity n and
        // all subsequent resize() calls are no-ops.
        SORT_SCRATCH.with(|cell| {
            let mut sorted = cell.borrow_mut();
            // Sentinel value used only on first call (warm-up). Subsequent
            // calls find len == n and resize() is a no-op. Every position
            // in [0..n] is overwritten by the scatter loop below.
            sorted.resize(n, BucketPosition { bucket_index: u32::MAX, scalar_index: 0 });
            let mut cursors = starts;
            let mut skip_cur = skip_start;
            for pos in bucket_positions.iter() {
                let idx = pos.bucket_index as usize;
                if idx < num_buckets {
                    let out_idx = cursors[idx] as usize;
                    sorted[out_idx] = *pos;
                    cursors[idx] += 1;
                } else {
                    sorted[skip_cur] = *pos;
                    skip_cur += 1;
                }
            }
            // Swap the sorted buffer into bucket_positions in O(1).
            // After the swap: bucket_positions holds sorted data,
            // sorted holds the old unsorted data (reused next call).
            std::mem::swap(bucket_positions, &mut *sorted);
        });
    }

    let mut num_scalars = bucket_positions.len();
    let mut all_ones = true;
    let mut new_scalar_length = 0;
    let mut global_counter = 0;
    let mut local_counter = 1;
    let mut number_of_bases_in_batch = 0;

    let mut instr = Vec::<(u32, u32)>::with_capacity(batch_size);
    let mut new_bases = Vec::with_capacity(bases.len());
    let mut scratch_space = Vec::with_capacity(batch_size / 2);

    // In the first loop, copy the results of the first in-place addition tree to
    // the vector `new_bases`.
    while global_counter < num_scalars {
        let current_bucket = bucket_positions[global_counter].bucket_index;
        while global_counter + 1 < num_scalars && bucket_positions[global_counter + 1].bucket_index == current_bucket {
            global_counter += 1;
            local_counter += 1;
        }
        if current_bucket >= num_buckets as u32 {
            local_counter = 1;
        } else if local_counter > 1 {
            // all ones is false if next len is not 1
            if local_counter > 2 {
                all_ones = false;
            }
            let is_odd = local_counter % 2 == 1;
            let half = local_counter / 2;
            for i in 0..half {
                instr.push((
                    bucket_positions[global_counter - (local_counter - 1) + 2 * i].scalar_index,
                    bucket_positions[global_counter - (local_counter - 1) + 2 * i + 1].scalar_index,
                ));
                bucket_positions[new_scalar_length + i] =
                    BucketPosition { bucket_index: current_bucket, scalar_index: (new_scalar_length + i) as u32 };
            }
            if is_odd {
                instr.push((bucket_positions[global_counter].scalar_index, !0u32));
                bucket_positions[new_scalar_length + half] =
                    BucketPosition { bucket_index: current_bucket, scalar_index: (new_scalar_length + half) as u32 };
            }
            // Reset the local_counter and update state
            new_scalar_length += half + (local_counter % 2);
            number_of_bases_in_batch += half;
            local_counter = 1;

            // When the number of bases in a batch crosses the threshold, perform a batch
            // addition.
            if number_of_bases_in_batch >= batch_size / 2 {
                // We need instructions for copying data in the case of noops.
                // We encode noops/copies as !0u32
                batch_add_write(bases, &instr, &mut new_bases, &mut scratch_space);

                instr.clear();
                number_of_bases_in_batch = 0;
            }
        } else {
            instr.push((bucket_positions[global_counter].scalar_index, !0u32));
            bucket_positions[new_scalar_length] =
                BucketPosition { bucket_index: current_bucket, scalar_index: new_scalar_length as u32 };
            new_scalar_length += 1;
        }
        global_counter += 1;
    }
    if !instr.is_empty() {
        batch_add_write(bases, &instr, &mut new_bases, &mut scratch_space);
        instr.clear();
    }
    global_counter = 0;
    number_of_bases_in_batch = 0;
    local_counter = 1;
    num_scalars = new_scalar_length;
    new_scalar_length = 0;

    // Next, perform all the updates in place.
    while !all_ones {
        all_ones = true;
        while global_counter < num_scalars {
            let current_bucket = bucket_positions[global_counter].bucket_index;
            while global_counter + 1 < num_scalars
                && bucket_positions[global_counter + 1].bucket_index == current_bucket
            {
                global_counter += 1;
                local_counter += 1;
            }
            if current_bucket >= num_buckets as u32 {
                local_counter = 1;
            } else if local_counter > 1 {
                // all ones is false if next len is not 1
                if local_counter != 2 {
                    all_ones = false;
                }
                let is_odd = local_counter % 2 == 1;
                let half = local_counter / 2;
                for i in 0..half {
                    instr.push((
                        bucket_positions[global_counter - (local_counter - 1) + 2 * i].scalar_index,
                        bucket_positions[global_counter - (local_counter - 1) + 2 * i + 1].scalar_index,
                    ));
                    bucket_positions[new_scalar_length + i] =
                        bucket_positions[global_counter - (local_counter - 1) + 2 * i];
                }
                if is_odd {
                    bucket_positions[new_scalar_length + half] = bucket_positions[global_counter];
                }
                // Reset the local_counter and update state
                new_scalar_length += half + (local_counter % 2);
                number_of_bases_in_batch += half;
                local_counter = 1;

                if number_of_bases_in_batch >= batch_size / 2 {
                    batch_add_in_place_same_slice(&mut new_bases, &instr);
                    instr.clear();
                    number_of_bases_in_batch = 0;
                }
            } else {
                bucket_positions[new_scalar_length] = bucket_positions[global_counter];
                new_scalar_length += 1;
            }
            global_counter += 1;
        }
        // If there are any remaining unprocessed instructions, proceed to perform batch
        // addition.
        if !instr.is_empty() {
            batch_add_in_place_same_slice(&mut new_bases, &instr);
            instr.clear();
        }
        global_counter = 0;
        number_of_bases_in_batch = 0;
        local_counter = 1;
        num_scalars = new_scalar_length;
        new_scalar_length = 0;
    }

    // Return the sparse (num_scalars, new_bases) pair. The caller walks
    // bucket_positions[..num_scalars] in reverse bucket-index order to
    // Horner-accumulate without materialising the dense res buffer.
    (num_scalars, new_bases)
}

#[inline]
fn batched_window<G: AffineCurve>(
    bases: &[G],
    scalars: &[<G::ScalarField as PrimeField>::BigInteger],
    w_start: usize,
    c: usize,
) -> (G::Projective, usize) {
    // We don't need the "zero" bucket, so we only have 2^c - 1 buckets
    let window_size = if (w_start % c) != 0 { w_start % c } else { c };
    let num_buckets = (1 << window_size) - 1;

    let mut bucket_positions: Vec<_> = scalars
        .iter()
        .enumerate()
        .map(|(scalar_index, &scalar)| {
            // Extract the c-bit window starting at bit w_start from the scalar
            // without a full BigInteger divn + modulo.  A BigInteger is stored
            // as an array of u64 limbs in little-endian order (limb 0 = bits
            // 0..63).  The window fits entirely in one or two limbs:
            //
            //   limb_idx = w_start / 64   (index of the limb containing bit w_start)
            //   bit_off  = w_start % 64   (position within that limb)
            //
            // If bit_off + c <= 64: all c bits reside in limb limb_idx.
            // Otherwise: low (64 - bit_off) bits from limb_idx and high
            //            (c - (64 - bit_off)) bits from limb_idx + 1.
            //
            // This replaces O(w_start/64 × 4) limb-shift operations in divn
            // with 1-2 array reads + 1-2 bit-ops per scalar.
            let limbs = scalar.as_ref();
            let limb_idx = w_start / 64;
            let bit_off = w_start % 64;
            let mask = (1u64 << c) - 1;
            let window_bits = if bit_off + c <= 64 {
                (limbs[limb_idx] >> bit_off) & mask
            } else {
                // Window straddles two limbs.
                let lo = limbs[limb_idx] >> bit_off;
                // Guard against reading beyond the last limb: if limb_idx+1
                // is out of range, the high bits are zero (the scalar has
                // fewer than w_start+c meaningful bits).
                let hi = if limb_idx + 1 < limbs.len() { limbs[limb_idx + 1] << (64 - bit_off) } else { 0 };
                (lo | hi) & mask
            };
            let scalar = window_bits as i32;

            BucketPosition { bucket_index: (scalar - 1) as u32, scalar_index: scalar_index as u32 }
        })
        .collect();

    let (num_reduced, new_bases) = batch_add(num_buckets, bases, &mut bucket_positions);

    // Horner accumulation directly from the sparse reduced bucket list.
    // After batch_add, bucket_positions[..num_reduced] holds (bucket_index,
    // scalar_index) pairs sorted in ascending bucket_index order.
    // We walk in descending bucket_index order (highest to lowest), merging
    // each non-empty bucket into the running sum and adding running_sum to res.
    // Empty buckets (no entry in bucket_positions for that index) contribute
    // the unchanged running_sum to res without an affine addition.
    // This replaces the former `vec![Zero::zero(); num_buckets]` scatter buffer
    // (196 KiB for c=11) with a pointer walk over ~2047 pairs (16 KiB).
    let mut res = G::Projective::zero();
    let mut running_sum = G::Projective::zero();
    let mut bp_idx = num_reduced; // points past the last valid entry
    for bucket_idx in (0..num_buckets as u32).rev() {
        // Check if the highest remaining bucket_position matches this index.
        if bp_idx > 0 && bucket_positions[bp_idx - 1].bucket_index == bucket_idx {
            bp_idx -= 1;
            running_sum.add_assign_mixed(&new_bases[bucket_positions[bp_idx].scalar_index as usize]);
        }
        res += &running_sum;
    }

    (res, window_size)
}

pub fn msm<G: AffineCurve>(bases: &[G], scalars: &[<G::ScalarField as PrimeField>::BigInteger]) -> G::Projective {
    if bases.len() < 15 {
        let num_bits = G::ScalarField::size_in_bits();
        let bigint_size = <G::ScalarField as PrimeField>::BigInteger::NUM_LIMBS * 64;
        let mut bits =
            scalars.iter().map(|s| BitIteratorBE::new(s.as_ref()).skip(bigint_size - num_bits)).collect::<Vec<_>>();
        let mut sum = G::Projective::zero();

        let mut encountered_one = false;
        for _ in 0..num_bits {
            if encountered_one {
                sum.double_in_place();
            }
            for (bits, base) in bits.iter_mut().zip(bases) {
                if let Some(true) = bits.next() {
                    sum.add_assign_mixed(base);
                    encountered_one = true;
                }
            }
        }
        debug_assert!(bits.iter_mut().all(|b| b.next().is_none()));
        sum
    } else {
        // Determine the bucket size `c` (chosen empirically).
        // Use `ln(n)` instead of the default `+ 2`: for n=65536 this gives
        // c=11 (2047 buckets) rather than c=13 (8191 buckets), quartering the
        // working-set size of the bucket accumulation arrays and improving
        // cache utilisation at the cost of more windows (ceil(255/11)=24).
        let c = match scalars.len() < 32 {
            true => 1,
            false => crate::msm::ln_without_floats(scalars.len()),
        };

        let num_bits = <G::ScalarField as PrimeField>::size_in_bits();

        // Each window is of size `c`.
        // We divide up the bits 0..num_bits into windows of size `c`, and
        // in parallel process each such window.
        let window_sums: Vec<_> =
            cfg_into_iter!(0..num_bits).step_by(c).map(|w_start| batched_window(bases, scalars, w_start, c)).collect();

        // We store the sum for the lowest window.
        let (lowest, window_sums) = window_sums.split_first().unwrap();

        // We're traversing windows from high to low.
        window_sums.iter().rev().fold(G::Projective::zero(), |mut total, (sum_i, window_size)| {
            total += sum_i;
            for _ in 0..*window_size {
                total.double_in_place();
            }
            total
        }) + lowest.0
    }
}
