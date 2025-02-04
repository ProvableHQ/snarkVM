// Copyright 2024 Aleo Network Foundation
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

#![allow(clippy::type_complexity)]

#[macro_use]
extern crate criterion;

use console::{
    account::*,
    network::{MainnetV0, Network},
    program::{Plaintext, Record, Value},
};
use ledger_block::Transition;
use ledger_store::{ConsensusStore, helpers::memory::ConsensusMemory};
use synthesizer::{VM, program::Program};

use criterion::Criterion;
use indexmap::IndexMap;

fn initialize_vm<R: Rng + CryptoRng>(
    private_key: &PrivateKey<MainnetV0>,
    rng: &mut R,
) -> (VM<MainnetV0, ConsensusMemory<MainnetV0>>, Vec<Record<MainnetV0, Plaintext<MainnetV0>>>) {
    // Initialize the VM.
    let vm = VM::from(ConsensusStore::open(None).unwrap()).unwrap();

    // Initialize the genesis block.
    let genesis = vm.genesis_beacon(private_key, rng).unwrap();

    // Fetch the unspent records.
    let records = genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();

    // Select a record to spend.
    let view_key = ViewKey::try_from(private_key).unwrap();
    let records = records.values().map(|record| record.decrypt(&view_key).unwrap()).collect();

    // Update the VM.
    vm.add_next_block(&genesis).unwrap();

    (vm, records)
}

fn deploy(c: &mut Criterion) {
    let rng = &mut TestRng::default();

    // Sample a new private key and address.
    let private_key = PrivateKey::<MainnetV0>::new(rng).unwrap();

    // Initialize the VM.
    let (vm, records) = initialize_vm(&private_key, rng);

    // Create a sample program.
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

    c.bench_function("Transaction::Deploy", |b| {
        b.iter(|| vm.deploy(&private_key, &program, Some(records[0].clone()), 600000, None, rng).unwrap())
    });

    c.bench_function("Transaction::Deploy - verify", |b| {
        let transaction = vm.deploy(&private_key, &program, Some(records[0].clone()), 600000, None, rng).unwrap();
        b.iter(|| vm.check_transaction(&transaction, None, rng).unwrap())
    });
}

fn execute(c: &mut Criterion) {
    let rng = &mut TestRng::default();

    // Sample a new private key and address.
    let private_key = PrivateKey::<MainnetV0>::new(rng).unwrap();
    let address = Address::try_from(&private_key).unwrap();

    // Initialize the VM.
    let (vm, records) = initialize_vm(&private_key, rng);

    {
        // Prepare the inputs.
        let inputs = [
            Value::<MainnetV0>::from_str(&address.to_string()).unwrap(),
            Value::<MainnetV0>::from_str("1u64").unwrap(),
        ]
        .into_iter();

        // Authorize the execution.
        let execute_authorization = vm.authorize(&private_key, "credits.aleo", "transfer_public", inputs, rng).unwrap();
        // Retrieve the execution ID.
        let execution_id = execute_authorization.to_execution_id().unwrap();
        // Authorize the fee.
        let fee_authorization = vm.authorize_fee_public(&private_key, 300000, 1000, execution_id, rng).unwrap();

        c.bench_function("Transaction::Execute(transfer_public)", |b| {
            b.iter(|| {
                vm.execute_authorization(
                    execute_authorization.replicate(),
                    Some(fee_authorization.replicate()),
                    None,
                    rng,
                )
                .unwrap();
            })
        });

        let transaction = vm
            .execute_authorization(execute_authorization.replicate(), Some(fee_authorization.replicate()), None, rng)
            .unwrap();

        // Bench the Transaction.write_le method using the LimitedWriter.
        c.bench_function("LimitedWriter::new - transfer_public", |b| {
            let mut buffer = Vec::with_capacity(3000);
            b.iter(|| transaction.write_le(LimitedWriter::new(&mut buffer, MainnetV0::MAX_TRANSACTION_SIZE)))
        });

        // Bench the execution of transfer_public.
        c.bench_function("Transaction::Execute(transfer_public) - verify", |b| {
            b.iter(|| vm.check_transaction(&transaction, None, rng).unwrap())
        });
    }

    {
        // Prepare the inputs.
        let inputs = [
            Value::<MainnetV0>::Record(records[0].clone()),
            Value::<MainnetV0>::from_str(&address.to_string()).unwrap(),
            Value::<MainnetV0>::from_str("1u64").unwrap(),
        ]
        .into_iter();

        // Authorize the execution.
        let execute_authorization =
            vm.authorize(&private_key, "credits.aleo", "transfer_private", inputs, rng).unwrap();
        // Retrieve the execution ID.
        let execution_id = execute_authorization.to_execution_id().unwrap();
        // Authorize the fee.
        let fee_authorization = vm.authorize_fee_public(&private_key, 300000, 1000, execution_id, rng).unwrap();

        // Bench the execution of transfer_private.
        c.bench_function("Transaction::Execute(transfer_private)", |b| {
            b.iter(|| {
                vm.execute_authorization(
                    execute_authorization.replicate(),
                    Some(fee_authorization.replicate()),
                    None,
                    rng,
                )
                .unwrap();
            })
        });

        let transaction = vm
            .execute_authorization(execute_authorization.replicate(), Some(fee_authorization.replicate()), None, rng)
            .unwrap();

        // Bench the Transaction.write_le method using the LimitedWriter.
        c.bench_function("LimitedWriter::new - transfer_private", |b| {
            let mut buffer = Vec::with_capacity(3000);
            b.iter(|| transaction.write_le(LimitedWriter::new(&mut buffer, MainnetV0::MAX_TRANSACTION_SIZE)))
        });

        // Bench the check_transaction method.
        c.bench_function("Transaction::Execute(transfer_private) - verify", |b| {
            b.iter(|| vm.check_transaction(&transaction, None, rng).unwrap())
        });
    }

    // Bench Transaction.write_le + VM.check_transaction methods for transactions above the maximum transaction size.
    {
        // Define a program that will create an execution transaction larger than the maximum transaction size.
        let program = Program::<MainnetV0>::from_str(
            r"
program too_big.aleo;

struct all_groups:
    g1 as [[[group; 4u32]; 4u32]; 4u32];
    g2 as [[[group; 4u32]; 4u32]; 4u32];

struct nested_groups:
    g1 as all_groups;
    g2 as all_groups;

function main:
    // Input the amount of microcredits to unbond.
    input r0 as group.public;
    cast r0 r0 r0 r0 into r1 as [group; 4u32];
    cast r1 r1 r1 r1 into r2 as [[group; 4u32]; 4u32];
    cast r2 r2 r2 r2 into r3 as [[[group; 4u32]; 4u32]; 4u32];
    cast r3 r3 into r4 as all_groups;
    cast r4 r4 into r5 as nested_groups;
    cast r4 r4 into r6 as nested_groups;
    cast r4 r4 into r7 as nested_groups;
    cast r4 r4 into r8 as nested_groups;
    cast r4 r4 into r9 as nested_groups;
    cast r4 r4 into r10 as nested_groups;
    cast r4 r4 into r11 as nested_groups;
    cast r4 r4 into r12 as nested_groups;
    cast r4 r4 into r13 as nested_groups;
    cast r4 r4 into r14 as nested_groups;
    cast r4 r4 into r15 as nested_groups;
    cast r4 r4 into r16 as nested_groups;
    cast r4 r4 into r17 as nested_groups;
    cast r4 r4 into r18 as nested_groups;
    cast r4 r4 into r19 as nested_groups;
    cast r4 r4 into r20 as nested_groups;
    cast r4 r4 into r21 as nested_groups;
    cast r4 r4 into r22 as nested_groups;
    cast r4 r4 into r23 as nested_groups;
    cast r4 r4 into r24 as nested_groups;
    cast r4 r4 into r25 as nested_groups;
    cast r4 r4 into r26 as nested_groups;
    cast r4 r4 into r27 as nested_groups;
    cast r4 r4 into r28 as nested_groups;
    cast r4 r4 into r29 as nested_groups;
    cast r4 r4 into r30 as nested_groups;
    cast r4 r4 into r31 as nested_groups;
    output r7 as nested_groups.public;
    output r8 as nested_groups.public;
    output r9 as nested_groups.public;
    output r10 as nested_groups.public;
    output r11 as nested_groups.public;
    output r12 as nested_groups.public;
    output r13 as nested_groups.public;
    output r14 as nested_groups.public;
    output r15 as nested_groups.public;
    output r16 as nested_groups.public;
    output r17 as nested_groups.public;
    output r18 as nested_groups.public;
    output r19 as nested_groups.public;
    output r20 as nested_groups.public;
    output r21 as nested_groups.public;
    output r22 as nested_groups.public;
    ",
        )
        .unwrap();
        // Prepare the inputs.
        let inputs = [Value::from_str("2group").unwrap()].into_iter();

        // Add the program to the VM.
        vm.process().write().add_program(&program).unwrap();

        // Create an execution transaction that is 164613 bytes in size.
        let transaction = vm.execute(&private_key, ("too_big.aleo", "main"), inputs, None, 0, None, rng).unwrap();

        // Bench the Transaction.write_le method using the LimitedWriter.
        c.bench_function("LimitedWriter::new - too_big.aleo", |b| {
            let mut buffer = Vec::with_capacity(MainnetV0::MAX_TRANSACTION_SIZE);
            b.iter(|| transaction.write_le(LimitedWriter::new(&mut buffer, MainnetV0::MAX_TRANSACTION_SIZE)))
        });

        // Bench the check_transaction method.
        c.bench_function("Transaction::Execute(too_big.aleo) - verify", |b| {
            b.iter(|| vm.check_transaction(&transaction, None, rng))
        });
    }
}

criterion_group! {
    name = transaction;
    config = Criterion::default().sample_size(10);
    targets = deploy, execute
}

criterion_main!(transaction);
