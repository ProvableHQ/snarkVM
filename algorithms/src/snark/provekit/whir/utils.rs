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

#[cfg(any())]
use std::fmt::Debug;
use std::{
    iter::{FusedIterator, RepeatN},
    slice::ChunksExact,
};

#[cfg(any())]
use serde::{Deserialize, Serialize};
use snarkvm_fields::Field;

#[macro_export]
macro_rules! ensure {
    ($cond:expr, $err:expr) => {
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if !$cond {
            return Err($err.into());
        };
    };
}

pub enum EitherIter<A, B> {
    Left(A),
    Right(B),
}

/// Target single-thread workload size for `T`.
/// Should ideally be a multiple of a cache line (64 bytes)
/// and close to the L1 cache size.
pub const fn workload_size<T: Sized>() -> usize {
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    const CACHE_SIZE: usize = 1 << 17; // 128KB for Apple Silicon

    #[cfg(all(target_arch = "aarch64", any(target_os = "ios", target_os = "android", target_os = "linux")))]
    const CACHE_SIZE: usize = 1 << 16; // 64KB for mobile/server ARM

    #[cfg(target_arch = "x86_64")]
    const CACHE_SIZE: usize = 1 << 15; // 32KB for x86-64

    #[cfg(not(any(
        all(target_arch = "aarch64", target_os = "macos"),
        all(target_arch = "aarch64", any(target_os = "ios", target_os = "android", target_os = "linux")),
        target_arch = "x86_64"
    )))]
    const CACHE_SIZE: usize = 1 << 15; // 32KB default

    CACHE_SIZE / size_of::<T>()
}

/// Like zip, but panics if the iterators are not the same length.
#[inline]
pub fn zip_strict<A, B>(
    a: impl IntoIterator<Item = A>,
    b: impl IntoIterator<Item = B>,
) -> impl Iterator<Item = (A, B)> {
    let mut a = a.into_iter();
    let mut b = b.into_iter();
    std::iter::from_fn(move || match (a.next(), b.next()) {
        (Some(x), Some(y)) => Some((x, y)),
        (None, None) => None,
        _ => panic!("Iterators had different lengths"),
    })
}

impl<T, A, B> Iterator for EitherIter<A, B>
where
    A: Iterator<Item = T>,
    B: Iterator<Item = T>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Left(a) => a.next(),
            Self::Right(b) => b.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Left(a) => a.size_hint(),
            Self::Right(b) => b.size_hint(),
        }
    }
}

impl<T, A, B> DoubleEndedIterator for EitherIter<A, B>
where
    A: DoubleEndedIterator<Item = T>,
    B: DoubleEndedIterator<Item = T>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Left(a) => a.next_back(),
            Self::Right(b) => b.next_back(),
        }
    }
}

impl<T, A, B> ExactSizeIterator for EitherIter<A, B>
where
    A: ExactSizeIterator<Item = T>,
    B: ExactSizeIterator<Item = T>,
{
    fn len(&self) -> usize {
        match self {
            Self::Left(a) => a.len(),
            Self::Right(b) => b.len(),
        }
    }
}

impl<T, A, B> FusedIterator for EitherIter<A, B>
where
    A: FusedIterator<Item = T>,
    B: FusedIterator<Item = T>,
{
}

pub fn chunks_exact_or_empty<T>(
    slice: &[T],
    chunk_size: usize,
    count: usize,
) -> EitherIter<RepeatN<&[T]>, ChunksExact<'_, T>> {
    assert_eq!(slice.len(), chunk_size * count);
    if chunk_size == 0 {
        EitherIter::Left(std::iter::repeat_n(&slice[0..0], count))
    } else {
        EitherIter::Right(slice.chunks_exact(chunk_size))
    }
}

#[cfg(not(feature = "serial"))]
pub fn par_chunks_exact_or_empty<T: Sync>(
    slice: &[T],
    chunk_size: usize,
    count: usize,
) -> rayon::iter::Either<rayon::iter::RepeatN<&[T]>, rayon::slice::ChunksExact<'_, T>> {
    use rayon::{iter::Either, slice::ParallelSlice};
    assert_eq!(slice.len(), chunk_size * count);
    if chunk_size == 0 {
        Either::Left(rayon::iter::repeat_n(&slice[0..0], count))
    } else {
        Either::Right(slice.par_chunks_exact(chunk_size))
    }
}

// TODO(Gotti): n_bits is a misnomer if base > 2. Should be n_limbs or sth.
// Also, should the behaviour for value >= base^n_bits be specified as part of
// the API or asserted not to happen? Currently, we compute the decomposition of
// value % (base^n_bits).

/// Decomposes `value` into its base-`base` representation with `n_bits` digits.
/// The result follows big-endian order:
/// ```ignore
/// value = v[0] * base^(n_bits-1) + v[1] * base^(n_bits-2) + ... + v[n_bits-1] * 1
/// ```
/// where each `v[i]` is in `0..base`. Always returns exactly `n_bits` digits
/// (padded with 0).
pub fn base_decomposition(mut value: usize, base: u8, n_bits: usize) -> Vec<u8> {
    debug_assert!(base > 1, "Base must be at least 2");

    // Preallocate a vector with zeroes, ensuring it's exactly `n_bits` long
    let mut result = vec![0u8; n_bits];

    // Compute the decomposition in reverse order (avoids shifting later)
    for digit in result.iter_mut().rev() {
        *digit = (value % (base as usize)) as u8;
        value /= base as usize;
    }

    result
}

// Gotti: Consider renaming this function. The name sounds like it's a PRG.
// TODO (Gotti): Check that ordering is actually correct at point of use
// (everything else is big-endian).

/// Generates a sequence of powers of `base`, starting from `1`.
///
/// This function returns a vector containing the sequence:
/// `[1, base, base^2, base^3, ..., base^(len-1)]`
pub fn expand_randomness<F: Field>(base: F, len: usize) -> Vec<F> {
    let mut res = Vec::with_capacity(len);
    let mut acc = F::one();
    for _ in 0..len {
        res.push(acc);
        acc *= base;
    }
    res
}

#[cfg(any())]
#[track_caller]
pub fn test_serde<T: Debug + PartialEq + Serialize + for<'a> Deserialize<'a>>(value: &T) {
    // Test in human-readable format
    let json = serde_json::to_string_pretty(value).expect("json serialization failed");
    let deserialized = serde_json::from_str(&json).expect("json deserialization failed");
    assert_eq!(value, &deserialized, "json serde roundtrip failed");

    // Test in schemaless binary format

    let bytes = {
        let mut writer = Vec::new();
        ciborium::into_writer(value, &mut writer).expect("ciborium serialization failed");
        writer
    };
    let deserialized = ciborium::from_reader(bytes.as_slice()).expect("ciborium deserialization failed");
    assert_eq!(value, &deserialized, "ciborium serde roundtrip failed");
}

#[cfg(any())]
mod tests {
    use snarkvm_fields::{Field, One, Zero};

    use super::*;
    use crate::snark::provekit::whir::algebra::fields::Field64;

    #[test]
    fn test_base_decomposition_binary() {
        // Base-2 decomposition (big-endian, padded to n_bits)
        // 11 in binary (6-bit representation): 001011
        assert_eq!(base_decomposition(0b1011, 2, 6), vec![0, 0, 1, 0, 1, 1]);

        // 5 in binary (4-bit representation): 0101
        assert_eq!(base_decomposition(5, 2, 4), vec![0, 1, 0, 1]);

        // 10 in binary (4-bit representation): 1010
        assert_eq!(base_decomposition(10, 2, 4), vec![1, 0, 1, 0]);

        // 0 in binary (4-bit representation): 0000
        assert_eq!(base_decomposition(0, 2, 4), vec![0, 0, 0, 0]);
    }

    #[test]
    fn test_base_decomposition_ternary() {
        // Base-3 decomposition (big-endian, padded to n_bits)
        // 15 in base-3 (3-digit representation): 120
        // Computation:
        // 15 / 3 = 5 remainder 0
        // 5 / 3  = 1 remainder 2
        // 1 / 3  = 0 remainder 1
        // Result = [1, 2, 0]
        assert_eq!(base_decomposition(15, 3, 3), vec![1, 2, 0]);

        // 8 in base-3 (4-digit representation): 0022
        // Computation:
        // 8 / 3 = 2 remainder 2
        // 2 / 3 = 0 remainder 2
        // Result = [0, 0, 2, 2] (padded to 4 bits)
        assert_eq!(base_decomposition(8, 3, 4), vec![0, 0, 2, 2]);
    }

    #[test]
    fn test_base_decomposition_large_values() {
        // Base-5 decomposition (4-digit representation of 123)
        // Computation:
        // 123 / 5 = 24 remainder 3
        // 24 / 5  = 4 remainder 4
        // 4 / 5   = 0 remainder 4
        // Result = [0, 4, 4, 3] (padded to 4 bits)
        assert_eq!(base_decomposition(123, 5, 4), vec![0, 4, 4, 3]);

        // Base-7 decomposition (5-digit representation of 100)
        // Computation:
        // 100 / 7 = 14 remainder 2
        // 14 / 7  = 2 remainder 0
        // 2 / 7   = 0 remainder 2
        // Result = [0, 0, 2, 0, 2] (padded to 5 bits)
        assert_eq!(base_decomposition(100, 7, 5), vec![0, 0, 2, 0, 2]);
    }

    #[test]
    fn test_base_decomposition_padding() {
        // Ensure correct padding when value is smaller than max base^n_bits
        // Base-4 decomposition (5 in base-4 with 5-bit representation)
        // 5 in base-4: 11 → padded to [0, 0, 0, 1, 1]
        assert_eq!(base_decomposition(5, 4, 5), vec![0, 0, 0, 1, 1]);

        // 2 in base-3 (4-bit representation): 0002
        assert_eq!(base_decomposition(2, 3, 4), vec![0, 0, 0, 2]);

        // 0 in base-5 (4-bit representation): 0000
        assert_eq!(base_decomposition(0, 5, 4), vec![0, 0, 0, 0]);
    }

    #[test]
    fn test_base_decomposition_edge_cases() {
        // Edge case: Maximum value within n_bits range
        // Base-2 decomposition of 15 in 4-bit representation: 1111
        assert_eq!(base_decomposition(15, 2, 4), vec![1, 1, 1, 1]);

        // Edge case: Maximum value within n_bits range for base-3
        // Base-3 decomposition of 26 in 3-bit representation (26 mod 3^3)
        // 26 in base-3: 222 → padded to 3 bits
        assert_eq!(base_decomposition(26, 3, 3), vec![2, 2, 2]);

        // Edge case: Maximum value that can fit in n_bits
        // Base-4 decomposition of 63 in 3-bit representation (63 mod 4^3)
        // 63 in base-4: 333 → padded to 3 bits
        assert_eq!(base_decomposition(63, 4, 3), vec![3, 3, 3]);
    }

    #[test]
    fn test_base_decomposition_truncation_behavior() {
        // Ensure truncation when value exceeds `base^n_bits`
        // (Undocumented but current behavior: computes modulo base^n_bits)
        //
        // Base-3 decomposition of (15 + 81) with 3-bit representation
        // (15 + 81) = 96, which is equivalent to (15 mod 27) = 15
        // 15 in base-3: 120
        assert_eq!(base_decomposition(15 + 81, 3, 3), vec![1, 2, 0]);

        // Base-2 decomposition of (20 + 16) with 4-bit representation
        // (20 + 16) = 36, which is equivalent to (20 mod 16) = 4
        // 4 in base-2: 0100
        assert_eq!(base_decomposition(20 + 16, 2, 4), vec![0, 1, 0, 0]);
    }

    #[test]
    #[should_panic]
    fn test_base_decomposition_invalid_base() {
        // Base cannot be 0 or 1 (should panic)
        base_decomposition(10, 0, 5);
    }

    #[test]
    fn test_expand_randomness_basic() {
        // Test with base = 2 and length = 5
        let base = Field64::from(2);
        let len = 5;

        let expected = vec![Field64::ONE, Field64::from(2), Field64::from(4), Field64::from(8), Field64::from(16)];

        assert_eq!(expand_randomness(base, len), expected);
    }

    #[test]
    fn test_expand_randomness_zero_length() {
        // If len = 0, should return an empty vector
        let base = Field64::from(3);
        assert!(expand_randomness(base, 0).is_empty());
    }

    #[test]
    fn test_expand_randomness_one_length() {
        // If len = 1, should return [1]
        let base = Field64::from(5);
        assert_eq!(expand_randomness(base, 1), vec![Field64::ONE]);
    }

    #[test]
    fn test_expand_randomness_large_base() {
        // Test with a large base value
        let base = Field64::from(10);
        let len = 4;

        let expected = vec![Field64::ONE, Field64::from(10), Field64::from(100), Field64::from(1000)];

        assert_eq!(expand_randomness(base, len), expected);
    }

    #[test]
    fn test_expand_randomness_identity_case() {
        // If base = 1, all values should be 1
        let base = Field64::ONE;
        let len = 6;

        let expected = vec![Field64::ONE; len];
        assert_eq!(expand_randomness(base, len), expected);
    }

    #[test]
    fn test_expand_randomness_zero_base() {
        // If base = 0, all values after the first should be 0
        let base = Field64::ZERO;
        let len = 5;

        let expected = vec![Field64::ONE, Field64::ZERO, Field64::ZERO, Field64::ZERO, Field64::ZERO];
        assert_eq!(expand_randomness(base, len), expected);
    }

    #[test]
    fn test_expand_randomness_negative_base() {
        // Test with base = -1, which should alternate between 1 and -1
        let base = -Field64::ONE;
        let len = 6;

        let expected = vec![Field64::ONE, -Field64::ONE, Field64::ONE, -Field64::ONE, Field64::ONE, -Field64::ONE];

        assert_eq!(expand_randomness(base, len), expected);
    }
}
