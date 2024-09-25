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

use crate::msm::*;
use snarkvm_curves::{
    bls12_377::{Fr, G1Projective},
    traits::{AffineCurve, ProjectiveCurve},
};
use snarkvm_fields::{PrimeField, Zero};
use snarkvm_utilities::{
    rand::{TestRng, Uniform},
    BitIteratorBE,
};

fn naive_variable_base_msm<G: AffineCurve>(
    bases: &[G],
    scalars: &[<G::ScalarField as PrimeField>::BigInteger],
) -> G::Projective {
    let mut acc = G::Projective::zero();

    for (base, scalar) in bases.iter().zip(scalars.iter()) {
        acc += base.mul_bits(BitIteratorBE::new(*scalar));
    }
    acc
}

#[test]
fn variable_base_test_with_bls12() {
    const SAMPLES: usize = 1 << 10;

    let mut rng = TestRng::default();

    let v = (0..SAMPLES).map(|_| Fr::rand(&mut rng).to_bigint()).collect::<Vec<_>>();
    let g = (0..SAMPLES).map(|_| G1Projective::rand(&mut rng).to_affine()).collect::<Vec<_>>();

    let naive = naive_variable_base_msm(g.as_slice(), v.as_slice());
    let fast = VariableBase::msm(g.as_slice(), v.as_slice());

    assert_eq!(naive.to_affine(), fast.to_affine());
}

#[test]
fn variable_base_test_with_bls12_unequal_numbers() {
    const SAMPLES: usize = 1 << 10;

    let mut rng = TestRng::default();

    let v = (0..SAMPLES - 100).map(|_| Fr::rand(&mut rng).to_bigint()).collect::<Vec<_>>();
    let g = (0..SAMPLES).map(|_| G1Projective::rand(&mut rng).to_affine()).collect::<Vec<_>>();

    let naive = naive_variable_base_msm(g.as_slice(), v.as_slice());
    let fast = VariableBase::msm(g.as_slice(), v.as_slice());

    assert_eq!(naive.to_affine(), fast.to_affine());
}
