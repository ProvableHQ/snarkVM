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

#![allow(clippy::cast_possible_truncation)]

use std::{collections::HashSet, sync::Arc};

use super::*;

use crate::vm::test_helpers::{CurrentNetwork, LedgerType, sample_genesis_private_key, sample_vm_at_height};

use console::{
    account::{Address, PrivateKey},
    network::ConsensusVersion,
    prelude::FromStr,
    program::Value,
};
use snarkvm_ledger_block::{Deployment, Solutions, Transaction};
use snarkvm_ledger_narwhal_subdag::test_helpers::subdag_with_cert_count;
use snarkvm_synthesizer_program::{FinalizeGlobalState, Program};
use snarkvm_synthesizer_snark::VerifyingKey;
use snarkvm_utilities::{TestRng, try_vm_runtime};

use super::test_v14::add_and_test_with_costs;

// Construct `count` deployer keys and fund them so more than one deployment can fit in a single block.
fn fund_deployer_keys(
    vm: &VM<CurrentNetwork, LedgerType>,
    genesis_private_key: &PrivateKey<CurrentNetwork>,
    genesis_address: &Address<CurrentNetwork>,
    count: usize,
    rng: &mut TestRng,
) -> Vec<PrivateKey<CurrentNetwork>> {
    let funds_per_deployer: usize = 4_000_000_000_000;

    let mut deployer_keys = Vec::with_capacity(count);

    while deployer_keys.len() < count {
        let candidate = PrivateKey::new(rng).unwrap();
        if !deployer_keys.contains(&candidate) {
            deployer_keys.push(candidate);
        }
    }

    for chunk in deployer_keys.chunks(VM::<CurrentNetwork, LedgerType>::MAXIMUM_CONFIRMED_TRANSACTIONS) {
        let funding_transactions: Vec<_> = chunk
            .iter()
            .map(|deployer_key| {
                let deployer_address = Address::try_from(deployer_key).unwrap();
                let inputs = [
                    Value::<CurrentNetwork>::from_str(&format!("{deployer_address}")).unwrap(),
                    Value::from_str(&format!("{funds_per_deployer}u64")).unwrap(),
                ];
                vm.execute(genesis_private_key, ("credits.aleo", "transfer_public"), inputs.iter(), None, 0, None, rng)
                    .unwrap()
            })
            .collect();

        add_and_test_with_costs(vm, genesis_private_key, genesis_address, None, &funding_transactions, rng);
    }

    deployer_keys
}

/// Samples `num_deployments` deployments, each signed and paid by a distinct private key.
fn sample_deployments(
    multipliers: Vec<usize>,
    name_prefix: &str,
    vm: &VM<CurrentNetwork, LedgerType>,
    deployer_keys: &mut Vec<PrivateKey<CurrentNetwork>>,
    rng: &mut TestRng,
) -> Vec<Transaction<CurrentNetwork>> {
    // Set to true to print the combined density of each individual deployment.
    let verbose = false;

    multipliers
        .into_iter()
        .enumerate()
        .map(|(i, multiplier)| {
            let program = program_from_multiplier(multiplier, name_prefix, i);
            let private_key = &deployer_keys.pop().unwrap();

            let deployment = vm.deploy(private_key, &program, None, 0, None, rng).unwrap();

            if verbose {
                println!(
                    "  Deployment with multiplier {multiplier}, combined density {}",
                    deployment.deployment().unwrap().combined_density()
                );
            }

            deployment
        })
        .collect()
}

fn program_from_multiplier(multiplier: usize, name_prefix: &str, suffix: usize) -> Program<CurrentNetwork> {
    let mut program_str = format!(
        r"
    program {name_prefix}_{suffix}.aleo;

    function fun:
        input r0 as [field; 32u32].public;
"
    );

    for j in 1..multiplier {
        program_str += &format!(
            r"
        hash.bhp256 r0 into r{j} as field;
    "
        );
    }

    program_str += r"
    constructor:
        assert.eq true true;
    ";

    Program::from_str(&program_str).unwrap()
}

/// Extracts the message from a panic payload caught by [`try_vm_runtime`].
fn vm_halt_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| format!("unexpected panic payload: {payload:?}"))
}

/// Checks that the block-wide synthesis limit is computed and enforced correctly.
#[test]
fn test_blockwide_limits() {
    let current_max_certificates = CurrentNetwork::LATEST_MAX_CERTIFICATES() as f64;

    let rng = &mut TestRng::default();

    let v15_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap();
    let vm = sample_vm_at_height(v15_height, rng);
    let genesis_private_key = sample_genesis_private_key(rng);
    let genesis_address = Address::try_from(&genesis_private_key).unwrap();

    // Each entry has the form (c, ms, as) with
    // - c: number of certificates
    // - ms: circuit-size multipliers, one per deployment
    // - as: whether each of the deployments is expected to be aborted
    let cases = vec![
        // Synthesis limit = 16_777_210, densities: [4_187_104, 4_187_104, 4_187_104, 4_187_104], total 16_748_416 below limit
        (2 * current_max_certificates as u64, vec![16; 4], vec![false; 4]),
        // Synthesis limit = 16_777_210, densities: [4_187_104, 4_187_104, 4_187_104, 4_187_104, 4_187_104], the last one goes over the limit
        (2 * current_max_certificates as u64, vec![16; 5], vec![false, false, false, false, true]),
        // Synthesis limit = 18_390_403, densities: [16_967_680]
        ((2.2 * current_max_certificates) as u64, vec![64], vec![false; 1]),
        // Synthesis limit = 18_390_403, densities: [8_447_296, 8_447_296, 8_447_296], the third one goes over the limit
        ((2.2 * current_max_certificates) as u64, vec![32, 32, 32], vec![false, false, true]),
        // Synthesis limit = 33554420, densities: [8_447_296, 8_447_296, 8_447_296], the third one now fits thanks to the increased limit
        (4 * current_max_certificates as u64, vec![32, 32, 32], vec![false, false, false]),
        // Synthesis limit = 16_777_210, densities: [4_187_104, 8_447_296, 4_187_104, 2_057_008, 4_187_104, 2_057_008], the third and fifth go over the limit, fourth and sixth still fit
        (2 * current_max_certificates as u64, vec![16, 32, 16, 8, 16, 8], vec![false, false, true, false, true, false]),
    ];

    let num_deployer_keys = cases.iter().map(|(_, ms, _)| ms.len()).sum::<usize>();

    let mut deployer_keys = fund_deployer_keys(&vm, &genesis_private_key, &genesis_address, num_deployer_keys, rng);

    let block_hash = vm.block_store().get_block_hash(vm.block_store().max_height().unwrap()).unwrap().unwrap();
    let previous_block = vm.block_store().get_block(&block_hash).unwrap().unwrap();
    let next_block_height = previous_block.height().saturating_add(1);

    for (i, (num_certs, multipliers, aborted)) in cases.into_iter().enumerate() {
        println!("Sampling subdag at height {next_block_height}");
        let subdag = subdag_with_cert_count(num_certs as usize, rng);
        let num_deployments = multipliers.len();

        let synthesis_limit = subdag.synthesis_limit(next_block_height).expect("Synthesis limit in >= V15");

        let name_prefix = format!("test_synthesis_{i}");

        println!("Sampling deployments with multipliers: {multipliers:?}");
        let deployments = sample_deployments(multipliers, &name_prefix, &vm, &mut deployer_keys, rng);

        let next_timestamp = previous_block.timestamp().saturating_add(CurrentNetwork::BLOCK_TIME as i64);
        let next_timestamp = (next_block_height
            >= CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V12).unwrap_or_default())
        .then_some(next_timestamp);

        println!("Sampling finalize state");
        let finalize_state = FinalizeGlobalState::from(
            previous_block.round().saturating_add(1),
            next_block_height,
            next_timestamp,
            [0u8; 32],
            subdag.spend_limit(next_block_height),
            Some(synthesis_limit),
        );

        println!("Speculating");
        let (ratifications, confirmed_transactions, aborted_transaction_ids, _finalize_operations) = vm
            .speculate(
                finalize_state,
                CurrentNetwork::BLOCK_TIME as i64,
                None,
                Vec::new(),
                &Solutions::from(None),
                deployments.iter(),
                rng,
            )
            .unwrap();

        // The first `num_deployments - num_aborted` deployments are expected to be accepted, the rest aborted.
        let expected_aborted_transaction_ids = deployments
            .iter()
            .zip(aborted.iter())
            .filter_map(|(deployment, should_be_aborted)| should_be_aborted.then_some(deployment.id()))
            .collect::<HashSet<_>>();

        println!("Synthesis limit: {synthesis_limit}\n");

        assert_eq!(ratifications.len(), 0);
        assert_eq!(confirmed_transactions.num_accepted(), num_deployments - expected_aborted_transaction_ids.len());
        assert_eq!(confirmed_transactions.num_rejected(), 0);
        assert_eq!(HashSet::from_iter(aborted_transaction_ids), expected_aborted_transaction_ids);
    }
}

/// Checks that, during synthesis, if the running density of one of the circuit matrices
/// surpasses the total claimed in the verifying key, synthesis stops.
#[test]
fn test_vk_num_non_zero_detected() {
    let rng = &mut TestRng::default();

    let v15_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap();
    let vm = sample_vm_at_height(v15_height, rng);
    let genesis_private_key = sample_genesis_private_key(rng);

    for (i, multiplier) in (1..=4).map(|i| (i, 1 << i)) {
        let program = program_from_multiplier(multiplier, "test", i);
        let transaction = vm.deploy(&genesis_private_key, &program, None, 0, None, rng).unwrap();

        let Transaction::Deploy(_, _, _, deployment, _) = transaction else {
            panic!("expected a deployment transaction");
        };

        assert!(deployment.verifying_keys().len() == 1);

        let (function_id, (vk, certificate)) = &deployment.verifying_keys()[0];

        for tamper_with in ["a", "b", "c"] {
            let mut circuit_vk = vk.deref().clone();
            assert!(
                circuit_vk.circuit_info.num_non_zero_a >= 1,
                "multiplier {multiplier}: num_non_zero_a must be at least 1 to under-report"
            );

            match tamper_with {
                "a" => circuit_vk.circuit_info.num_non_zero_a -= 1,
                "b" => circuit_vk.circuit_info.num_non_zero_b -= 1,
                "c" => circuit_vk.circuit_info.num_non_zero_c -= 1,
                _ => panic!("tamper_with must be a, b or c, got {tamper_with}"),
            }

            let tampered_vks = vec![(
                *function_id,
                (VerifyingKey::new(Arc::new(circuit_vk), vk.num_variables()), certificate.clone()),
            )];

            let tampered_deployment = Deployment::new(
                deployment.edition(),
                deployment.program().clone(),
                tampered_vks,
                deployment.program_checksum(),
                deployment.program_owner(),
            )
            .unwrap();

            // check_transaction uses try_vm_runtime! and replaces the halt panic with a generic message.
            // We call the latter directly to receive the finer-grained error.
            let verification_result = try_vm_runtime!(|| {
                vm.process().verify_deployment::<CurrentAleo, _>(ConsensusVersion::V15, &tampered_deployment, rng)
            });
            let error_message = match verification_result {
                Ok(Ok(())) => panic!("Expected deployment verification to fail (matrices are denser than vk claims)"),
                Ok(Err(error)) => error.to_string(),
                Err(payload) => vm_halt_message(payload),
            };

            assert!(error_message.contains("Surpassed the circuit density limit"));
        }
    }
}
