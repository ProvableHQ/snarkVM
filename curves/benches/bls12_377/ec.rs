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

pub(crate) mod g1 {
    use snarkvm_curves::{
        bls12_377::{Fr, G1Affine, G1Projective as G1},
        traits::ProjectiveCurve,
        AffineCurve,
    };
    use snarkvm_utilities::rand::{TestRng, Uniform};

    use criterion::Criterion;
    use std::ops::{AddAssign, MulAssign};

    pub fn bench_g1_rand(c: &mut Criterion) {
        let mut rng = TestRng::default();
        c.bench_function("bls12_377: g1_rand", |c| c.iter(|| G1::rand(&mut rng)));
    }

    pub fn bench_g1_mul_assign(c: &mut Criterion) {
        const SAMPLES: usize = 1000;

        let mut rng = TestRng::default();

        let v: Vec<(G1, Fr)> = (0..SAMPLES).map(|_| (G1::rand(&mut rng), Fr::rand(&mut rng))).collect();

        let mut count = 0;
        c.bench_function("bls12_377: g1_mul_assign", |c| {
            c.iter(|| {
                let mut tmp = v[count].0;
                tmp.mul_assign(v[count].1);
                count = (count + 1) % SAMPLES;
            })
        });
    }

    pub fn bench_g1_add_assign(c: &mut Criterion) {
        const SAMPLES: usize = 1000;

        let mut rng = TestRng::default();

        let v: Vec<(G1, G1)> = (0..SAMPLES).map(|_| (G1::rand(&mut rng), G1::rand(&mut rng))).collect();

        let mut count = 0;
        c.bench_function("bls12_377: g1_add_assign", |c| {
            c.iter(|| {
                let mut tmp = v[count].0;
                tmp.add_assign(v[count].1);
                count = (count + 1) % SAMPLES;
                tmp
            })
        });
    }

    pub fn bench_g1_add_assign_mixed(c: &mut Criterion) {
        const SAMPLES: usize = 1000;

        let mut rng = TestRng::default();

        let v: Vec<(G1, G1Affine)> = (0..SAMPLES).map(|_| (G1::rand(&mut rng), G1::rand(&mut rng).into())).collect();

        let mut count = 0;
        c.bench_function("bls12_377: g1_add_assign_mixed", |c| {
            c.iter(|| {
                let mut tmp = v[count].0;
                tmp.add_assign_mixed(&v[count].1);
                count = (count + 1) % SAMPLES;
                tmp
            })
        });
    }

    pub fn bench_g1_double(c: &mut Criterion) {
        const SAMPLES: usize = 1000;

        let mut rng = TestRng::default();

        let v: Vec<(G1, G1)> = (0..SAMPLES).map(|_| (G1::rand(&mut rng), G1::rand(&mut rng))).collect();

        let mut count = 0;
        c.bench_function("bls12_377: g1_double", |c| {
            c.iter(|| {
                let mut tmp = v[count].0;
                tmp.double_in_place();
                count = (count + 1) % SAMPLES;
                tmp
            })
        });
    }

    pub fn bench_g1_check_subgroup_membership(c: &mut Criterion) {
        const SAMPLES: usize = 1000;

        let mut rng = TestRng::default();

        let v: Vec<G1> = (0..SAMPLES).map(|_| G1::rand(&mut rng)).collect();
        let v = G1::batch_normalization_into_affine(v);

        let mut count = 0;
        c.bench_function("bls12_377: g1_is_in_correct_subgroup", |c| {
            c.iter(|| {
                let result = v[count].is_in_correct_subgroup_assuming_on_curve();
                count = (count + 1) % SAMPLES;
                result
            })
        });
    }
}

pub(crate) mod g2 {
    use snarkvm_curves::{
        bls12_377::{Fr, G2Affine, G2Projective as G2},
        traits::ProjectiveCurve,
    };
    use snarkvm_utilities::rand::{TestRng, Uniform};

    use criterion::Criterion;
    use std::ops::{AddAssign, MulAssign};

    pub fn bench_g2_rand(c: &mut Criterion) {
        let mut rng = TestRng::default();
        c.bench_function("bls12_377: g2_rand", |c| c.iter(|| G2::rand(&mut rng)));
    }

    pub fn bench_g2_mul_assign(c: &mut Criterion) {
        const SAMPLES: usize = 1000;

        let mut rng = TestRng::default();

        let v: Vec<(G2, Fr)> = (0..SAMPLES).map(|_| (G2::rand(&mut rng), Fr::rand(&mut rng))).collect();

        let mut count = 0;
        c.bench_function("bls12_377: g2_mul_assign", |c| {
            c.iter(|| {
                let mut tmp = v[count].0;
                tmp.mul_assign(v[count].1);
                count = (count + 1) % SAMPLES;
                tmp
            })
        });
    }

    pub fn bench_g2_add_assign(c: &mut Criterion) {
        const SAMPLES: usize = 1000;

        let mut rng = TestRng::default();

        let v: Vec<(G2, G2)> = (0..SAMPLES).map(|_| (G2::rand(&mut rng), G2::rand(&mut rng))).collect();

        let mut count = 0;
        c.bench_function("bls12_377: g2_add_assign", |c| {
            c.iter(|| {
                let mut tmp = v[count].0;
                tmp.add_assign(v[count].1);
                count = (count + 1) % SAMPLES;
                tmp
            })
        });
    }

    pub fn bench_g2_add_assign_mixed(c: &mut Criterion) {
        const SAMPLES: usize = 1000;

        let mut rng = TestRng::default();

        let v: Vec<(G2, G2Affine)> = (0..SAMPLES).map(|_| (G2::rand(&mut rng), G2::rand(&mut rng).into())).collect();

        let mut count = 0;
        c.bench_function("bls12_377: g2_add_assign_mixed", |c| {
            c.iter(|| {
                let mut tmp = v[count].0;
                tmp.add_assign_mixed(&v[count].1);
                count = (count + 1) % SAMPLES;
                tmp
            })
        });
    }

    pub fn bench_g2_double(c: &mut Criterion) {
        const SAMPLES: usize = 1000;

        let mut rng = TestRng::default();

        let v: Vec<(G2, G2)> = (0..SAMPLES).map(|_| (G2::rand(&mut rng), G2::rand(&mut rng))).collect();

        let mut count = 0;
        c.bench_function("bls12_377: g2_double", |c| {
            c.iter(|| {
                let mut tmp = v[count].0;
                tmp.double_in_place();
                count = (count + 1) % SAMPLES;
                tmp
            })
        });
    }
}
