// Copyright (c) 2019-2025 Provable Inc.
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

use std::sync::OnceLock;

use aleo_std::StorageMode;
use snarkvm_console::{account::PrivateKey, network::MainnetV0};
use snarkvm_ledger_block::Block;
use snarkvm_ledger_store::{ConsensusStore, helpers::memory::ConsensusMemory};
use snarkvm_synthesizer::VM;
use snarkvm_utilities::TestRng;

pub type CurrentNetwork = MainnetV0;
pub type LedgerType = ConsensusMemory<CurrentNetwork>;

pub fn sample_genesis_private_key(rng: &mut TestRng) -> PrivateKey<CurrentNetwork> {
    static INSTANCE: OnceLock<PrivateKey<CurrentNetwork>> = OnceLock::new();
    *INSTANCE.get_or_init(|| {
        // Initialize a new caller.
        PrivateKey::<CurrentNetwork>::new(rng).unwrap()
    })
}

pub fn sample_genesis_block(rng: &mut TestRng) -> Block<CurrentNetwork> {
    static INSTANCE: OnceLock<Block<CurrentNetwork>> = OnceLock::new();
    INSTANCE
        .get_or_init(|| {
            // Initialize the VM.
            let vm = sample_vm();
            // Initialize a new caller.
            let caller_private_key = sample_genesis_private_key(rng);
            // Return the block.
            vm.genesis_beacon(&caller_private_key, rng).unwrap()
        })
        .clone()
}

pub fn sample_vm() -> VM<CurrentNetwork, LedgerType> {
    // Initialize a new VM.
    VM::from(ConsensusStore::open(StorageMode::new_test(None)).unwrap()).unwrap()
}

pub fn sample_vm_with_genesis_block(rng: &mut TestRng) -> VM<CurrentNetwork, LedgerType> {
    // Initialize the VM.
    let vm = sample_vm();
    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);
    // Update the VM.
    vm.add_next_block(&genesis).unwrap();
    // Return the VM.
    vm
}
