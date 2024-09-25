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

use snarkvm_algorithms::msm::*;
use snarkvm_curves::AffineCurve;
use snarkvm_fields::PrimeField;
use snarkvm_utilities::TestRng;

use criterion::Criterion;

#[macro_use]
extern crate criterion;

fn create_scalar_bases<G: AffineCurve<ScalarField = F>, F: PrimeField>(size: usize) -> (Vec<G>, Vec<F::BigInteger>) {
    let mut rng = TestRng::default();

    let bases = std::iter::repeat((0..(size / 1000)).map(|_| G::rand(&mut rng)).collect::<Vec<_>>())
        .take(1000)
        .flatten()
        .collect::<Vec<_>>();
    let scalars = (0..size).map(|_| F::rand(&mut rng).to_bigint()).collect::<Vec<_>>();
    (bases, scalars)
}

fn variable_base_bls12_377(c: &mut Criterion) {
    use snarkvm_curves::bls12_377::{Fr, G1Affine};
    let (bases, scalars) = create_scalar_bases::<G1Affine, Fr>(2000000);

    for size in [10_000, 100_000, 200_000, 300_000, 400_000, 500_000, 1_000_000, 2_000_000] {
        c.bench_function(&format!("VariableBase MSM on BLS12-377 ({size})"), |b| {
            b.iter(|| VariableBase::msm(&bases[..size], &scalars[..size]))
        });
    }
}

fn variable_base_edwards_bls12(c: &mut Criterion) {
    use snarkvm_curves::edwards_bls12::{EdwardsAffine, Fr};
    let (bases, scalars) = create_scalar_bases::<EdwardsAffine, Fr>(1_000_000);

    for size in [10_000, 100_000, 1_000_000] {
        c.bench_function(&format!("Variable MSM on Edwards-BLS12 ({size})"), |b| {
            b.iter(|| VariableBase::msm(&bases[..size], &scalars[..size]))
        });
    }
}

criterion_group! {
    name = variable_base_group;
    config = Criterion::default().sample_size(10);
    targets = variable_base_bls12_377, variable_base_edwards_bls12
}

criterion_main!(variable_base_group);
