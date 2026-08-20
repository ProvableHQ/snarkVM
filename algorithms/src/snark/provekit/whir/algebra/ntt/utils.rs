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

/// Compute the largest factor of `n` that is ≤ sqrt(n).
/// Assumes `n` is of the form `2^k * {1,3,9}`.
pub fn sqrt_factor(n: usize) -> usize {
    // Count the number of trailing zeros in `n`, i.e., the power of 2 in `n`
    let twos = n.trailing_zeros();

    // Divide `n` by the highest power of 2 to extract the base component
    let base = n >> twos;

    // Determine the largest factor ≤ sqrt(n) based on the extracted `base`
    match base {
        // Case: `n` is purely a power of 2 (base = 1)
        // The largest factor ≤ sqrt(n) is 2^(twos/2)
        1 => 1 << (twos / 2),

        // Case: `n = 2^k * 3`
        3 => {
            if twos == 0 {
                // sqrt(3) ≈ 1.73, so the largest integer factor ≤ sqrt(3) is 1
                1
            } else {
                // - If `twos` is even: The largest factor is `3 * 2^((twos - 1) / 2)`
                // - If `twos` is odd: The largest factor is `2^((twos / 2))`
                if twos.is_multiple_of(2) { 3 << ((twos - 1) / 2) } else { 2 << (twos / 2) }
            }
        }

        // Case: `n = 2^k * 9`
        9 => {
            if twos == 1 {
                // sqrt(9 * 2^1) = sqrt(18) ≈ 4.24, largest factor ≤ sqrt(18) is 3
                3
            } else {
                // - If `twos` is even: The largest factor is `3 * 2^(twos / 2)`
                // - If `twos` is odd: The largest factor is `4 * 2^(twos / 2)`
                if twos.is_multiple_of(2) { 3 << (twos / 2) } else { 4 << (twos / 2) }
            }
        }

        // If `base` is not in {1,3,9}, `n` is not in the expected form
        _ => panic!("n is not in the form 2^k * {{1,3,9}}"),
    }
}

/// Least common multiple.
///
/// Note that lcm(0,0) will panic (rather than give the correct answer 0).
pub const fn lcm(a: usize, b: usize) -> usize {
    a * (b / gcd(a, b))
}

/// Greatest common divisor.
pub const fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Asserts that `n` factors over `primes` and returns a sorted list of
/// divisors.
pub fn divisors(n: usize, primes: &[usize]) -> Vec<usize> {
    let mut result = vec![1usize];
    let mut remaining = n;
    for &p in primes {
        let mut pk = 1usize;
        let existing = result.clone();
        while remaining.is_multiple_of(p) {
            pk *= p;
            remaining /= p;
            result.extend(existing.iter().map(|d| d * pk));
        }
    }
    assert_eq!(remaining, 1);
    result.sort_unstable();
    result
}

#[cfg(any())]
mod tests {
    use proptest::prelude::*;

    use super::{divisors, gcd, lcm, sqrt_factor};

    /// Computes the largest factor of `x` that is ≤ sqrt(x).
    /// If `x` is 0, returns 0.
    fn get_largest_divisor_up_to_sqrt(x: usize) -> usize {
        if x == 0 {
            return 0;
        }

        let mut result = 1;

        // Compute integer square root of `x` using floating point arithmetic.
        #[allow(clippy::cast_sign_loss)]
        let isqrt_x = (x as f64).sqrt() as usize;

        // Iterate from 1 to `isqrt_x` to find the largest factor of `x`.
        for i in 1..=isqrt_x {
            if x.is_multiple_of(i) {
                // Update `result` with the largest divisor found.
                result = i;
            }
        }

        result
    }

    #[test]
    fn test_gcd() {
        assert_eq!(gcd(4, 6), 2);
        assert_eq!(gcd(0, 4), 4);
        assert_eq!(gcd(4, 0), 4);
        assert_eq!(gcd(1, 1), 1);
        assert_eq!(gcd(64, 16), 16);
        assert_eq!(gcd(81, 9), 9);
        assert_eq!(gcd(0, 0), 0);
    }

    #[test]
    fn test_lcm() {
        assert_eq!(lcm(5, 6), 30);
        assert_eq!(lcm(3, 7), 21);
        assert_eq!(lcm(0, 10), 0);
    }

    #[test]
    fn test_sqrt_factor() {
        // Cases where n = 2^k * 1
        assert_eq!(sqrt_factor(1), 1); // 1 = 2^0 * 1
        assert_eq!(sqrt_factor(4), 2); // 4 = 2^2 * 1
        assert_eq!(sqrt_factor(16), 4); // 16 = 2^4 * 1
        assert_eq!(sqrt_factor(32), 4); // 32 = 2^5 * 1
        assert_eq!(sqrt_factor(64), 8); // 64 = 2^6 * 1
        assert_eq!(sqrt_factor(256), 16); // 256 = 2^8 * 1

        // Cases where n = 2^k * 3
        assert_eq!(sqrt_factor(3), 1); // 3 = 2^0 * 3
        assert_eq!(sqrt_factor(12), 3); // 12 = 2^2 * 3
        assert_eq!(sqrt_factor(48), 6); // 48 = 2^4 * 3
        assert_eq!(sqrt_factor(192), 12); // 192 = 2^6 * 3
        assert_eq!(sqrt_factor(768), 24); // 768 = 2^8 * 3

        // Cases where n = 2^k * 9
        assert_eq!(sqrt_factor(9), 3); // 9 = 2^0 * 9
        assert_eq!(sqrt_factor(36), 6); // 36 = 2^2 * 9
        assert_eq!(sqrt_factor(144), 12); // 144 = 2^4 * 9
        assert_eq!(sqrt_factor(576), 24); // 576 = 2^6 * 9
        assert_eq!(sqrt_factor(2304), 48); // 2304 = 2^8 * 9
    }

    #[test]
    fn test_divisors() {
        assert_eq!(divisors(24, &[2, 3]), vec![1, 2, 3, 4, 6, 8, 12, 24]);
        assert_eq!(divisors(210, &[2, 3, 5, 7]), vec![1, 2, 3, 5, 6, 7, 10, 14, 15, 21, 30, 35, 42, 70, 105, 210]);
        assert_eq!(divisors(8 * 9 * 25, &[2, 3, 5]), vec![
            1, 2, 3, 4, 5, 6, 8, 9, 10, 12, 15, 18, 20, 24, 25, 30, 36, 40, 45, 50, 60, 72, 75, 90, 100, 120, 150, 180,
            200, 225, 300, 360, 450, 600, 900, 1800
        ]);
    }

    proptest! {
        #[test]
        fn proptest_sqrt_factor(k in 0usize..30, base in prop_oneof![Just(1), Just(3), Just(9)])
        {
            let n = (1 << k) * base;
            let expected = get_largest_divisor_up_to_sqrt(n);
            prop_assert_eq!(sqrt_factor(n), expected);
        }
    }
}
