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

//! Callgrind measurement of the transaction checks in `transaction.rs`.
//!
//! Proving stays in setup. The measured work is `check_transaction` and `LimitedWriter` serialization
//! for the deploy, public transfer, and private transfer cases. Rayon is pinned to one worker.

#![allow(clippy::type_complexity)]

use snarkvm_console::{
    account::*,
    network::{MainnetV0, Network},
    program::{Plaintext, Record, Value},
};
use snarkvm_ledger::Ledger;
use snarkvm_ledger_block::{Transaction, Transition};
use snarkvm_ledger_store::ConsensusStore;
use snarkvm_synthesizer::{VM, program::Program};
use snarkvm_utilities::{LimitedWriter, TestRng};

use aleo_std::StorageMode;
use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use indexmap::IndexMap;
use std::{hint::black_box, str::FromStr};

#[cfg(not(feature = "rocks"))]
type LedgerType = snarkvm_ledger_store::helpers::memory::ConsensusMemory<MainnetV0>;
#[cfg(feature = "rocks")]
type LedgerType = snarkvm_ledger_store::helpers::rocksdb::ConsensusDB<MainnetV0>;

/// Fixed RNG seed so benchmark inputs are reproducible across CI runs.
const BENCH_RNG_SEED: u64 = 0xB34D_CAFE_CDEC_0123;

struct Cases {
    deploy_ledger: Ledger<MainnetV0, LedgerType>,
    deploy: Transaction<MainnetV0>,
    execute_ledger: Ledger<MainnetV0, LedgerType>,
    public_tx: Transaction<MainnetV0>,
    private_tx: Transaction<MainnetV0>,
}

fn initialize_vm(
    private_key: &PrivateKey<MainnetV0>,
    rng: &mut TestRng,
) -> (Ledger<MainnetV0, LedgerType>, Vec<Record<MainnetV0, Plaintext<MainnetV0>>>) {
    let vm = VM::<MainnetV0, LedgerType>::from(ConsensusStore::open(StorageMode::new_test(None)).unwrap()).unwrap();
    let genesis = vm.genesis_beacon(private_key, rng).unwrap();

    let records = {
        let records = genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
        let view_key = ViewKey::try_from(private_key).unwrap();
        records.values().map(|record| record.decrypt(&view_key).unwrap()).collect()
    };

    let ledger = Ledger::<MainnetV0, LedgerType>::load(genesis, StorageMode::new_test(None)).unwrap();
    (ledger, records)
}

fn setup_cases() -> Cases {
    let deploy_rng = &mut TestRng::from_seed(BENCH_RNG_SEED);
    let deploy_key = PrivateKey::<MainnetV0>::new(deploy_rng).unwrap();
    let (deploy_ledger, records) = initialize_vm(&deploy_key, deploy_rng);
    let program = Program::<MainnetV0>::from_str(
        r"
program helloworld.aleo;

function hello:
    input r0 as u32.private;
    input r1 as u32.private;
    add r0 r1 into r2;
    output r2 as u32.private;
",
    )
    .unwrap();
    let deploy =
        deploy_ledger.vm().deploy(&deploy_key, &program, Some(records[0].clone()), 600000, None, deploy_rng).unwrap();

    let execute_rng = &mut TestRng::from_seed(BENCH_RNG_SEED ^ 1);
    let execute_key = PrivateKey::<MainnetV0>::new(execute_rng).unwrap();
    let address = Address::try_from(&execute_key).unwrap();
    let (execute_ledger, records) = initialize_vm(&execute_key, execute_rng);
    let vm = execute_ledger.vm();

    let public_inputs =
        [Value::<MainnetV0>::from_str(&address.to_string()).unwrap(), Value::<MainnetV0>::from_str("1u64").unwrap()]
            .into_iter();
    let public_authorization =
        vm.authorize(&execute_key, "credits.aleo", "transfer_public", public_inputs, execute_rng).unwrap();
    let public_execution_id = public_authorization.to_execution_id().unwrap();
    let public_fee = vm.authorize_fee_public(&execute_key, 300000, 1000, public_execution_id, execute_rng).unwrap();
    let public_tx = vm.execute_authorization(public_authorization, Some(public_fee), None, execute_rng).unwrap();

    let private_inputs = [
        Value::<MainnetV0>::Record(records[0].clone()),
        Value::<MainnetV0>::from_str(&address.to_string()).unwrap(),
        Value::<MainnetV0>::from_str("1u64").unwrap(),
    ]
    .into_iter();
    let private_authorization =
        vm.authorize(&execute_key, "credits.aleo", "transfer_private", private_inputs, execute_rng).unwrap();
    let private_execution_id = private_authorization.to_execution_id().unwrap();
    let private_fee = vm.authorize_fee_public(&execute_key, 300000, 1000, private_execution_id, execute_rng).unwrap();
    let private_tx = vm.execute_authorization(private_authorization, Some(private_fee), None, execute_rng).unwrap();

    Cases { deploy_ledger, deploy, execute_ledger, public_tx, private_tx }
}

fn write_len(transaction: &Transaction<MainnetV0>) -> usize {
    let max = MainnetV0::LATEST_MAX_TRANSACTION_SIZE();
    let mut buffer = Vec::with_capacity(3000);
    transaction.write_le(LimitedWriter::new(&mut buffer, max)).unwrap();
    buffer.len()
}

#[library_benchmark]
#[bench::checks(setup_cases())]
fn transaction_checks(cases: Cases) -> usize {
    let mut rng = TestRng::from_seed(BENCH_RNG_SEED);
    cases.deploy_ledger.vm().check_transaction(&cases.deploy, None, &mut rng).unwrap();

    let mut rng = TestRng::from_seed(BENCH_RNG_SEED ^ 1);
    let vm = cases.execute_ledger.vm();
    let public_len = write_len(&cases.public_tx);
    vm.check_transaction(&cases.public_tx, None, &mut rng).unwrap();
    let private_len = write_len(&cases.private_tx);
    vm.check_transaction(&cases.private_tx, None, &mut rng).unwrap();
    black_box(public_len + private_len)
}

library_benchmark_group!(name = transaction, benchmarks = [transaction_checks]);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = transaction
);
