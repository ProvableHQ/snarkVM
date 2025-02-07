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
use synthesizer_program::StackProgram;

#[test]
fn test_simple_update() -> Result<()> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = sample_genesis_private_key(rng);

    // Initialize the genesis block.
    let genesis = sample_genesis_block(rng);

    // Initialize the VM.
    let vm = sample_vm();
    vm.add_next_block(&genesis)?;

    // Initialize the program.
    let program = Program::from_str(
        r"
program adder.aleo;

function binary_add:
    input r0 as u8.public;
    input r1 as u8.public;
    add r0 r1 into r2;
    output r2 as u8.public;
    ",
    )?;

    // Deploy the program.
    let transaction = vm.deploy_updatable(
        &caller_private_key,
        Address::try_from(&caller_private_key)?,
        0,
        &program,
        None,
        0,
        None,
        rng,
    )?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    vm.add_next_block(&block)?;

    // Check that the program is deployed.
    let stack = vm.process().read().get_stack("adder.aleo")?;
    assert_eq!(stack.program_id(), &ProgramID::from_str("adder.aleo")?);
    assert_eq!(stack.edition(), 0);

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
    vm.check_transaction(&original_execution, None, rng)?;

    // Check that the output is correct.
    let output = match original_execution.transitions().next().unwrap().outputs().last().unwrap() {
        Output::Public(_, Some(Plaintext::Literal(Literal::U8(value), _))) => **value,
        output => bail!(format!("Unexpected output: {output}")),
    };
    assert_eq!(output, 2u8);

    // Update the program.
    let updated_program = Program::from_str(
        r"
program adder.aleo;

function binary_add:
    input r0 as u8.public;
    input r1 as u8.public;
    add r0 r1 into r2;
    add r2 1u8 into r3;
    output r3 as u8.public;
    ",
    )?;

    // Deploy the updated program.
    let transaction = vm.deploy_updatable(
        &caller_private_key,
        Address::try_from(&caller_private_key)?,
        1,
        &updated_program,
        None,
        0,
        None,
        rng,
    )?;
    assert_eq!(transaction.deployment().unwrap().edition(), 1);
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block)?;

    // Check that the program is updated.
    let stack = vm.process().read().get_stack("adder.aleo")?;
    assert_eq!(stack.program_id(), &ProgramID::from_str("adder.aleo")?);
    assert_eq!(stack.edition(), 1);

    // Check that the old execution is no longer valid.
    vm.check_transaction(&original_execution, None, rng)?;

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
    vm.check_transaction(&new_execution, None, rng)?;

    // Check that the output is correct.
    let output = match new_execution.transitions().next().unwrap().outputs().last().unwrap() {
        Output::Public(_, Some(Plaintext::Literal(Literal::U8(value), _))) => **value,
        output => bail!(format!("Unexpected output: {output}")),
    };
    assert_eq!(output, 3u8);

    Ok(())
}
