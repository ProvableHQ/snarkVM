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

use snarkvm_algorithms::{
    r1cs::{ConstraintCounter, ConstraintSynthesizer},
    snark::provekit::{
        PoseidonPermutationCircuit,
        ProvekitSNARK,
        SynthesizedCircuit,
        WhirR1CSProof,
        WhirR1CSScheme,
        proof_size,
        synthesize,
    },
};
use snarkvm_curves::bls12_377::Fr;
use snarkvm_utilities::{TestRng, Uniform};

use criterion::{BenchmarkId, Criterion};
use std::time::Duration;

/// Counts R1CS constraints for `n` chained Poseidon permutations.
fn count_constraints(seed: Fr, n: usize) -> usize {
    let circuit = PoseidonPermutationCircuit::new(seed, n);
    let mut cs = ConstraintCounter::default();
    circuit.generate_constraints(&mut cs).expect("poseidon circuit should synthesize");
    cs.num_constraints
}

/// Chooses `N` so the Poseidon circuit lands near `target` constraints,
/// matching ProofBench's linear calibration for ProveKit poseidon2.
fn permutations_for_target(seed: Fr, target: usize) -> usize {
    let c1 = count_constraints(seed, 1);
    let c2 = count_constraints(seed, 2);
    let slope = c2.saturating_sub(c1);
    let intercept = c1.saturating_sub(slope);
    if slope == 0 {
        return 1;
    }
    let n = ((target.saturating_sub(intercept) as f64) / (slope as f64)).round() as usize;
    n.max(1)
}

fn prepare(
    target_constraints: usize,
) -> (WhirR1CSScheme<snarkvm_algorithms::snark::provekit::Bls12_377Field>, SynthesizedCircuit, usize, usize) {
    let rng = &mut TestRng::default();
    let seed = Fr::rand(rng);
    let n = permutations_for_target(seed, target_constraints);
    let circuit = PoseidonPermutationCircuit::new(seed, n);
    let synthesized = synthesize(&circuit).expect("poseidon circuit should synthesize");
    let constraints = synthesized.r1cs.num_constraints();
    println!(
        "poseidon target={target_constraints} permutations={n} constraints={constraints} (proofbench-style chained Poseidon-2)"
    );
    let scheme = ProvekitSNARK::setup(&synthesized.r1cs);
    (scheme, synthesized, n, constraints)
}

fn provekit_prover(c: &mut Criterion) {
    let mut group = c.benchmark_group("provekit_prover");
    for size in [1 << 14, 1 << 16] {
        let (scheme, synthesized, n, constraints) = prepare(size);
        let id = format!("{size}/n={n}/c={constraints}");
        if size == 1 << 16 {
            group.sample_size(10);
            group.measurement_time(Duration::from_secs(60));
            group.warm_up_time(Duration::from_secs(5));
        } else {
            group.sample_size(10);
            group.measurement_time(Duration::from_secs(30));
        }
        group.bench_function(BenchmarkId::from_parameter(id), |b| {
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
        let (scheme, synthesized, n, constraints) = prepare(size);
        let proof =
            ProvekitSNARK::prove(&scheme, &synthesized.r1cs, synthesized.witness.clone(), &synthesized.public_inputs)
                .unwrap();
        let bytes = proof_size(&proof);
        println!("provekit_proof_size target={size} n={n} constraints={constraints}: {bytes} bytes");
        let id = format!("{size}/n={n}/c={constraints}");
        group.bench_function(BenchmarkId::from_parameter(id), |b| b.iter(|| proof_size(&proof)));
    }
    group.finish();
}

fn provekit_verifier(c: &mut Criterion) {
    let mut group = c.benchmark_group("provekit_verifier");
    for size in [1 << 14, 1 << 16] {
        let (scheme, synthesized, n, constraints) = prepare(size);
        let proof: WhirR1CSProof =
            ProvekitSNARK::prove(&scheme, &synthesized.r1cs, synthesized.witness.clone(), &synthesized.public_inputs)
                .unwrap();
        let id = format!("{size}/n={n}/c={constraints}");
        if size == 1 << 16 {
            group.sample_size(10);
            group.measurement_time(Duration::from_secs(20));
        } else {
            group.sample_size(10);
        }
        group.bench_function(BenchmarkId::from_parameter(id), |b| {
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
