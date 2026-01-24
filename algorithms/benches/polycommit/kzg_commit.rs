// Copyright (c) 2019-2025 Provable Inc.
// This file is part of the snarkVM library.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[macro_use]
extern crate criterion;

use criterion::{BenchmarkId, Criterion};
use std::hint::black_box;

use snarkvm_algorithms::{
    fft::{DensePolynomial, EvaluationDomain, Polynomial},
    polycommit::kzg10::{KZG10, LagrangeBasis, Powers},
};
use snarkvm_curves::bls12_377::{Bls12_377, Fr};
use snarkvm_utilities::TestRng;

use std::{borrow::Cow, time::Instant};

type Kzg = KZG10<Bls12_377>;

/// Benchmarks KZG10 commitment in coefficient basis vs Lagrange basis.
///
/// This measures commit-only cost (no hiding), with all SRS trimming, Lagrange
/// basis construction, and evaluation generation done *outside* the timed loop.
fn kzg_commit_vs_commit_lagrange(c: &mut Criterion) {
    // Keep these as powers-of-two to satisfy `commit_lagrange` invariants.
    let sizes: &[usize] = &[
        1 << 15, // 32768
        1 << 20, // 1048576
        1 << 21, /* 33554432
                  * 1 << 22, // 134217728
                  * 1 << 23, // 268435456
                  * 1 << 24, // 536870912
                  * 1 << 25, // 1073741824
                  * 1 << 26, // 2147483648
                  * 1 << 27, // 4294967296
                  * 1 << 28, // 8589934592 */
    ];

    // TODO: run this if you just want to download powers of the SRS.
    // for max_degree in sizes {
    //     let max_size = *max_degree;
    //     let max_degree = max_size - 1;
    //     let pp_max_degree_is_ok = KZG::load_srs(max_degree).is_ok();
    //     let pp_max_size_is_ok = KZG::load_srs(max_size).is_ok();
    //     println!("max_degree = {max_degree}, max_size = {max_size}");
    //     println!("pp_max_degree_is_ok = {pp_max_degree_is_ok}, pp_max_size_is_ok
    // = {pp_max_size_is_ok}"); }

    let max_size = *sizes.last().unwrap();
    let max_degree = max_size - 1;

    // Load SRS once at the maximum size; per-size derived structures are built from
    // this.
    let pp = Kzg::load_srs(max_degree).unwrap();

    let mut group = c.benchmark_group("kzg10_commit");
    group.sample_size(10);

    // Pre-generate all inputs so Criterion iterations only measure commit time.
    let mut rng = TestRng::default();
    for &n in sizes {
        let domain = EvaluationDomain::<Fr>::new(n).unwrap();

        // Polynomial of degree n-1.
        let poly = DensePolynomial::<Fr>::rand(n - 1, &mut rng);
        let poly_ref: Polynomial<'_, Fr> = (&poly).into();

        // Evaluations on the same domain, used for commit_lagrange.
        let evals = domain.fft(&poly.coeffs);

        // Powers for coefficient-basis commit (need n powers for degree n-1).
        let powers = Powers::<Bls12_377> {
            powers_of_beta_g: Cow::Owned(pp.powers_of_beta_g(0, n).unwrap()),
            powers_of_beta_times_gamma_g: Cow::Owned(vec![]), // no hiding
        };

        // Lagrange basis for Lagrange-basis commit.
        let lagrange_basis = LagrangeBasis::<Bls12_377> {
            lagrange_basis_at_beta_g: Cow::Owned(pp.lagrange_basis(domain).unwrap()),
            powers_of_beta_times_gamma_g: Cow::Owned(vec![]), // no hiding
            domain,
        };

        // let mut dummy_coeffs = evals.clone();
        // let start_time = Instant::now();
        // domain.ifft_in_place(&mut dummy_coeffs);
        // let end_time = Instant::now();
        // let ifft_time = end_time.duration_since(start_time);
        // println!("ifft time = {ifft_time:?}");
        // let start_time = Instant::now();
        // let _ = Kzg::commit(black_box(&powers), black_box(&poly_ref), None,
        // None).unwrap(); let end_time = Instant::now();
        // let commit_time = end_time.duration_since(start_time);
        // println!("commit time = {commit_time:?}");
        // let start_time = Instant::now();
        // let _ = Kzg::commit_lagrange(black_box(&lagrange_basis), black_box(&evals),
        // None, None).unwrap(); let end_time = Instant::now();
        // let commit_lagrange_time = end_time.duration_since(start_time);
        // println!("commit_lagrange time = {commit_lagrange_time:?}");

        group.bench_with_input(BenchmarkId::new("ifft", n), &n, |b, _| {
            let mut dummy_coeffs = evals.clone();
            b.iter(|| {
                domain.ifft_in_place(&mut dummy_coeffs);
            })
        });

        group.bench_with_input(BenchmarkId::new("commit", n), &n, |b, _| {
            b.iter(|| {
                let _ = Kzg::commit(black_box(&powers), black_box(&poly_ref), None, None).unwrap();
            })
        });

        group.bench_with_input(BenchmarkId::new("commit_lagrange", n), &n, |b, _| {
            b.iter(|| {
                let _ = Kzg::commit_lagrange(black_box(&lagrange_basis), black_box(&evals), None, None).unwrap();
            })
        });
    }

    group.finish();
}

criterion_group!(benches, kzg_commit_vs_commit_lagrange);
criterion_main!(benches);
