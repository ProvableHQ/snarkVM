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

#[macro_use]
extern crate criterion;

use snarkvm_algorithms::snark::{
    provekit::{ProvekitSNARK, SynthesizedCircuit, WhirR1CSProof, WhirR1CSScheme, proof_size, synthesize},
    varuna::TestCircuit,
};
use snarkvm_curves::bls12_377::Fr;
use snarkvm_utilities::TestRng;

use criterion::{BenchmarkId, Criterion};
use std::time::Duration;

fn prepare(
    num_constraints: usize,
) -> (WhirR1CSScheme<snarkvm_algorithms::snark::provekit::Bls12_377Field>, SynthesizedCircuit) {
    let rng = &mut TestRng::default();
    let (circuit, _) = TestCircuit::<Fr>::gen_rand(1, num_constraints, num_constraints, rng);
    let synthesized = synthesize(&circuit).expect("test circuit should synthesize");
    let scheme = ProvekitSNARK::setup(&synthesized.r1cs);
    (scheme, synthesized)
}

fn provekit_prover(c: &mut Criterion) {
    let mut group = c.benchmark_group("provekit_prover");
    for size in [1 << 14, 1 << 16] {
        let (scheme, synthesized) = prepare(size);
        if size == 1 << 16 {
            group.sample_size(10);
            group.measurement_time(Duration::from_secs(60));
            group.warm_up_time(Duration::from_secs(5));
        } else {
            group.sample_size(10);
            group.measurement_time(Duration::from_secs(30));
        }
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| {
                ProvekitSNARK::prove(
                    &scheme,
                    &synthesized.r1cs,
                    synthesized.witness.clone(),
                    &synthesized.public_inputs,
                )
                .unwrap()
            })
        });
    }
    group.finish();
}

fn provekit_proof_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("provekit_proof_size");
    group.sample_size(10);
    for size in [1 << 14, 1 << 16] {
        let (scheme, synthesized) = prepare(size);
        let proof =
            ProvekitSNARK::prove(&scheme, &synthesized.r1cs, synthesized.witness.clone(), &synthesized.public_inputs)
                .unwrap();
        let bytes = proof_size(&proof);
        println!("provekit_proof_size_{size}: {bytes} bytes");
        group.bench_function(BenchmarkId::from_parameter(size), |b| b.iter(|| proof_size(&proof)));
    }
    group.finish();
}

fn provekit_verifier(c: &mut Criterion) {
    let mut group = c.benchmark_group("provekit_verifier");
    for size in [1 << 14, 1 << 16] {
        let (scheme, synthesized) = prepare(size);
        let proof: WhirR1CSProof =
            ProvekitSNARK::prove(&scheme, &synthesized.r1cs, synthesized.witness.clone(), &synthesized.public_inputs)
                .unwrap();
        if size == 1 << 16 {
            group.sample_size(10);
            group.measurement_time(Duration::from_secs(20));
        } else {
            group.sample_size(10);
        }
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| ProvekitSNARK::verify(&scheme, &synthesized.r1cs, &synthesized.public_inputs, &proof).unwrap())
        });
    }
    group.finish();
}

criterion_group! {
    name = provekit_benches;
    config = Criterion::default().sample_size(10);
    targets = provekit_prover, provekit_proof_size, provekit_verifier
}
criterion_main!(provekit_benches);
