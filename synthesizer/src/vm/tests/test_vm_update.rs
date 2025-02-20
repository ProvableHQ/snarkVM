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

use super::*;

// This test checks that:
//  - the logic of a simple transition without records can be updated.
//  - once a program is updated, the old executions are no longer valid.
//  - an invalid admin cannot update a program.
#[test]
fn test_simple_update() -> Result<()> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);

    // Initialize the VM.
    let vm = sample_vm();
    vm.add_next_block(&genesis)?;

    // Initialize the program.
    let program = Program::from_str(&format!(
        r"
program adder.aleo;

mapping admins:
    key as address.public;
    value as boolean.public;

_init:
    metadata.get edition into r0;
    branch.neq r0 0u16 to rest;
    set true into admins[{caller_address}];
    branch.eq true true to end;
    position rest;
    metadata.get owner into r1;
    get admins[r1] into r0;
    assert.eq r0 true;
    position end;

function binary_add:
    input r0 as u8.public;
    input r1 as u8.public;
    add r0 r1 into r2;
    output r2 as u8.public;
    "
    ))?;

    // Deploy the program.
    let transaction = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    vm.add_next_block(&block)?;

    // Check that the program is deployed.
    let stack = vm.process().read().get_stack("adder.aleo")?;
    assert_eq!(stack.program_id(), &ProgramID::from_str("adder.aleo")?);
    assert_eq!(stack.edition(), 0);

    // Check that the caller is an admin.
    let Some(Value::Plaintext(Plaintext::Literal(Literal::Boolean(caller_is_admin), _))) =
        vm.finalize_store().get_value_confirmed(
            ProgramID::from_str("adder.aleo")?,
            Identifier::from_str("admins")?,
            &Plaintext::from(Literal::Address(caller_address)),
        )?
    else {
        bail!("Unexpected value");
    };
    assert!(*caller_is_admin);

    // Execute the program.
    let original_execution = vm.execute(
        &caller_private_key,
        ("adder.aleo", "binary_add"),
        vec![Value::from_str("1u8")?, Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    assert!(vm.check_transaction(&original_execution, None, rng).is_ok());

    // Check that the output is correct.
    let output = match original_execution.transitions().next().unwrap().outputs().last().unwrap() {
        Output::Public(_, Some(Plaintext::Literal(Literal::U8(value), _))) => **value,
        output => bail!(format!("Unexpected output: {output}")),
    };
    assert_eq!(output, 2u8);

    // Update the program.
    let updated_program = Program::from_str(&format!(
        r"
program adder.aleo;

_init:
    metadata.get edition into r0;
    branch.neq r0 0u8 into rest;
    set true into admins[{caller_address}];
    branch.eq true true into end;
    position rest;
    metadata.get owner into r1;
    get admins[r1] into r0;
    assert.eq r0 true;
    position end;

function binary_add:
    input r0 as u8.public;
    input r1 as u8.public;
    add.w r0 r1 into r2;
    output r2 as u8.public;
    "
    ))?;

    // Attempt to deploy the updated program with an invalid authority.
    let invalid_private_key = PrivateKey::new(rng)?;
    let transaction = vm.deploy(&invalid_private_key, &updated_program, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &invalid_private_key, &[transaction], rng)?;
    assert_eq!(block.aborted_transaction_ids().len(), 1);
    vm.add_next_block(&block)?;

    // Deploy the updated program.
    let transaction = vm.deploy(&caller_private_key, &updated_program, None, 0, None, rng)?;
    assert_eq!(transaction.deployment().unwrap().edition(), 1);
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Check that the program is updated.
    let stack = vm.process().read().get_stack("adder.aleo")?;
    assert_eq!(stack.program_id(), &ProgramID::from_str("adder.aleo")?);
    assert_eq!(stack.edition(), 1);

    // Check that the old execution is no longer valid.
    vm.partially_verified_transactions().write().clear();
    assert!(vm.check_transaction(&original_execution, None, rng).is_err());

    // Execute the updated program.
    let new_execution = vm.execute(
        &caller_private_key,
        ("adder.aleo", "binary_add"),
        vec![Value::from_str("1u8")?, Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    assert!(vm.check_transaction(&new_execution, None, rng).is_ok());

    // Check that the output is correct.
    let output = match new_execution.transitions().next().unwrap().outputs().last().unwrap() {
        Output::Public(_, Some(Plaintext::Literal(Literal::U8(value), _))) => **value,
        output => bail!(format!("Unexpected output: {output}")),
    };
    assert_eq!(output, 2u8);

    Ok(())
}

// TODO (@d0cd): This can readapted to check that one cannot update a program with a restricted init block
#[test]
fn test_program_owner_v1_is_not_updatable() -> Result<()> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = sample_genesis_private_key(rng);

    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);

    // Initialize the VM.
    let vm = sample_vm();
    vm.add_next_block(&genesis)?;

    // Initialize the program.
    let program = Program::from_str("program basic.aleo;function foo:")?;

    // Deploy the program.
    let transaction = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    vm.add_next_block(&block)?;

    // Initialize the updated program.
    let updated_program = Program::from_str("program basic.aleo;function foo:function bar:")?;

    // Attempt to deploy the updated program using `VM::deploy`.
    let transaction = vm.deploy(&caller_private_key, &updated_program, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.aborted_transaction_ids().len(), 1);
    vm.add_next_block(&block)?;

    // Attempt to deploy the updated program using `VM::deploy_updatable`.
    let transaction = vm.deploy(&caller_private_key, &updated_program, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.aborted_transaction_ids().len(), 1);
    vm.add_next_block(&block)?;

    Ok(())
}

// This test checks that:
//  - the first instance of a program must be the zero-th edition.
//  - subsequent updates to the program must be sequential.
#[test]
fn test_editions_are_sequential() -> Result<()> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = sample_genesis_private_key(rng);

    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);

    // Initialize two VMs.
    let off_chain_vm = sample_vm();
    let on_chain_vm = sample_vm();
    off_chain_vm.add_next_block(&genesis)?;
    on_chain_vm.add_next_block(&genesis)?;

    // Define the three versions of the program.
    let program_v0 = Program::from_str("program basic.aleo;function foo:")?;
    let program_v1 = Program::from_str("program basic.aleo;function foo:function bar:")?;
    let program_v2 = Program::from_str("program basic.aleo;function foo:function bar:function baz:")?;

    // Using the off-chain VM, generate a sequence of deployments.
    let deployment_v0_pass = off_chain_vm.deploy(&caller_private_key, &program_v0, None, 0, None, rng)?;
    off_chain_vm.process().write().add_program(&program_v0)?;
    let deployment_v1_fail = off_chain_vm.deploy(&caller_private_key, &program_v1, None, 0, None, rng)?;
    let deployment_v1_pass = off_chain_vm.deploy(&caller_private_key, &program_v1, None, 0, None, rng)?;
    let deployment_v2_as_v1_fail = off_chain_vm.deploy(&caller_private_key, &program_v2, None, 0, None, rng)?;
    off_chain_vm.process().write().add_program(&program_v1)?;
    let deployment_v2_fail = off_chain_vm.deploy(&caller_private_key, &program_v2, None, 0, None, rng)?;
    let deployment_v2_pass = off_chain_vm.deploy(&caller_private_key, &program_v2, None, 0, None, rng)?;

    // Deploy the programs to the on-chain VM individually in the following sequence:
    // - deployment_v1_fail
    // - deployment_v0_pass
    // - deployment_v2_fail
    // - deployment_v1_pass
    // - deployment_v2_as_v1_fail
    // - deployment_v2_pass
    // Their name should indicate whether the deployment should pass or fail.

    // This deployment should fail because the it is not the zero-th edition.
    let block = sample_next_block(&on_chain_vm, &caller_private_key, &[deployment_v1_fail], rng)?;
    assert_eq!(block.aborted_transaction_ids().len(), 1);
    on_chain_vm.add_next_block(&block)?;

    // This deployment should pass.
    let block = sample_next_block(&on_chain_vm, &caller_private_key, &[deployment_v0_pass], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    on_chain_vm.add_next_block(&block)?;
    let stack = on_chain_vm.process().read().get_stack("basic.aleo")?;
    assert_eq!(stack.edition(), 0);

    // This deployment should fail because it does not increment the edition.
    let block = sample_next_block(&on_chain_vm, &caller_private_key, &[deployment_v2_fail], rng)?;
    assert_eq!(block.aborted_transaction_ids().len(), 1);
    on_chain_vm.add_next_block(&block)?;

    // This deployment should pass.
    let block = sample_next_block(&on_chain_vm, &caller_private_key, &[deployment_v1_pass], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    on_chain_vm.add_next_block(&block)?;
    let stack = on_chain_vm.process().read().get_stack("basic.aleo")?;
    assert_eq!(stack.edition(), 1);

    // This deployment should fail because it attempt to redeploy at the same edition.
    let block = sample_next_block(&on_chain_vm, &caller_private_key, &[deployment_v2_as_v1_fail], rng)?;
    assert_eq!(block.aborted_transaction_ids().len(), 1);
    on_chain_vm.add_next_block(&block)?;

    // This deployment should pass.
    let block = sample_next_block(&on_chain_vm, &caller_private_key, &[deployment_v2_pass], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    on_chain_vm.add_next_block(&block)?;
    let stack = on_chain_vm.process().read().get_stack("basic.aleo")?;
    assert_eq!(stack.edition(), 2);

    Ok(())
}

// This test checks that:
//  - records created before an update are still valid after an update.
//  - records created after an update can be created and used in the updated program.
//  - records are semantically distinct (old records cannot be used in functions that require new records).
//  - functions can be disabled using `assert.neq self.caller self.caller`.
#[test]
fn test_update_with_records() -> Result<()> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_view_key = ViewKey::try_from(&caller_private_key)?;

    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);

    // Initialize the VM.
    let vm = sample_vm();
    vm.add_next_block(&genesis)?;

    // Define the two versions of the program.
    let program_v0 = Program::from_str(
        r"
program record_test.aleo;

record data_v1:
    owner as address.private;
    data as u8.public;

function mint:
    input r0 as u8.public;
    cast self.caller r0 into r1 as data_v1.record;
    output r1 as data_v1.record;",
    )?;

    let program_v1 = Program::from_str(
        r"
program record_test.aleo;

record data_v1:
    owner as address.private;
    data as u8.public;

record data_v2:
    owner as address.private;
    data as u8.public;

function mint:
    input r0 as u8.public;
    assert.neq self.caller self.caller;
    cast self.caller r0 into r1 as data_v1.record;
    output r1 as data_v1.record;

function convert:
    input r0 as data_v1.record;
    cast r0.owner r0.data into r1 as data_v2.record;
    output r1 as data_v2.record;

function burn:
    input r0 as data_v2.record;",
    )?;

    // Deploy the first version of the program.
    let transaction = vm.deploy(&caller_private_key, &program_v0, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Execute the mint function twice.
    let mint_execution_0 = vm.execute(
        &caller_private_key,
        ("record_test.aleo", "mint"),
        vec![Value::from_str("0u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let mint_execution_1 = vm.execute(
        &caller_private_key,
        ("record_test.aleo", "mint"),
        vec![Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[mint_execution_0, mint_execution_1], rng)?;
    assert_eq!(block.transactions().num_accepted(), 2);
    let mut v1_records = block
        .records()
        .map(|(_, record)| record.decrypt(&caller_view_key))
        .collect::<Result<Vec<Record<CurrentNetwork, Plaintext<CurrentNetwork>>>>>()?;
    assert_eq!(v1_records.len(), 2);
    vm.add_next_block(&block)?;

    // Update the program.
    let transaction = vm.deploy(&caller_private_key, &program_v1, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Attempt to execute the mint function.
    assert!(
        vm.execute(
            &caller_private_key,
            ("record_test.aleo", "mint"),
            vec![Value::from_str("0u8")?].into_iter(),
            None,
            0,
            None,
            rng
        )
        .is_err()
    );

    // Get the first record and execute the convert function.
    let record = v1_records.pop().unwrap();
    let convert_execution = vm.execute(
        &caller_private_key,
        ("record_test.aleo", "convert"),
        vec![Value::Record(record)].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[convert_execution], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    let mut v2_records = block
        .records()
        .map(|(_, record)| record.decrypt(&caller_view_key))
        .collect::<Result<Vec<Record<CurrentNetwork, Plaintext<CurrentNetwork>>>>>()?;
    assert_eq!(v2_records.len(), 1);
    vm.add_next_block(&block)?;

    // Get the v2 record and execute the burn function.
    let record = v2_records.pop().unwrap();
    let burn_execution = vm.execute(
        &caller_private_key,
        ("record_test.aleo", "burn"),
        vec![Value::Record(record)].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[burn_execution], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Attempt to execute the burn function with the remaining v1 record.
    let record = v1_records.pop().unwrap();
    assert!(
        vm.execute(
            &caller_private_key,
            ("record_test.aleo", "burn"),
            vec![Value::Record(record)].into_iter(),
            None,
            0,
            None,
            rng
        )
        .is_err()
    );

    Ok(())
}

// This test checks that:
//  - mappings created before an update are still valid after an update.
//  - mappings created by and updated are correctly initialized and usable in the program.
//  - functions can be disabled by inserting a failing condition in the on-chain logic.
#[test]
fn test_update_with_mappings() -> Result<()> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = sample_genesis_private_key(rng);

    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);

    // Initialize the VM.
    let vm = sample_vm();
    vm.add_next_block(&genesis)?;

    // Define the two versions of the program.
    let program_v0 = Program::from_str(
        r"
program mapping_test.aleo;

mapping data_v1:
    key as u8.public;
    value as u8.public;

function store_data_v1:
    input r0 as u8.public;
    input r1 as u8.public;
    async store_data_v1 r0 r1 into r2;
    output r2 as mapping_test.aleo/store_data_v1.future;
finalize store_data_v1:
    input r0 as u8.public;
    input r1 as u8.public;
    set r1 into data_v1[r0];",
    )?;

    let program_v1 = Program::from_str(
        r"
program mapping_test.aleo;

mapping data_v1:
    key as u8.public;
    value as u8.public;

mapping data_v2:
    key as u8.public;
    value as u8.public;

function store_data_v1:
    input r0 as u8.public;
    input r1 as u8.public;
    async store_data_v1 r0 r1 into r2;
    output r2 as mapping_test.aleo/store_data_v1.future;
finalize store_data_v1:
    input r0 as u8.public;
    input r1 as u8.public;
    assert.neq true true;

function migrate_data_v1_to_v2:
    input r0 as u8.public;
    async migrate_data_v1_to_v2 r0 into r1;
    output r1 as mapping_test.aleo/migrate_data_v1_to_v2.future;
finalize migrate_data_v1_to_v2:
    input r0 as u8.public;
    get data_v1[r0] into r1;
    remove data_v1[r0];
    set r1 into data_v2[r0];

function store_data_v2:
    input r0 as u8.public;
    input r1 as u8.public;
    async store_data_v2 r0 r1 into r2;
    output r2 as mapping_test.aleo/store_data_v2.future;
finalize store_data_v2:
    input r0 as u8.public;
    input r1 as u8.public;
    set r1 into data_v2[r0];",
    )?;

    // Deploy the first version of the program.
    let transaction = vm.deploy(&caller_private_key, &program_v0, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Execute the store_data_v1 function.
    let store_data_v1_execution = vm.execute(
        &caller_private_key,
        ("mapping_test.aleo", "store_data_v1"),
        vec![Value::from_str("0u8")?, Value::from_str("0u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[store_data_v1_execution], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Check that the value was stored correctly.
    let value = match vm.finalize_store().get_value_confirmed(
        ProgramID::from_str("mapping_test.aleo")?,
        Identifier::from_str("data_v1")?,
        &Plaintext::from_str("0u8")?,
    )? {
        Some(Value::Plaintext(Plaintext::Literal(Literal::U8(value), _))) => *value,
        value => bail!(format!("Unexpected value: {:?}", value)),
    };
    assert_eq!(value, 0u8);

    // Update the program.
    let transaction = vm.deploy(&caller_private_key, &program_v1, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Attempt to execute the store_data_v1 function.
    let transaction = vm.execute(
        &caller_private_key,
        ("mapping_test.aleo", "store_data_v1"),
        vec![Value::from_str("1u8")?, Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_rejected(), 1);
    vm.add_next_block(&block)?;

    // Execute the migrate_data_v1_to_v2 function.
    let migrate_data_v1_to_v2_execution = vm.execute(
        &caller_private_key,
        ("mapping_test.aleo", "migrate_data_v1_to_v2"),
        vec![Value::from_str("0u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[migrate_data_v1_to_v2_execution], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Check that the value was migrated correctly.
    let value = match vm.finalize_store().get_value_confirmed(
        ProgramID::from_str("mapping_test.aleo")?,
        Identifier::from_str("data_v2")?,
        &Plaintext::from_str("0u8")?,
    )? {
        Some(Value::Plaintext(Plaintext::Literal(Literal::U8(value), _))) => *value,
        value => bail!(format!("Unexpected value: {:?}", value)),
    };
    assert_eq!(value, 0u8);

    // Check that the old value was removed.
    assert!(
        vm.finalize_store()
            .get_value_confirmed(
                ProgramID::from_str("mapping_test.aleo")?,
                Identifier::from_str("data_v1")?,
                &Plaintext::from_str("0u8")?
            )?
            .is_none()
    );

    // Execute the store_data_v2 function.
    let store_data_v2_execution = vm.execute(
        &caller_private_key,
        ("mapping_test.aleo", "store_data_v2"),
        vec![Value::from_str("1u8")?, Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[store_data_v2_execution], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Check that the value was stored correctly.
    let value = match vm.finalize_store().get_value_confirmed(
        ProgramID::from_str("mapping_test.aleo")?,
        Identifier::from_str("data_v2")?,
        &Plaintext::from_str("1u8")?,
    )? {
        Some(Value::Plaintext(Plaintext::Literal(Literal::U8(value), _))) => *value,
        value => bail!(format!("Unexpected value: {:?}", value)),
    };
    assert_eq!(value, 1u8);

    Ok(())
}

// This test checks that:
//  - a dependent program accepts an update to off-chain logic
//  - a dependent program accepts an update to on-chain logic
//  - a dependent program can fix a specific version of the dependency
//  - old executions of the dependent program are no longer valid after an update
#[test]
fn test_update_with_dependents() -> Result<()> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = sample_genesis_private_key(rng);

    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);

    // Initialize the VM.
    let vm = sample_vm();
    vm.add_next_block(&genesis)?;

    // Define the two versions of the dependency program.
    let dependency_v0 = Program::from_str(
        r"
program dependency.aleo;

function sum:
    input r0 as u8.public;
    input r1 as u8.public;
    add r0 r1 into r2;
    output r0 as u8.public;

function sum_and_check:
    input r0 as u8.public;
    input r1 as u8.public;
    add r0 r1 into r2;
    async sum_and_check into r3;
    output r2 as u8.public;
    output r3 as dependency.aleo/sum_and_check.future;
finalize sum_and_check:
    assert.eq true true;",
    )?;

    let dependency_v1 = Program::from_str(
        r"
program dependency.aleo;

function sum:
    input r0 as u8.public;
    input r1 as u8.public;
    add.w r0 r1 into r2;
    output r0 as u8.public;

function sum_and_check:
    input r0 as u8.public;
    input r1 as u8.public;
    add.w r0 r1 into r2;
    async sum_and_check into r3;
    output r2 as u8.public;
    output r3 as dependency.aleo/sum_and_check.future;
finalize sum_and_check:
    assert.eq true false;",
    )?;

    // Define the two versions of the dependent program.
    let dependent_v0 = Program::from_str(
        r"
import dependency.aleo;

program dependent.aleo;

function sum_unchecked:
    input r0 as u8.public;
    input r1 as u8.public;
    call dependency.aleo/sum r0 r1 into r2;
    output r2 as u8.public;

function sum:
    input r0 as u8.public;
    input r1 as u8.public;
    call dependency.aleo/sum r0 r1 into r2;
    async sum into r3;
    output r2 as u8.public;
    output r3 as dependent.aleo/sum.future;
finalize sum:
    metadata.get dependency.aleo/edition into r0;
    assert.eq r0 0u16;

function sum_and_check:
    input r0 as u8.public;
    input r1 as u8.public;
    call dependency.aleo/sum_and_check r0 r1 into r2 r3;
    async sum_and_check r3 into r4;
    output r2 as u8.public;
    output r4 as dependent.aleo/sum_and_check.future;
finalize sum_and_check:
    input r0 as dependency.aleo/sum_and_check.future;
    await r0;",
    )?;

    let dependent_v1 = Program::from_str(
        r"
import dependency.aleo;

program dependent.aleo;

function sum_unchecked:
    input r0 as u8.public;
    input r1 as u8.public;
    call dependency.aleo/sum r0 r1 into r2;
    output r2 as u8.public;

function sum:
    input r0 as u8.public;
    input r1 as u8.public;
    call dependency.aleo/sum r0 r1 into r2;
    async sum into r3;
    output r2 as u8.public;
    output r3 as dependent.aleo/sum.future;
finalize sum:
    metadata.get dependency.aleo/edition into r0;
    assert.eq r0 1u16;

function sum_and_check:
    input r0 as u8.public;
    input r1 as u8.public;
    call dependency.aleo/sum_and_check r0 r1 into r2 r3;
    async sum_and_check r3 into r4;
    output r2 as u8.public;
    output r4 as dependent.aleo/sum_and_check.future;
finalize sum_and_check:
    input r0 as dependency.aleo/sum_and_check.future;
    await r0;",
    )?;

    // At a high level, this test will:
    // 1. Deploy the v0 dependency and v0 dependent.
    // 2. Verify that the the dependent program can be correctly executed.
    // 3. Update the dependency to v1.
    // 4. Verify that the call to `sum_and_check` automatically uses the new logic, however, the call `sum` fails because the edition is not 0.
    // 5. Update the dependent to v1.
    // 6. Verify that the call to `sum` now passes because the edition is 1.

    // Deploy the v0 dependency.
    let transaction = vm.deploy(&caller_private_key, &dependency_v0, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Deploy the v0 dependent.
    let transaction = vm.deploy(&caller_private_key, &dependent_v0, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Execute the functions.
    let tx_1 = vm.execute(
        &caller_private_key,
        ("dependent.aleo", "sum"),
        vec![Value::from_str("1u8")?, Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let tx_2 = vm.execute(
        &caller_private_key,
        ("dependent.aleo", "sum_and_check"),
        vec![Value::from_str("1u8")?, Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[tx_1, tx_2], rng)?;
    assert_eq!(block.transactions().num_accepted(), 2);
    vm.add_next_block(&block)?;

    // Verify that the sum function fails on overflow.
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        vm.execute(
            &caller_private_key,
            ("dependent.aleo", "sum"),
            vec![Value::from_str("255u8")?, Value::from_str("1u8")?].into_iter(),
            None,
            0,
            None,
            rng,
        )
    }));
    assert!(result.is_err());

    // Get a valid execution before the dependency update.
    let sum_unchecked = vm.execute(
        &caller_private_key,
        ("dependent.aleo", "sum_unchecked"),
        vec![Value::from_str("1u8")?, Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    assert!(vm.check_transaction(&sum_unchecked, None, rng).is_ok());

    // Update the dependency to v1.
    let transaction = vm.deploy(&caller_private_key, &dependency_v1, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Verify that the original sum transaction fails after the dependency update.
    vm.partially_verified_transactions().write().clear();
    assert!(vm.check_transaction(&sum_unchecked, None, rng).is_err());
    let block = sample_next_block(&vm, &caller_private_key, &[sum_unchecked], rng)?;
    assert_eq!(block.aborted_transaction_ids().len(), 1);
    vm.add_next_block(&block)?;

    // Verify that the sum function fails on edition check.
    let tx_1 = vm.execute(
        &caller_private_key,
        ("dependent.aleo", "sum"),
        vec![Value::from_str("1u8")?, Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let tx_2 = vm.execute(
        &caller_private_key,
        ("dependent.aleo", "sum_and_check"),
        vec![Value::from_str("1u8")?, Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[tx_1, tx_2], rng)?;
    assert_eq!(block.transactions().num_rejected(), 2);
    vm.add_next_block(&block)?;

    // Update the dependent to v1.
    let transaction = vm.deploy(&caller_private_key, &dependent_v1, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Verify that the sum function passes.
    let tx_1 = vm.execute(
        &caller_private_key,
        ("dependent.aleo", "sum"),
        vec![Value::from_str("1u8")?, Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let tx_2 = vm.execute(
        &caller_private_key,
        ("dependent.aleo", "sum"),
        vec![Value::from_str("255u8")?, Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[tx_1, tx_2], rng)?;
    assert_eq!(block.transactions().num_accepted(), 2);
    vm.add_next_block(&block)?;

    Ok(())
}

// This test checks that:
//  - programs can be updated to create cycles in the dependency graph.
//  - programs can be updated to create cycles in the call graph.
//  - executions of cyclic programs w.r.t. to the call graph are rejected.
#[test]
fn test_update_with_cycles() -> Result<()> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = sample_genesis_private_key(rng);

    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);

    // Initialize the VM.
    let vm = sample_vm();
    vm.add_next_block(&genesis)?;

    // Define the programs.
    let first_v0 = Program::from_str(
        r"
program first.aleo;

function foo:
    input r0 as u8.public;
    output r0 as u8.public;
    ",
    )?;

    let second_v0 = Program::from_str(
        r"
import first.aleo;

program second.aleo;

function foo:
    input r0 as u8.public;
    call first.aleo/foo r0 into r1;
    output r1 as u8.public;
    ",
    )?;

    let first_v1 = Program::from_str(
        r"
import second.aleo;

program first.aleo;

function foo:
    input r0 as u8.public;
    output r0 as u8.public;
    ",
    )?;

    let first_v2 = Program::from_str(
        r"
import second.aleo;

program first.aleo;

function foo:
    input r0 as u8.public;
    call second.aleo/foo r0 into r1;
    output r1 as u8.public;
    ",
    )?;

    // Deploy the first version of the programs.
    let transaction = vm.deploy(&caller_private_key, &first_v0, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    let transaction = vm.deploy(&caller_private_key, &second_v0, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Verify that both can be executed correctly.
    let tx_1 = vm.execute(
        &caller_private_key,
        ("first.aleo", "foo"),
        vec![Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let tx_2 = vm.execute(
        &caller_private_key,
        ("second.aleo", "foo"),
        vec![Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[tx_1, tx_2], rng)?;
    assert_eq!(block.transactions().num_accepted(), 2);
    vm.add_next_block(&block)?;

    // Update the first program to create a cycle in the dependency graph.
    let transaction = vm.deploy(&caller_private_key, &first_v1, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Verify that both programs can be executed correctly.
    let tx_1 = vm.execute(
        &caller_private_key,
        ("first.aleo", "foo"),
        vec![Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let tx_2 = vm.execute(
        &caller_private_key,
        ("second.aleo", "foo"),
        vec![Value::from_str("1u8")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[tx_1, tx_2], rng)?;
    assert_eq!(block.transactions().num_accepted(), 2);
    vm.add_next_block(&block)?;

    // Update the first program to create mutual recursion.
    let transaction = vm.deploy(&caller_private_key, &first_v2, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Verify that the first program is no longer executable.
    assert!(
        vm.execute(
            &caller_private_key,
            ("first.aleo", "foo"),
            vec![Value::from_str("1u8")?].into_iter(),
            None,
            0,
            None,
            rng,
        )
        .is_err()
    );

    Ok(())
}

// This test checks that a deployment with a failing _init block is rejected.
#[test]
fn test_failing_init_block() -> Result<()> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = sample_genesis_private_key(rng);

    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);

    // Initialize the VM.
    let vm = sample_vm();
    vm.add_next_block(&genesis)?;

    // Define the programs.
    let passing_program = Program::from_str(
        r"
program hello1.aleo;

_init:
    assert.eq true true;

function foo:
    input r0 as u8.public;
    output r0 as u8.public;
    ",
    )?;

    let failing_program = Program::from_str(
        r"
program hello2.aleo;

_init:
    assert.eq true false;

function foo:
    input r0 as u8.public;
    output r0 as u8.public;
    ",
    )?;

    // Deploy the passing program.
    let transaction = vm.deploy(&caller_private_key, &passing_program, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block)?;

    // Deploy the failing program.
    let transaction = vm.deploy(&caller_private_key, &failing_program, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.aborted_transaction_ids().len(), 1);
    vm.add_next_block(&block)?;

    Ok(())
}
