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

use super::*;

use crate::vm::test_helpers::{sample_vm_at_height, *};

use anyhow::Result;
use console::{
    network::ConsensusVersion,
    program::{Identifier, Value},
};
use synthesizer_program::Program;
use utilities::TestRng;

#[test]
fn test_dynamic_call_to_transfer_public() -> Result<()> {
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key)?;

    // Initialize the VM.
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V6)?, rng);

    // Define the program to be executed.
    let program = Program::from_str(
        r"
import credits.aleo;
        
program test_dcall_to_transfer_public.aleo;

function static:
    input r0 as address.public;
    input r1 as u64.public;
    dcall credits transfer_public with r0 r1 (as address.public u64.public) into r2 (as dynamic.future);
    async static r2 into r3;
    output r3 as test_dcall_to_transfer_public.aleo/static.future;
finalize static:
    input r0 as dynamic.future;
    await r0; 
        
function dynamic:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as address.public;
    input r3 as u64.public;
    dcall r0 r1 with r2 r3 (as address.public u64.public) into r4 (as dynamic.future);
    async dynamic r4 into r5;
    output r5 as test_dcall_to_transfer_public.aleo/dynamic.future;
finalize dynamic:
    input r0 as dynamic.future;
    await r0;
    ",
    )?;

    // Deploy the program.
    println!("Deploying program: {}", program.id());
    let transaction = vm.deploy(&caller_private_key, &program, None, 0, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block)?;

    // Execute the "static" function.
    println!("Executing the `static` function...");
    let transaction = vm.execute(
        &caller_private_key,
        ("test_dynamic_call_to_transfer_public.aleo", "static"),
        vec![Value::from_str(&format!("{caller_address}"))?, Value::from_str("1234u64")?].into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    vm.check_transaction(&transaction, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block)?;

    // Execute the "dynamic" function.
    let credits_as_field = Identifier::<CurrentNetwork>::from_str("credits")?.to_field()?;
    let transfer_public_as_field = Identifier::<CurrentNetwork>::from_str("transfer_public")?.to_field()?;
    println!("credits_as_field: {credits_as_field}");
    println!("transfer_public_as_field: {transfer_public_as_field}");
    let transaction = vm.execute(
        &caller_private_key,
        ("test_dynamic_call_to_transfer_public.aleo", "dynamic"),
        vec![
            Value::from_str(&format!("{credits_as_field}"))?,
            Value::from_str(&format!("{transfer_public_as_field}"))?,
            Value::from_str(&format!("{caller_address}"))?,
            Value::from_str("1234u64")?,
        ]
        .into_iter(),
        None,
        0,
        None,
        rng,
    )?;
    vm.check_transaction(&transaction, None, rng)?;
    let block = sample_next_block(&vm, &caller_private_key, &[transaction], rng)?;
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block)?;

    Ok(())
}
