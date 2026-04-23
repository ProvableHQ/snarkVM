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

// LLVM loop-unrolling miscompilation reproducer for aarch64-linux-gnu.
//
// The miscompilation is in `EvaluationDomain::evaluate_all_lagrange_coefficients`,
// which computes Lagrange basis polynomial values at a given point. This function
// uses `batch_inversion` on the domain-sized vector. In release mode, the returned
// coefficients violate the partition-of-unity property (they must sum to 1).
//
// Internally, the function:
//   1. Computes u[i] = tau - omega^i  for all i in 0..n
//   2. Calls batch_inversion(u)  (Montgomery's trick, parallelized via rayon)
//   3. Multiplies each u[i] by the corresponding numerator
//
// The miscompilation is in the batch_inversion or the loop that feeds it.
//
// Trigger conditions:
//   target:      aarch64-unknown-linux-gnu
//   opt-level:   3
//   lto:         "thin"
//   incremental: true
//
// Usage:
//   cargo run --release    # FAILS
//   cargo run              # PASSES
//
// Workaround:
//   RUSTFLAGS="-C llvm-args=--unroll-threshold=0" cargo run --release
//
// Observed with:
//   rustc 1.88.0 (6b00bc388 2025-06-23)
//   aarch64-unknown-linux-gnu (GCP ARM VM, linux 6.17.0-1002-gcp)

use snarkvm_algorithms::fft::EvaluationDomain;
use snarkvm_curves::bls12_377::Fr;
use snarkvm_fields::{Field, One};

fn main() {
    let mut fail_any = false;
    fail_any |= !test_lagrange_coefficients(1 << 4);
    fail_any |= !test_lagrange_coefficients(1 << 8);
    fail_any |= !test_lagrange_coefficients(1 << 13);
    fail_any |= !test_lagrange_coefficients(1 << 14);
    fail_any |= !test_lagrange_coefficients(1 << 15);
    fail_any |= !test_lagrange_coefficients(1 << 16);
    fail_any |= !test_lagrange_coefficients(1 << 17);
    if fail_any {
        println!("FAIL");
    } else {
        println!("PASS");
    }
}

fn test_lagrange_coefficients(domain_size: usize) -> bool {
    let log_n = (domain_size as f64).log2() as u32;
    print!("Lagrange coefficients (domain 2^{log_n})...\n");

    let domain = EvaluationDomain::<Fr>::new(domain_size).unwrap();
    let alpha = Fr::from(9999999999999u64);

    let coeffs = domain.evaluate_all_lagrange_coefficients(alpha);

    // Partition-of-unity check: sum of all Lagrange coefficients must be 1.
    let sum: Fr = coeffs.iter().copied().sum();
    if !sum.is_one() {
        println!(" FAIL (sum != 1)");
        return false;
    }

    // Spot-check: L_i(alpha) = (alpha^n - 1) * omega^i / (n * (alpha - omega^i))
    let alpha_n_minus_1 = alpha.pow([domain.size as u64]) - Fr::one();
    let n_inv = domain.size_inv;
    let mut omega_i = Fr::one();
    for i in 0..10 {
        let expected = alpha_n_minus_1 * omega_i * n_inv * (alpha - omega_i).inverse().unwrap();
        if coeffs[i] != expected {
            println!(" FAIL (wrong value at index {i})");
            return false;
        }
        omega_i *= domain.group_gen;
    }

    true
}
