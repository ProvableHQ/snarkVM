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

// TODO (Antonio) document

/*
Performs time measurements on the verification of the large one_to_many_records transaction.
 - Generate artifacts (both the blocks where programs are deployed and the transactions themselves) with:
   cargo bench --bench check_transaction_multirecord -- --generate
 - Artifacts are ignored by git. To clean them, run:
   cargo bench --bench check_transaction_multirecord -- --clean
 - Obtain time measurements with:
   cargo bench --bench check_transaction_multirecord
   The --serial feature can be added to deactivate parallelism.
 - Flamegraph with:
   cargo flamegraph --bench check_transaction_multirecord --features serial
*/

use std::time::Instant;

use snarkvm_console::{
    account::PrivateKey,
    network::{
        MainnetV0,
        prelude::{ConsensusVersion, CryptoRng, FromStr, Network, Result, Rng, TestRng, Zero},
    },
    types::Field,
};
use snarkvm_ledger_block::{Block, Header, Metadata, Transaction};
use snarkvm_ledger_store::{ConsensusStore, helpers::memory::ConsensusMemory};
use snarkvm_synthesizer::VM;
use snarkvm_synthesizer_program::{FinalizeGlobalState, Program};

use aleo_std::StorageMode;

type CurrentNetwork = MainnetV0;
type CurrentLedger = ConsensusMemory<CurrentNetwork>;

fn sample_next_block<R: Rng + CryptoRng>(
    vm: &VM<CurrentNetwork, CurrentLedger>,
    private_key: &PrivateKey<CurrentNetwork>,
    transactions: &[Transaction<CurrentNetwork>],
    rng: &mut R,
) -> Result<Block<CurrentNetwork>> {
    let block_hash = vm.block_store().get_block_hash(vm.block_store().max_height().unwrap()).unwrap().unwrap();
    let previous_block = vm.block_store().get_block(&block_hash).unwrap().unwrap();

    let next_block_height = previous_block.height() + 1;
    let time_since_last_block = CurrentNetwork::BLOCK_TIME as i64;
    let finalize_state =
        FinalizeGlobalState::from(next_block_height as u64, next_block_height, [0u8; 32]);

    let (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations) =
        vm.speculate(finalize_state, time_since_last_block, None, vec![], &None.into(), transactions.iter(), rng)?;

    let metadata = Metadata::new(
        CurrentNetwork::ID,
        previous_block.round() + 1,
        previous_block.height() + 1,
        0,
        0,
        CurrentNetwork::GENESIS_COINBASE_TARGET,
        CurrentNetwork::GENESIS_PROOF_TARGET,
        previous_block.last_coinbase_target(),
        previous_block.last_coinbase_timestamp(),
        previous_block.timestamp().saturating_add(time_since_last_block),
    )?;

    let header = Header::from(
        vm.block_store().current_state_root(),
        transactions.to_transactions_root().unwrap(),
        transactions.to_finalize_root(ratified_finalize_operations).unwrap(),
        ratifications.to_ratifications_root().unwrap(),
        Field::zero(),
        Field::zero(),
        metadata,
    )?;

    Block::new_beacon(
        private_key,
        previous_block.hash(),
        header,
        ratifications,
        None.into(),
        vec![],
        transactions,
        aborted_transaction_ids,
        rng,
    )
}

fn main() {

    let rng = &mut TestRng::from_seed(160426);

    // Generate the genesis private key.
    let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();

    // Generate the genesis block using a temporary VM.
    let genesis = {
        let vm = VM::<CurrentNetwork, CurrentLedger>::from(ConsensusStore::open(StorageMode::new_test(None)).unwrap())
            .unwrap();
        vm.genesis_beacon(&private_key, rng).unwrap()
    };

    // Initialize the VM.
    let vm =
        VM::<CurrentNetwork, CurrentLedger>::from(ConsensusStore::open(StorageMode::new_test(None)).unwrap()).unwrap();

    // Add the genesis block.
    vm.add_next_block(&genesis).unwrap();

    // Advance the ledger to the latest consensus version
    let transactions: [Transaction<CurrentNetwork>; 0] = [];
    while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::latest()).unwrap() {
        let next_block = sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
        vm.add_next_block(&next_block).unwrap();
    }
    println!("Advanced to latest consensus version: {:?}", ConsensusVersion::latest());

    let deployment_configs = [
        // (Number of programs, number of times each input is hashed)
        (1, 1 << 4),
        (1, 1 << 5),
        (1, 1 << 6),
        (1, 1 << 7),

        (2, 1 << 4),
        (2, 1 << 5),
        (2, 1 << 6),

        (4, 1 << 4),
        (4, 1 << 5),

        (8, 1 << 4),
    ];

    println!("");

    for (deployment_idx, (num_progs, multiplier)) in deployment_configs.into_iter().enumerate() {

        println!("Processing deployment with {num_progs} program(s) with multiplier {multiplier}");

        let deployments = (0..num_progs).map(|i| {

            let mut program_str = format!(r"
                program test_{deployment_idx}_{i}.aleo;

                function fun:
                    input r0 as [field; 32u32].public;
            ");

            for j in 1..multiplier {
                program_str += &format!(r"
                    hash.bhp256 r0 into r{} as field;
                ", j);
            }
                
            program_str += r"
            constructor:
                    assert.eq true true;
                ";

            let program = Program::from_str(&program_str).unwrap();

            // Deploy the first program
            let deployment_tx = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();
            let deployment = deployment_tx.deployment().unwrap();

            assert!(deployment.verifying_keys().len() == 1);
            let circuit_info = deployment.verifying_keys().first().unwrap().1.0.circuit_info;
            let total_density = circuit_info.num_non_zero_a + circuit_info.num_non_zero_b + circuit_info.num_non_zero_c;
            println!(" - Program {:?}: total density: {:?}", deployment.program().id(), total_density);
            
            deployment_tx
        }).collect::<Vec<_>>();

        let start = Instant::now();
        vm.check_transactions(&deployments.iter().map(|deployment| (deployment, None)).collect::<Vec<_>>(), rng).unwrap();
        let elapsed = start.elapsed().as_millis() as f64 / 1000.0;
        println!("Deployment(s) checked in {elapsed:.2} s\n");
    }
}
