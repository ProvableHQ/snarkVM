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

use aleo_std::StorageMode;
use criterion::Criterion;
use snarkvm_console::{
    account::{Address, PrivateKey, ViewKey},
    network::MainnetV0,
    program::{Ciphertext, Value},
    types::Field,
};
use snarkvm_ledger_block::Transition;
use snarkvm_ledger_store::{ConsensusStore, helpers::memory::ConsensusMemory};
use snarkvm_synthesizer::VM;
use snarkvm_utilities::TestRng;

use indexmap::IndexMap;
use std::str::FromStr;

type CurrentNetwork = MainnetV0;
type CurrentStorage = ConsensusMemory<CurrentNetwork>;

/// Prepares the VM with a genesis block and returns the unspent records.
fn prepare_vm(
    rng: &mut TestRng,
) -> (
    VM<CurrentNetwork, CurrentStorage>,
    PrivateKey<CurrentNetwork>,
    IndexMap<Field<CurrentNetwork>, snarkvm_console::program::Record<CurrentNetwork, Ciphertext<CurrentNetwork>>>,
) {
    // Initialize a new caller.
    let caller_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();

    // Initialize the VM.
    let vm = VM::from(ConsensusStore::open(StorageMode::new_test(None)).unwrap()).unwrap();

    // Create genesis block.
    let genesis = vm.genesis_beacon(&caller_private_key, rng).unwrap();

    // Fetch the unspent records.
    let records = genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();

    // Update the VM.
    vm.add_next_block(&genesis).unwrap();

    (vm, caller_private_key, records)
}

fn bench_execute_authorization(c: &mut Criterion) {
    let mut rng = TestRng::fixed(42);

    // Prepare the VM and records.
    let (vm, caller_private_key, records) = prepare_vm(&mut rng);
    let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();
    let address = Address::try_from(&caller_private_key).unwrap();

    // Get a record from the genesis block.
    let record = records.values().next().unwrap().decrypt(&caller_view_key).unwrap();

    // Pre-generate multiple authorizations for benchmarking (since each execution consumes the record).
    let authorizations: Vec<_> = (0..20)
        .map(|_| {
            let inputs = [
                Value::<CurrentNetwork>::Record(record.clone()),
                Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str("1u64").unwrap(),
            ];
            vm.authorize(&caller_private_key, "credits.aleo", "transfer_private", inputs, &mut rng).unwrap()
        })
        .collect();

    let mut auth_iter = authorizations.into_iter().cycle();

    c.bench_function("vm.execute_authorization (transfer_private)", |b| {
        b.iter(|| {
            let authorization = auth_iter.next().unwrap();
            std::hint::black_box(vm.execute_authorization(authorization, None, None, &mut rng).unwrap())
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_execute_authorization
}
criterion_main!(benches);
