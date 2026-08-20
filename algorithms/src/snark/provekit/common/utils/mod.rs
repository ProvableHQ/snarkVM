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

pub mod serde_ark;
pub mod serde_ark_option;
pub mod serde_ark_vec;
pub mod serde_hex;
pub mod sumcheck;

use std::mem::size_of;

/// Target single-thread workload size for `T`.
/// Should ideally be a multiple of a cache line (64 bytes)
/// and close to the L1 cache size (32 KB).
pub const fn workload_size<T: Sized>() -> usize {
    const CACHE_SIZE: usize = 1 << 15;
    CACHE_SIZE / size_of::<T>()
}

/// Unzip a `[[(T, T); N]; M]` into `([[T; N]; M], [[T; N]; M])` using move
/// semantics.
pub(super) fn unzip_double_array<T, const N: usize, const M: usize>(
    input: [[(T, T); N]; M],
) -> ([[T; N]; M], [[T; N]; M]) {
    let mut left_vec = Vec::with_capacity(M);
    let mut right_vec = Vec::with_capacity(M);
    for row in input {
        let mut left_row = Vec::with_capacity(N);
        let mut right_row = Vec::with_capacity(N);
        for (left, right) in row {
            left_row.push(left);
            right_row.push(right);
        }
        // The inner vectors have length `N` by construction.
        left_vec.push(left_row.try_into().unwrap_or_else(|_| unreachable!("row length is N")));
        right_vec.push(right_row.try_into().unwrap_or_else(|_| unreachable!("row length is N")));
    }
    // The outer vectors have length `M` by construction.
    (
        left_vec.try_into().unwrap_or_else(|_| unreachable!("column length is M")),
        right_vec.try_into().unwrap_or_else(|_| unreachable!("column length is M")),
    )
}

/// Calculates the degree of the next smallest power of two.
pub const fn next_power_of_two(n: usize) -> usize {
    let mut power = 1;
    let mut ans = 0;
    while power < n {
        power <<= 1;
        ans += 1;
    }
    ans
}

/// Pads the vector with the default value so that the number of elements is a
/// power of 2.
pub fn pad_to_power_of_two<T: Default>(mut witness: Vec<T>) -> Vec<T> {
    let target_len = 1 << next_power_of_two(witness.len());
    witness.reserve_exact(target_len - witness.len());
    while witness.len() < target_len {
        witness.push(T::default());
    }
    witness
}
