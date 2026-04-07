// Copyright (c) 2019-2026 Provable Inc.
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

use super::*;

use console::{
    account::ViewKey,
    network::ConsensusVersion,
    program::{Plaintext, Record},
};
use snarkvm_algorithms::snark::varuna::VarunaVersion;
use snarkvm_synthesizer_process::InclusionVersion;

fn verify_execution_proof(vm: &VM<CurrentNetwork, LedgerType>, transaction: &Transaction<CurrentNetwork>) {
    let execution = match transaction {
        Transaction::Execute(_, _, execution, _) => execution,
        _ => panic!("Expected execute transaction"),
    };

    let block_height = vm.block_store().current_block_height();
    let consensus_version = CurrentNetwork::CONSENSUS_VERSION(block_height).unwrap();
    let varuna_version = match (ConsensusVersion::V1..=ConsensusVersion::V3).contains(&consensus_version) {
        true => VarunaVersion::V1,
        false => VarunaVersion::V2,
    };
    let is_network_behind_upgrade_height = block_height < CurrentNetwork::INCLUSION_UPGRADE_HEIGHT().unwrap();
    let inclusion_version = match (ConsensusVersion::V1..=ConsensusVersion::V7).contains(&consensus_version)
        || is_network_behind_upgrade_height
    {
        true => InclusionVersion::V0,
        false => InclusionVersion::V1,
    };

    vm.process().read().verify_execution(consensus_version, varuna_version, inclusion_version, execution).unwrap();
}

fn mint_credits_record(
    vm: &VM<CurrentNetwork, LedgerType>,
    caller_private_key: &PrivateKey<CurrentNetwork>,
    caller_view_key: &ViewKey<CurrentNetwork>,
    caller_address: &Address<CurrentNetwork>,
    rng: &mut TestRng,
) -> Record<CurrentNetwork, Plaintext<CurrentNetwork>> {
    let tx = vm
        .execute(
            caller_private_key,
            ("credits.aleo", "transfer_public_to_private"),
            vec![
                Value::<CurrentNetwork>::from_str(&caller_address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str("1_000_000u64").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    add_and_test(vm, caller_private_key, &[tx.clone()], rng);

    let output = tx.transitions().next().unwrap().outputs().iter().next().unwrap();
    match output {
        Output::Record(_, _, record_ciphertext, _) => record_ciphertext.as_ref().unwrap().decrypt(caller_view_key).unwrap(),
        _ => panic!("Expected record output"),
    }
}

#[test]
fn test_credits_methods_proof_correctness() {
    let rng = &mut TestRng::default();

    // Initialize the VM at the V14 height.
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();
    let vm = sample_vm_at_height(v14_height, rng);

    // Initialize the genesis caller.
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();
    let caller_address = Address::try_from(&caller_private_key).unwrap();

    // Mint fresh private credits records on-chain for private-path methods.
    let mut records = Vec::with_capacity(5);
    for _ in 0..5 {
        records.push(mint_credits_record(&vm, &caller_private_key, &caller_view_key, &caller_address, rng));
    }
    let mut records = records.into_iter();

    // transfer_public
    let transfer_public = vm
        .execute(
            &caller_private_key,
            ("credits.aleo", "transfer_public"),
            vec![
                Value::<CurrentNetwork>::from_str(&caller_address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str("1u64").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    verify_execution_proof(&vm, &transfer_public);

    // transfer_private
    let transfer_private = vm
        .execute(
            &caller_private_key,
            ("credits.aleo", "transfer_private"),
            vec![
                Value::<CurrentNetwork>::Record(records.next().unwrap()),
                Value::<CurrentNetwork>::from_str(&caller_address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str("1u64").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    verify_execution_proof(&vm, &transfer_private);

    // transfer_public_to_private
    let transfer_public_to_private = vm
        .execute(
            &caller_private_key,
            ("credits.aleo", "transfer_public_to_private"),
            vec![
                Value::<CurrentNetwork>::from_str(&caller_address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str("1u64").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    verify_execution_proof(&vm, &transfer_public_to_private);

    // transfer_private_to_public
    let transfer_private_to_public = vm
        .execute(
            &caller_private_key,
            ("credits.aleo", "transfer_private_to_public"),
            vec![
                Value::<CurrentNetwork>::Record(records.next().unwrap()),
                Value::<CurrentNetwork>::from_str(&caller_address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str("1u64").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    verify_execution_proof(&vm, &transfer_private_to_public);

    // join
    let join = vm
        .execute(
            &caller_private_key,
            ("credits.aleo", "join"),
            vec![
                Value::<CurrentNetwork>::Record(records.next().unwrap()),
                Value::<CurrentNetwork>::Record(records.next().unwrap()),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    verify_execution_proof(&vm, &join);

    // split
    let split = vm
        .execute(
            &caller_private_key,
            ("credits.aleo", "split"),
            vec![
                Value::<CurrentNetwork>::Record(records.next().unwrap()),
                Value::<CurrentNetwork>::from_str("1u64").unwrap(),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    verify_execution_proof(&vm, &split);
}
