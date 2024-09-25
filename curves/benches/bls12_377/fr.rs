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

use snarkvm_curves::bls12_377::Fr;
use snarkvm_fields::{Field, PrimeField, SquareRootField};
use snarkvm_utilities::{
    biginteger::{BigInteger, BigInteger256 as FrRepr},
    rand::{TestRng, Uniform},
};

use criterion::Criterion;
use std::ops::{AddAssign, MulAssign, SubAssign};

pub(crate) fn bench_fr_repr_add_nocarry(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<(FrRepr, FrRepr)> = (0..SAMPLES)
        .map(|_| {
            let mut tmp1 = FrRepr::rand(&mut rng);
            let mut tmp2 = FrRepr::rand(&mut rng);
            // Shave a few bits off to avoid overflow.
            for _ in 0..3 {
                tmp1.div2();
                tmp2.div2();
            }
            (tmp1, tmp2)
        })
        .collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_repr_add_nocarry", |c| {
        c.iter(|| {
            let mut tmp = v[count].0;
            tmp.add_nocarry(&v[count].1);
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fr_repr_sub_noborrow(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<(FrRepr, FrRepr)> = (0..SAMPLES)
        .map(|_| {
            let tmp1 = FrRepr::rand(&mut rng);
            let mut tmp2 = tmp1;
            // Ensure tmp2 is smaller than tmp1.
            for _ in 0..10 {
                tmp2.div2();
            }
            (tmp1, tmp2)
        })
        .collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_repr_sub_noborrow", |c| {
        c.iter(|| {
            let mut tmp = v[count].0;
            tmp.sub_noborrow(&v[count].1);
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fr_repr_num_bits(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<FrRepr> = (0..SAMPLES).map(|_| FrRepr::rand(&mut rng)).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_repr_num_bits", |c| {
        c.iter(|| {
            let tmp = v[count].num_bits();
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fr_repr_mul2(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<FrRepr> = (0..SAMPLES).map(|_| FrRepr::rand(&mut rng)).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_repr_mul2", |c| {
        c.iter(|| {
            let mut tmp = v[count];
            tmp.mul2();
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fr_repr_div2(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<FrRepr> = (0..SAMPLES).map(|_| FrRepr::rand(&mut rng)).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_repr_div2", |c| {
        c.iter(|| {
            let mut tmp = v[count];
            tmp.div2();
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fr_add_assign(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<(Fr, Fr)> = (0..SAMPLES).map(|_| (Fr::rand(&mut rng), Fr::rand(&mut rng))).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_add_assign", |c| {
        c.iter(|| {
            let mut tmp = v[count].0;
            tmp.add_assign(v[count].1);
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fr_sub_assign(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<(Fr, Fr)> = (0..SAMPLES).map(|_| (Fr::rand(&mut rng), Fr::rand(&mut rng))).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_sub_assign", |c| {
        c.iter(|| {
            let mut tmp = v[count].0;
            tmp.sub_assign(&v[count].1);
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fr_mul_assign(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<(Fr, Fr)> = (0..SAMPLES).map(|_| (Fr::rand(&mut rng), Fr::rand(&mut rng))).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_mul_assign", |c| {
        c.iter(|| {
            let mut tmp = v[count].0;
            tmp.mul_assign(&v[count].1);
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fr_double(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<Fr> = (0..SAMPLES).map(|_| Fr::rand(&mut rng)).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_double", |c| {
        c.iter(|| {
            let mut tmp = v[count];
            tmp.double_in_place();
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fr_square(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<Fr> = (0..SAMPLES).map(|_| Fr::rand(&mut rng)).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_square", |c| {
        c.iter(|| {
            let mut tmp = v[count];
            tmp.square_in_place();
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fr_inverse(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<Fr> = (0..SAMPLES).map(|_| Fr::rand(&mut rng)).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_inverse", |c| {
        c.iter(|| {
            count = (count + 1) % SAMPLES;
            v[count].inverse()
        })
    });
}

pub(crate) fn bench_fr_negate(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<Fr> = (0..SAMPLES).map(|_| Fr::rand(&mut rng)).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_negate", |c| {
        c.iter(|| {
            let mut tmp = v[count];
            tmp = -tmp;
            count = (count + 1) % SAMPLES;
            tmp
        })
    });
}

pub(crate) fn bench_fr_sqrt(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<Fr> = (0..SAMPLES)
        .map(|_| {
            let mut tmp = Fr::rand(&mut rng);
            tmp.square_in_place();
            tmp
        })
        .collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_sqrt", |c| {
        c.iter(|| {
            count = (count + 1) % SAMPLES;
            v[count].sqrt()
        })
    });
}

pub(crate) fn bench_fr_to_bigint(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<Fr> = (0..SAMPLES).map(|_| Fr::rand(&mut rng)).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_to_bigint", |c| {
        c.iter(|| {
            count = (count + 1) % SAMPLES;
            v[count].to_bigint()
        })
    });
}

pub(crate) fn bench_fr_from_bigint(c: &mut Criterion) {
    const SAMPLES: usize = 1000;

    let mut rng = TestRng::default();

    let v: Vec<FrRepr> = (0..SAMPLES).map(|_| Fr::rand(&mut rng).to_bigint()).collect();

    let mut count = 0;
    c.bench_function("bls12_377: fr_from_bigint", |c| {
        c.iter(|| {
            count = (count + 1) % SAMPLES;
            Fr::from_bigint(v[count])
        })
    });
}
