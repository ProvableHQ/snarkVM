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

use crate::vm::test_helpers::*;

use console::{
    account::{Address, ViewKey},
    network::ConsensusVersion,
    program::{ProgramID, Value},
};
use snarkvm_synthesizer_program::Program;
use snarkvm_utilities::TestRng;

#[test]
fn test_universal_swap() {
    // Turn on trace logging.
    tracing_subscriber::fmt::init();
    // Define a mint_private function and constructor.
    let mint_private_function = r"

function mint_private:
    input r0 as u64.private;
    cast self.caller r0 into r1 as credits.record;
    cast self.caller r0 into r2 as credits.record;
    output r1 as credits.record;
    output r2 as credits.record;

constructor:
    assert.eq true true;
";
    // Define the credits programs.
    let credits_program = Program::<CurrentNetwork>::credits().unwrap().to_string();
    let mut credits_a_program = credits_program.replace("credits.aleo", "credits_a.aleo");
    credits_a_program.push_str(mint_private_function);
    let credits_a_program = Program::from_str(&credits_a_program).unwrap();
    let mut credits_b_program = credits_program.replace("credits.aleo", "credits_b.aleo");
    credits_b_program.push_str(mint_private_function);
    let credits_b_program = Program::from_str(&credits_b_program).unwrap();

    // Define the swap program.
    let amm_program = Program::from_str(
        r"
import credits_a.aleo;
import credits_b.aleo;

program amm.aleo;

struct reserves:
  // corresponds to credits_a.aleo
  token_a as u64;
  // corresponds to credits_b.aleo
  token_b as u64;

mapping reserves_mapping:
  key as address.public;
  value as reserves.public;

function buy_token_b:
  input r0 as credits_a.aleo/credits.record;
  // Token a amount
  input r1 as u64.public;
  // Token b amount
  input r2 as u64.public;
  cast r1 r2 into r3 as reserves;
  call credits_a.aleo/transfer_private_to_public r0 aleo1rrj2mgall8mw57lcpkkvkxwqkawpc5rjarqm57w8gux2ahnt9sxqf0md56 r1 into r4 r5;
  call credits_b.aleo/transfer_public_to_private r0.owner r2 into r6 r7;
  async buy_token_b r1 r2 r5 r7 into r8;
  // token_a change record
  output r4 as credits_a.aleo/credits.record;
  // token_b receiver record
  output r6 as credits_b.aleo/credits.record;
  output r8 as amm.aleo/buy_token_b.future;

finalize buy_token_b:
  // token_a amount
  input r0 as u64.public;
  // token_b amount
  input r1 as u64.public;
  input r2 as credits_a.aleo/transfer_private_to_public.future;
  input r3 as credits_b.aleo/transfer_public_to_private.future;
  await r2;
  await r3;
  // TODO: implement reserve update logic here.

constructor:
    assert.eq true true;
",
    ).unwrap();

    // Initialize an RNG.
    let rng = &mut TestRng::default();

    // Initialize a new caller.
    let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
    let caller_view_key = ViewKey::<CurrentNetwork>::try_from(caller_private_key).unwrap();

    // Initialize the VM at the V12 height.
    let v12_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V12).unwrap();
    let vm = crate::vm::test_helpers::sample_vm_at_height(v12_height, rng);

    // Deploy the program - one at a time so as not to surpass public payer limits.
    for program in [credits_a_program, credits_b_program, amm_program] {
        let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
        let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();
        assert_eq!(block.transactions().num_accepted(), 1);
        assert_eq!(block.transactions().num_rejected(), 0);
        assert_eq!(block.aborted_transaction_ids().len(), 0);
        vm.add_next_block(&block).unwrap();
    }

    // Execute credits_a.aleo/mint_private to mint a few credits_a records.
    let execute_mint_a = vm.execute(&caller_private_key, ("credits_a.aleo", "mint_private"), vec![Value::from_str("100u64")].into_iter(), None, 0, None, rng).unwrap();
    // Execute credits_b.aleo/mint_private to mint a few credits_b records.
    let execute_mint_b = vm.execute(&caller_private_key, ("credits_b.aleo", "mint_private"), vec![Value::from_str("100u64")].into_iter(), None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execute_mint_a, execute_mint_b], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 2);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
    
    // Obtain the credits records.
    let records = block.records().map(|(_, record)| record.decrypt(&caller_view_key)).collect::<Result<Vec<_>>>().unwrap();
    // Split the records into credits_a and credits_b records.
    let (records_a, records_b) = records.split_at(2);

    // Create the AMM program address.
    let amm_address: Address<CurrentNetwork> = ProgramID::from_str("amm.aleo").unwrap().to_address().unwrap();
    let amm_address_value = Value::from_str(&amm_address.to_string()).unwrap();

    // Execute credits_a.aleo/transfer_private_to_public to give amm.aleo an initial balance of credits_a.
    let execute_transfer_a = vm.execute(&caller_private_key, ("credits_a.aleo", "transfer_private_to_public"), vec![Value::Record(records_a[0].clone()), amm_address_value.clone(), Value::from_str("100u64").unwrap()].into_iter(), None, 0, None, rng).unwrap();
    // Execute credits_b.aleo/transfer_private_to_public to give amm.aleo an initial balance of credits_b.
    let execute_transfer_b = vm.execute(&caller_private_key, ("credits_b.aleo", "transfer_private_to_public"), vec![Value::Record(records_b[0].clone()), amm_address_value.clone(), Value::from_str("100u64").unwrap()].into_iter(), None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execute_transfer_a, execute_transfer_b], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 2);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
    
    // Execute amm.aleo/buy_token_b to buy token_b.
    let execute_buy_token_b = vm.execute(&caller_private_key, ("amm.aleo", "buy_token_b"), vec![Value::Record(records_a[1].clone()), Value::from_str("100u64").unwrap(), Value::from_str("100u64").unwrap()].into_iter(), None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[execute_buy_token_b], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
    
    // Obtain the credits_a change and credits_b receiver records.
    let (_change_record, _receiver_record) = block.records().map(|(_, record)| record.decrypt(&caller_view_key)).collect::<Result<Vec<_>>>().unwrap().split_at(1);
}
