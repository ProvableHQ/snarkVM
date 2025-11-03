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

#[macro_use]
extern crate criterion;

use snarkvm_console::{
    account::{Address, PrivateKey, ViewKey},
    network::MainnetV0,
    program::{Ciphertext, Value},
    types::Field,
};
use snarkvm_ledger_block::Transition;
use snarkvm_ledger_store::{ConsensusStore, helpers::memory::ConsensusMemory};
use snarkvm_ledger_test_helpers::{sample_genesis_block, sample_genesis_private_key};
use snarkvm_synthesizer::VM;
use snarkvm_utilities::TestRng;

use aleo_std::StorageMode;
use criterion::{BatchSize, Criterion};
use indexmap::IndexMap;
use std::str::FromStr;

type CurrentNetwork = MainnetV0;
type LedgerType = ConsensusMemory<CurrentNetwork>;

fn prepare_vm(
    rng: &mut TestRng,
) -> Result<
    (
        VM<CurrentNetwork, LedgerType>,
        IndexMap<Field<CurrentNetwork>, snarkvm_console::program::Record<CurrentNetwork, Ciphertext<CurrentNetwork>>>,
    ),
    Box<dyn std::error::Error>,
> {
    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);

    // Fetch the unspent records.
    let records = genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();

    // Initialize the VM.
    let vm = VM::from(ConsensusStore::open(StorageMode::new_test(None))?)?;
    // Update the VM.
    vm.add_next_block(&genesis)?;

    Ok((vm, records))
}

fn bench_transfer_private_execution(c: &mut Criterion) {
    let mut rng = TestRng::default();

    // Initialize a new caller and recipient.
    let caller_private_key = sample_genesis_private_key(&mut rng);
    let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();
    let recipient_private_key = PrivateKey::<CurrentNetwork>::new(&mut rng).unwrap();
    let recipient_address = Address::try_from(&recipient_private_key).unwrap();

    // Prepare the VM and records.
    let (vm, records) = prepare_vm(&mut rng).unwrap();

    // Fetch the unspent record.
    let record = records.values().next().unwrap().decrypt(&caller_view_key).unwrap();
    let transfer_amount = 1_000_000u64;

    // Prepare the inputs.
    let inputs = [
        Value::<CurrentNetwork>::Record(record),
        Value::<CurrentNetwork>::from_str(&recipient_address.to_string()).unwrap(),
        Value::<CurrentNetwork>::from_str(&format!("{transfer_amount}u64")).unwrap(),
    ]
    .into_iter();

    c.bench_function("vm.execute transfer_private", |b| {
        b.iter_batched_ref(
            || {},
            |_| {
                // This is the only part that gets benchmarked
                vm.execute(
                    &caller_private_key,
                    ("credits.aleo", "transfer_private"),
                    inputs.clone(),
                    None,
                    0,
                    None,
                    &mut rng,
                )
                .unwrap()
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20);
    targets = bench_transfer_private_execution,
}
criterion_main!(benches);
