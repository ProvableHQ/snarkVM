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

//! Callgrind measurements of the BLS12-377 operations in `curves.rs`.
//!
//! Each function measures one operation on a fixed input, the body of the criterion bench with the same name.
//! Rayon is pinned to one worker so the instruction count does not follow the pool size.

use snarkvm_curves::{
    AffineCurve,
    bls12_377::{
        Bls12_377,
        Bls12_377Parameters,
        Fq,
        Fq2,
        Fq12,
        Fr,
        G1Affine,
        G1Projective as G1,
        G2Affine,
        G2Projective as G2,
    },
    templates::bls12::{G1Prepared, G2Prepared},
    traits::{PairingCurve, PairingEngine, ProjectiveCurve},
};
use snarkvm_fields::{Field, PrimeField, SquareRootField};
use snarkvm_utilities::{
    biginteger::{BigInteger, BigInteger256 as FrRepr, BigInteger384 as FqRepr},
    rand::{TestRng, Uniform},
};

use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::{
    hint::black_box,
    iter,
    ops::{AddAssign, MulAssign, SubAssign},
};

const SEED: u64 = 0xB15_12377;

fn rng() -> TestRng {
    TestRng::from_seed(SEED)
}

fn setup_fq_repr_add() -> (FqRepr, FqRepr) {
    let mut rng = rng();
    let mut left = FqRepr::rand(&mut rng);
    let mut right = FqRepr::rand(&mut rng);
    for _ in 0..3 {
        left.div2();
        right.div2();
    }
    (left, right)
}

fn setup_fq_repr_sub() -> (FqRepr, FqRepr) {
    let mut rng = rng();
    let left = FqRepr::rand(&mut rng);
    let mut right = left;
    for _ in 0..10 {
        right.div2();
    }
    (left, right)
}

fn setup_fq_repr() -> FqRepr {
    FqRepr::rand(&mut rng())
}

fn setup_fq_pair() -> (Fq, Fq) {
    let mut rng = rng();
    (Fq::rand(&mut rng), Fq::rand(&mut rng))
}

fn setup_fq() -> Fq {
    Fq::rand(&mut rng())
}

fn setup_fq_square() -> Fq {
    let mut value = setup_fq();
    value.square_in_place();
    value
}

fn setup_fr_repr_add() -> (FrRepr, FrRepr) {
    let mut rng = rng();
    let mut left = FrRepr::rand(&mut rng);
    let mut right = FrRepr::rand(&mut rng);
    for _ in 0..3 {
        left.div2();
        right.div2();
    }
    (left, right)
}

fn setup_fr_repr_sub() -> (FrRepr, FrRepr) {
    let mut rng = rng();
    let left = FrRepr::rand(&mut rng);
    let mut right = left;
    for _ in 0..10 {
        right.div2();
    }
    (left, right)
}

fn setup_fr_repr() -> FrRepr {
    FrRepr::rand(&mut rng())
}

fn setup_fr_pair() -> (Fr, Fr) {
    let mut rng = rng();
    (Fr::rand(&mut rng), Fr::rand(&mut rng))
}

fn setup_fr() -> Fr {
    Fr::rand(&mut rng())
}

fn setup_fr_square() -> Fr {
    let mut value = setup_fr();
    value.square_in_place();
    value
}

fn setup_fq2_pair() -> (Fq2, Fq2) {
    let mut rng = rng();
    (Fq2::rand(&mut rng), Fq2::rand(&mut rng))
}

fn setup_fq2() -> Fq2 {
    Fq2::rand(&mut rng())
}

fn setup_fq12_pair() -> (Fq12, Fq12) {
    let mut rng = rng();
    (Fq12::rand(&mut rng), Fq12::rand(&mut rng))
}

fn setup_fq12() -> Fq12 {
    Fq12::rand(&mut rng())
}

fn setup_g1_fr() -> (G1, Fr) {
    let mut rng = rng();
    (G1::rand(&mut rng), Fr::rand(&mut rng))
}

fn setup_g1_pair() -> (G1, G1) {
    let mut rng = rng();
    (G1::rand(&mut rng), G1::rand(&mut rng))
}

fn setup_g1() -> G1 {
    G1::rand(&mut rng())
}

fn setup_g1_mixed() -> (G1, G1Affine) {
    let mut rng = rng();
    (G1::rand(&mut rng), G1::rand(&mut rng).into())
}

fn setup_g1_affine() -> G1Affine {
    let point = setup_g1();
    G1::batch_normalization_into_affine(vec![point]).pop().unwrap()
}

fn setup_g2_fr() -> (G2, Fr) {
    let mut rng = rng();
    (G2::rand(&mut rng), Fr::rand(&mut rng))
}

fn setup_g2_pair() -> (G2, G2) {
    let mut rng = rng();
    (G2::rand(&mut rng), G2::rand(&mut rng))
}

fn setup_g2() -> G2 {
    G2::rand(&mut rng())
}

fn setup_g2_mixed() -> (G2, G2Affine) {
    let mut rng = rng();
    (G2::rand(&mut rng), G2::rand(&mut rng).into())
}

fn setup_prepared() -> (G1Prepared<Bls12_377Parameters>, G2Prepared<Bls12_377Parameters>) {
    let mut rng = rng();
    (G1Affine::from(G1::rand(&mut rng)).prepare(), G2Affine::from(G2::rand(&mut rng)).prepare())
}

fn setup_miller() -> Fq12 {
    let (p, q) = setup_prepared();
    Bls12_377::miller_loop(iter::once((&p, &q)))
}

fn setup_pairing_points() -> (G1, G2) {
    let mut rng = rng();
    (G1::rand(&mut rng), G2::rand(&mut rng))
}

#[library_benchmark]
fn g1_rand() -> G1 {
    black_box(G1::rand(&mut rng()))
}

#[library_benchmark]
#[bench::once(setup_g1_fr())]
fn g1_mul_assign(input: (G1, Fr)) -> G1 {
    let (mut point, scalar) = black_box(input);
    point.mul_assign(scalar);
    black_box(point)
}

#[library_benchmark]
#[bench::once(setup_g1_pair())]
fn g1_add_assign(input: (G1, G1)) -> G1 {
    let (mut point, other) = black_box(input);
    point.add_assign(other);
    black_box(point)
}

#[library_benchmark]
#[bench::once(setup_g1_mixed())]
fn g1_add_assign_mixed(input: (G1, G1Affine)) -> G1 {
    let (mut point, other) = black_box(input);
    point.add_assign_mixed(&other);
    black_box(point)
}

#[library_benchmark]
#[bench::once(setup_g1())]
fn g1_double(point: G1) -> G1 {
    let mut point = black_box(point);
    point.double_in_place();
    black_box(point)
}

#[library_benchmark]
#[bench::once(setup_g1_affine())]
fn g1_is_in_correct_subgroup(point: G1Affine) -> bool {
    black_box(point.is_in_correct_subgroup_assuming_on_curve())
}

#[library_benchmark]
fn g2_rand() -> G2 {
    black_box(G2::rand(&mut rng()))
}

#[library_benchmark]
#[bench::once(setup_g2_fr())]
fn g2_mul_assign(input: (G2, Fr)) -> G2 {
    let (mut point, scalar) = black_box(input);
    point.mul_assign(scalar);
    black_box(point)
}

#[library_benchmark]
#[bench::once(setup_g2_pair())]
fn g2_add_assign(input: (G2, G2)) -> G2 {
    let (mut point, other) = black_box(input);
    point.add_assign(other);
    black_box(point)
}

#[library_benchmark]
#[bench::once(setup_g2_mixed())]
fn g2_add_assign_mixed(input: (G2, G2Affine)) -> G2 {
    let (mut point, other) = black_box(input);
    point.add_assign_mixed(&other);
    black_box(point)
}

#[library_benchmark]
#[bench::once(setup_g2())]
fn g2_double(point: G2) -> G2 {
    let mut point = black_box(point);
    point.double_in_place();
    black_box(point)
}

#[library_benchmark]
#[bench::once(setup_fq_repr_add())]
fn fq_repr_add_nocarry(input: (FqRepr, FqRepr)) -> FqRepr {
    let (mut left, right) = black_box(input);
    black_box(left.add_nocarry(&right));
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fq_repr_sub())]
fn fq_repr_sub_noborrow(input: (FqRepr, FqRepr)) -> FqRepr {
    let (mut left, right) = black_box(input);
    black_box(left.sub_noborrow(&right));
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fq_repr())]
fn fq_repr_num_bits(value: FqRepr) -> u32 {
    black_box(value.num_bits())
}

#[library_benchmark]
#[bench::once(setup_fq_repr())]
fn fq_repr_mul2(value: FqRepr) -> FqRepr {
    let mut value = black_box(value);
    value.mul2();
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fq_repr())]
fn fq_repr_div2(value: FqRepr) -> FqRepr {
    let mut value = black_box(value);
    value.div2();
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fq_pair())]
fn fq_add_assign(input: (Fq, Fq)) -> Fq {
    let (mut left, right) = black_box(input);
    left.add_assign(right);
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fq_pair())]
fn fq_sub_assign(input: (Fq, Fq)) -> Fq {
    let (mut left, right) = black_box(input);
    left.sub_assign(&right);
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fq_pair())]
fn fq_mul_assign(input: (Fq, Fq)) -> Fq {
    let (mut left, right) = black_box(input);
    left.mul_assign(&right);
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fq())]
fn fq_double(value: Fq) -> Fq {
    let mut value = black_box(value);
    value.double_in_place();
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fq())]
fn fq_square(value: Fq) -> Fq {
    let mut value = black_box(value);
    value.square_in_place();
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fq())]
fn fq_inverse(value: Fq) -> Option<Fq> {
    black_box(value.inverse())
}

#[library_benchmark]
#[bench::once(setup_fq())]
fn fq_negate(value: Fq) -> Fq {
    let mut value = black_box(value);
    value = -value;
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fq_square())]
fn fq_sqrt(value: Fq) -> Option<Fq> {
    black_box(value.sqrt())
}

#[library_benchmark]
#[bench::once(setup_fq())]
fn fq_to_bigint(value: Fq) -> <Fq as PrimeField>::BigInteger {
    black_box(value.to_bigint())
}

#[library_benchmark]
#[bench::once(setup_fq_repr())]
fn fq_from_bigint(value: FqRepr) -> Option<Fq> {
    black_box(Fq::from_bigint(black_box(value)))
}

#[library_benchmark]
#[bench::once(setup_fq2_pair())]
fn fq2_add_assign(input: (Fq2, Fq2)) -> Fq2 {
    let (mut left, right) = black_box(input);
    left.add_assign(right);
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fq2_pair())]
fn fq2_sub_assign(input: (Fq2, Fq2)) -> Fq2 {
    let (mut left, right) = black_box(input);
    left.sub_assign(&right);
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fq2_pair())]
fn fq2_mul_assign(input: (Fq2, Fq2)) -> Fq2 {
    let (mut left, right) = black_box(input);
    left.mul_assign(&right);
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fq2())]
fn fq2_double(value: Fq2) -> Fq2 {
    let mut value = black_box(value);
    value.double_in_place();
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fq2())]
fn fq2_square(value: Fq2) -> Fq2 {
    let mut value = black_box(value);
    value.square_in_place();
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fq2())]
fn fq2_inverse(value: Fq2) -> Option<Fq2> {
    black_box(value.inverse())
}

#[library_benchmark]
#[bench::once(setup_fq2())]
fn fq2_sqrt(value: Fq2) -> Option<Fq2> {
    black_box(value.sqrt())
}

#[library_benchmark]
#[bench::once(setup_fq12_pair())]
fn fq12_add_assign(input: (Fq12, Fq12)) -> Fq12 {
    let (mut left, right) = black_box(input);
    left.add_assign(right);
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fq12_pair())]
fn fq12_sub_assign(input: (Fq12, Fq12)) -> Fq12 {
    let (mut left, right) = black_box(input);
    left.sub_assign(&right);
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fq12_pair())]
fn fq12_mul_assign(input: (Fq12, Fq12)) -> Fq12 {
    let (mut left, right) = black_box(input);
    left.mul_assign(&right);
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fq12())]
fn fq12_double(value: Fq12) -> Fq12 {
    let mut value = black_box(value);
    value.double_in_place();
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fq12())]
fn fq12_square(value: Fq12) -> Fq12 {
    let mut value = black_box(value);
    value.square_in_place();
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fq12())]
fn fq12_inverse(value: Fq12) -> Option<Fq12> {
    black_box(value.inverse())
}

#[library_benchmark]
#[bench::once(setup_fr_repr_add())]
fn fr_repr_add_nocarry(input: (FrRepr, FrRepr)) -> FrRepr {
    let (mut left, right) = black_box(input);
    black_box(left.add_nocarry(&right));
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fr_repr_sub())]
fn fr_repr_sub_noborrow(input: (FrRepr, FrRepr)) -> FrRepr {
    let (mut left, right) = black_box(input);
    black_box(left.sub_noborrow(&right));
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fr_repr())]
fn fr_repr_num_bits(value: FrRepr) -> u32 {
    black_box(value.num_bits())
}

#[library_benchmark]
#[bench::once(setup_fr_repr())]
fn fr_repr_mul2(value: FrRepr) -> FrRepr {
    let mut value = black_box(value);
    value.mul2();
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fr_repr())]
fn fr_repr_div2(value: FrRepr) -> FrRepr {
    let mut value = black_box(value);
    value.div2();
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fr_pair())]
fn fr_add_assign(input: (Fr, Fr)) -> Fr {
    let (mut left, right) = black_box(input);
    left.add_assign(right);
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fr_pair())]
fn fr_sub_assign(input: (Fr, Fr)) -> Fr {
    let (mut left, right) = black_box(input);
    left.sub_assign(&right);
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fr_pair())]
fn fr_mul_assign(input: (Fr, Fr)) -> Fr {
    let (mut left, right) = black_box(input);
    left.mul_assign(&right);
    black_box(left)
}

#[library_benchmark]
#[bench::once(setup_fr())]
fn fr_double(value: Fr) -> Fr {
    let mut value = black_box(value);
    value.double_in_place();
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fr())]
fn fr_square(value: Fr) -> Fr {
    let mut value = black_box(value);
    value.square_in_place();
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fr())]
fn fr_inverse(value: Fr) -> Option<Fr> {
    black_box(value.inverse())
}

#[library_benchmark]
#[bench::once(setup_fr())]
fn fr_negate(value: Fr) -> Fr {
    let mut value = black_box(value);
    value = -value;
    black_box(value)
}

#[library_benchmark]
#[bench::once(setup_fr_square())]
fn fr_sqrt(value: Fr) -> Option<Fr> {
    black_box(value.sqrt())
}

#[library_benchmark]
#[bench::once(setup_fr())]
fn fr_to_bigint(value: Fr) -> <Fr as PrimeField>::BigInteger {
    black_box(value.to_bigint())
}

#[library_benchmark]
#[bench::once(setup_fr_repr())]
fn fr_from_bigint(value: FrRepr) -> Option<Fr> {
    black_box(Fr::from_bigint(black_box(value)))
}

#[library_benchmark]
#[bench::once(setup_prepared())]
fn pairing_miller_loop(input: (G1Prepared<Bls12_377Parameters>, G2Prepared<Bls12_377Parameters>)) -> Fq12 {
    let (p, q) = black_box(input);
    black_box(Bls12_377::miller_loop(iter::once((&p, &q))))
}

#[library_benchmark]
#[bench::once(setup_miller())]
fn pairing_final_exponentiation(value: Fq12) -> Option<Fq12> {
    black_box(Bls12_377::final_exponentiation(&black_box(value)))
}

#[library_benchmark]
#[bench::once(setup_pairing_points())]
fn pairing_full(input: (G1, G2)) -> Fq12 {
    let (p, q) = black_box(input);
    black_box(Bls12_377::pairing(p, q))
}

library_benchmark_group!(
    name = curves,
    benchmarks = [
        g1_rand,
        g1_mul_assign,
        g1_add_assign,
        g1_add_assign_mixed,
        g1_double,
        g1_is_in_correct_subgroup,
        g2_rand,
        g2_mul_assign,
        g2_add_assign,
        g2_add_assign_mixed,
        g2_double,
        fq_repr_add_nocarry,
        fq_repr_sub_noborrow,
        fq_repr_num_bits,
        fq_repr_mul2,
        fq_repr_div2,
        fq_add_assign,
        fq_sub_assign,
        fq_mul_assign,
        fq_double,
        fq_square,
        fq_inverse,
        fq_negate,
        fq_sqrt,
        fq_to_bigint,
        fq_from_bigint,
        fq2_add_assign,
        fq2_sub_assign,
        fq2_mul_assign,
        fq2_double,
        fq2_square,
        fq2_inverse,
        fq2_sqrt,
        fq12_add_assign,
        fq12_sub_assign,
        fq12_mul_assign,
        fq12_double,
        fq12_square,
        fq12_inverse,
        fr_repr_add_nocarry,
        fr_repr_sub_noborrow,
        fr_repr_num_bits,
        fr_repr_mul2,
        fr_repr_div2,
        fr_add_assign,
        fr_sub_assign,
        fr_mul_assign,
        fr_double,
        fr_square,
        fr_inverse,
        fr_negate,
        fr_sqrt,
        fr_to_bigint,
        fr_from_bigint,
        pairing_miller_loop,
        pairing_final_exponentiation,
        pairing_full,
    ]
);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = curves
);
