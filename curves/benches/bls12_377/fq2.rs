// Copyright 2024 Aleo Network Foundation
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

use snarkvm_curves::bls12_377::Fq2;
use snarkvm_fields::{Field, SquareRootField};
use snarkvm_utilities::rand::{TestRng, Uniform};

use criterion::Criterion;
use std::ops::{AddAssign, MulAssign, SubAssign};

pub(crate) fn bench_fq2_add_assign(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<(Fq2, Fq2)> = (0..SAMPLES).map(|_| (Fq2::rand(&mut rng), Fq2::rand(&mut rng))).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fq2_add_assign", |c| {
        c.iter(|| {
            let mut tmp = v[count].0;
            tmp.add_assign(v[count].1);
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fq2_sub_assign(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<(Fq2, Fq2)> = (0..SAMPLES).map(|_| (Fq2::rand(&mut rng), Fq2::rand(&mut rng))).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fq2_sub_assign", |c| {
        c.iter(|| {
            let mut tmp = v[count].0;
            tmp.sub_assign(&v[count].1);
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fq2_mul_assign(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<(Fq2, Fq2)> = (0..SAMPLES).map(|_| (Fq2::rand(&mut rng), Fq2::rand(&mut rng))).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fq2_mul_assign", |c| {
        c.iter(|| {
            let mut tmp = v[count].0;
            tmp.mul_assign(&v[count].1);
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fq2_double(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<Fq2> = (0..SAMPLES).map(|_| Fq2::rand(&mut rng)).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fq2_double", |c| {
        c.iter(|| {
            let mut tmp = v[count];
            tmp.double_in_place();
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fq2_square(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<Fq2> = (0..SAMPLES).map(|_| Fq2::rand(&mut rng)).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fq2_square", |c| {
        c.iter(|| {
            let mut tmp = v[count];
            tmp.square_in_place();
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fq2_inverse(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<Fq2> = (0..SAMPLES).map(|_| Fq2::rand(&mut rng)).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fq2_inverse", |c| {
        c.iter(|| {
            let tmp = v[count].inverse();
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fq2_sqrt(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<Fq2> = (0..SAMPLES).map(|_| Fq2::rand(&mut rng)).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fq2_sqrt", |c| {
        c.iter(|| {
            let tmp = v[count].sqrt();
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}
