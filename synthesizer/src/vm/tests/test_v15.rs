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

use super::*;

use crate::vm::test_helpers::*;

use console::{
    account::{Address, PrivateKey},
    network::ConsensusVersion,
    program::{Identifier, Literal, Plaintext, ProgramID, Value},
};
use snarkvm_utilities::TestRng;

// The number of blocks that must pass before an unbond can be claimed before V15.
const UNBOND_BLOCKS_BEFORE_V15: u32 = 360;
// The number of blocks that must pass before an unbond can be claimed at and after V15.
const UNBOND_BLOCKS_AFTER_V15: u32 = 403_200;
// The minimum delegator stake required to bond to a validator, in microcredits.
const MIN_DELEGATOR_STAKE: u64 = 10_000_000_000u64;

// Returns `true` if the credits.aleo stack contains the `redelegate` function.
fn credits_has_redelegate(vm: &VM<CurrentNetwork, LedgerType>) -> bool {
    vm.process()
        .read()
        .get_stack("credits.aleo")
        .unwrap()
        .program()
        .contains_function(&Identifier::from_str("redelegate").unwrap())
}

// Returns the unlock height from the unbonding state of the given staker.
fn unbond_height(vm: &VM<CurrentNetwork, LedgerType>, staker: &Address<CurrentNetwork>) -> u32 {
    let Some(Value::Plaintext(Plaintext::Struct(state, _))) = vm
        .finalize_store()
        .get_value_confirmed(
            ProgramID::from_str("credits.aleo").unwrap(),
            Identifier::from_str("unbonding").unwrap(),
            &Plaintext::from(Literal::Address(*staker)),
        )
        .unwrap()
    else {
        panic!("Expected an unbond state for {staker}");
    };
    match state.get(&Identifier::from_str("height").unwrap()) {
        Some(Plaintext::Literal(Literal::U32(h), _)) => **h,
        _ => panic!("Expected a height in the unbond state for {staker}"),
    }
}

// This test verifies that `redelegate` cannot be executed before V15, as the function does not
// exist in credits.aleo prior to V15.
#[test]
fn test_redelegate_before_v15_fails() {
    let rng = &mut TestRng::default();

    let caller_private_key = sample_genesis_private_key(rng);
    let v15_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap();

    // Initialize the VM at one block before V15. Credits.aleo is V1 at this height.
    let vm = sample_vm_at_height(v15_height - 1, rng);
    assert!(!credits_has_redelegate(&vm), "Expected credits.aleo not to have `redelegate` before V15");

    // Generate a fresh address to use as the intended new validator.
    let new_validator_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let new_validator_address = Address::try_from(&new_validator_key).unwrap();

    // Attempt to call `redelegate` before V15. This should fail because the function does not
    // exist in credits.aleo V1.
    let result = vm.execute(
        &caller_private_key,
        ("credits.aleo", "redelegate"),
        [Value::from_str(&new_validator_address.to_string()).unwrap()].iter(),
        None,
        0,
        None,
        rng,
    );
    assert!(result.is_err(), "Expected `redelegate` to fail before V15, as the function does not exist");
}

// This test verifies that `redelegate` can be executed after V15.
#[test]
fn test_redelegate_after_v15_succeeds() {
    let rng = &mut TestRng::default();

    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key).unwrap();
    let v15_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap();

    // Initialize the VM at V15. Credits.aleo is V2 at this height.
    let vm = sample_vm_at_height(v15_height, rng);
    assert!(credits_has_redelegate(&vm), "Expected credits.aleo to have `redelegate` at V15");

    // Create a fresh delegator account and a fresh address to redelegate to.
    let delegator_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let delegator_address = Address::try_from(&delegator_key).unwrap();
    let new_validator_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let new_validator_address = Address::try_from(&new_validator_key).unwrap();

    // Fund the delegator with enough credits to bond and pay fees.
    let transfer = vm
        .execute(
            &caller_private_key,
            ("credits.aleo", "transfer_public"),
            [
                Value::from_str(&delegator_address.to_string()).unwrap(),
                Value::from_str(&format!("{}u64", MIN_DELEGATOR_STAKE * 2)).unwrap(),
            ]
            .iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[transfer], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block).unwrap();

    // Bond the delegator to the genesis validator (which is open by default).
    let bond = vm
        .execute(
            &delegator_key,
            ("credits.aleo", "bond_public"),
            [
                Value::from_str(&caller_address.to_string()).unwrap(),
                Value::from_str(&delegator_address.to_string()).unwrap(),
                Value::from_str(&format!("{MIN_DELEGATOR_STAKE}u64")).unwrap(),
            ]
            .iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[bond], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block).unwrap();

    // Execute `redelegate` from the delegator to the new validator address. Since the new validator
    // is not in the committee, it defaults to open, so the redelegate should succeed.
    let redelegate = vm
        .execute(
            &delegator_key,
            ("credits.aleo", "redelegate"),
            [Value::from_str(&new_validator_address.to_string()).unwrap()].iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[redelegate], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();
}

// This test verifies that unbonding before V15 produces an unlock height of `block_height + 360`.
#[test]
fn test_unbond_height_before_v15() {
    let rng = &mut TestRng::default();

    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key).unwrap();
    let v15_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap();

    // Initialize the VM at one block before V15. Credits.aleo is V1 at this height, which uses
    // a 360-block unbonding period.
    let vm = sample_vm_at_height(v15_height - 1, rng);

    // Unbond the genesis validator. Block 18 (the V15 block) finalizes with V1 credits before
    // the credits program is updated to V2, so the unbond height should reflect V1 semantics.
    let unbond = vm
        .execute(
            &caller_private_key,
            ("credits.aleo", "unbond_public"),
            [Value::from_str(&caller_address.to_string()).unwrap(), Value::from_str("1u64").unwrap()].iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[unbond], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block).unwrap();

    // The unbond was finalized at the current block height. Verify the unlock height reflects
    // the 360-block unbonding period from credits.aleo V1.
    let expected_height = vm.block_store().current_block_height() + UNBOND_BLOCKS_BEFORE_V15;
    assert_eq!(unbond_height(&vm, &caller_address), expected_height);
}

// This test verifies that unbonding after V15 produces an unlock height of `block_height + 403,200`.
#[test]
fn test_unbond_height_after_v15() {
    let rng = &mut TestRng::default();

    let caller_private_key = sample_genesis_private_key(rng);
    let caller_address = Address::try_from(&caller_private_key).unwrap();
    let v15_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap();

    // Initialize the VM at V15. Credits.aleo is V2 at this height, which uses a 403,200-block
    // unbonding period.
    let vm = sample_vm_at_height(v15_height, rng);

    // Unbond the genesis validator. The next block finalizes with V2 credits, so the unbond
    // height should reflect V2 semantics.
    let unbond = vm
        .execute(
            &caller_private_key,
            ("credits.aleo", "unbond_public"),
            [Value::from_str(&caller_address.to_string()).unwrap(), Value::from_str("1u64").unwrap()].iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[unbond], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    vm.add_next_block(&block).unwrap();

    // The unbond was finalized at the current block height. Verify the unlock height reflects
    // the 403,200-block unbonding period from credits.aleo V2.
    let expected_height = vm.block_store().current_block_height() + UNBOND_BLOCKS_AFTER_V15;
    assert_eq!(unbond_height(&vm, &caller_address), expected_height);
}

// This test verifies that the credits.aleo stack is updated with the `redelegate` function at V15.
#[test]
fn test_credits_stack_updated_at_v15() {
    let rng = &mut TestRng::default();

    let v15_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap();

    // Initialize the VM at one block before V15 and verify that credits.aleo does not yet contain
    // the `redelegate` function.
    let vm_before = sample_vm_at_height(v15_height - 1, rng);
    assert!(!credits_has_redelegate(&vm_before), "Expected credits.aleo not to have `redelegate` before V15");

    // Initialize the VM at V15 and verify that credits.aleo now contains the `redelegate` function.
    let vm_after = sample_vm_at_height(v15_height, rng);
    assert!(credits_has_redelegate(&vm_after), "Expected credits.aleo to have `redelegate` at V15");
}
