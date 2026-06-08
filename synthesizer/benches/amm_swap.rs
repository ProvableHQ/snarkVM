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
    network::{ConsensusVersion, MainnetV0},
    prelude::{Network, ToField, Zero},
    program::{Identifier, Literal, LiteralType, Plaintext, PlaintextType, ProgramID, Record, Value},
    types::{Field, Scalar},
};
use snarkvm_ledger_block::{Block, Header, Metadata, Transaction};
use snarkvm_ledger_store::{ConsensusStore, helpers::memory::ConsensusMemory};
use snarkvm_synthesizer::{VM, process::Authorization};
use snarkvm_synthesizer_program::{CommitVariant, FinalizeGlobalState, HashVariant, Program, evaluate_commit, evaluate_hash};
use snarkvm_utilities::TestRng;

use std::str::FromStr;

type CurrentNetwork = MainnetV0;
type CurrentStorage = ConsensusMemory<CurrentNetwork>;

const TOKEN_REGISTRY_PROGRAM: &str = include_str!("amm/token_registry.aleo");
const AMM_PROGRAM: &str = include_str!("amm/leo_amm.aleo");
const TEST_TOKEN_PROGRAM: &str = include_str!("amm/test_token.aleo");

/// Sets up the VM with genesis, adds `token_registry` and `leo_amm` to the process
/// (in-process only, since their executions are merely proved and never block-verified),
/// advances the chain to the V14 consensus height, and deploys `test_token` on-chain.
///
/// Advancing to V14 ensures the active Varuna version is V2 (matching cuVaruna), so that
/// cuVaruna-generated proofs verify when added to a block, and that all V14 opcodes used by
/// the AMM are active. Deploying `test_token` on-chain lets minted `Token` records be added
/// to a block, which is required for the private swaps' record-inclusion proofs.
fn prepare_amm_vm(rng: &mut TestRng) -> (VM<CurrentNetwork, CurrentStorage>, PrivateKey<CurrentNetwork>) {
    let caller_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();

    let vm = VM::from(ConsensusStore::open(StorageMode::new_test(None)).unwrap()).unwrap();

    let genesis = vm.genesis_beacon(&caller_private_key, rng).unwrap();
    vm.add_next_block(&genesis).unwrap();

    // Add the programs the AMM calls into the process for execution (not deployed on-chain).
    let token_registry = Program::<CurrentNetwork>::from_str(TOKEN_REGISTRY_PROGRAM).unwrap();
    vm.process().lock().add_program(&token_registry).unwrap();

    let amm = Program::<CurrentNetwork>::from_str(AMM_PROGRAM).unwrap();
    vm.process().lock().add_program(&amm).unwrap();

    // Advance the chain to the V14 height (Varuna V2 + all V14 opcodes active).
    let v14_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V14).unwrap();
    advance_to_height(&vm, &caller_private_key, v14_height, rng);

    // Deploy `test_token` on-chain so that minted Token records can be added to a block.
    let test_token = Program::<CurrentNetwork>::from_str(TEST_TOKEN_PROGRAM).unwrap();
    let deploy_tx = vm.deploy(&caller_private_key, &test_token, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &caller_private_key, &[deploy_tx], rng);
    assert_eq!(block.transactions().num_accepted(), 1, "test_token deployment was not accepted");
    vm.add_next_block(&block).unwrap();

    (vm, caller_private_key)
}

/// Advances the VM to `height` by appending empty blocks, mirroring the in-crate
/// `advance_vm_to_height` test helper.
fn advance_to_height(
    vm: &VM<CurrentNetwork, CurrentStorage>,
    private_key: &PrivateKey<CurrentNetwork>,
    height: u32,
    rng: &mut TestRng,
) {
    while vm.block_store().current_block_height() < height {
        let block = sample_next_block(vm, private_key, &[], rng);
        vm.add_next_block(&block).unwrap();
    }
}

/// Builds the next beacon block containing `transactions`, mirroring the in-crate
/// `sample_next_block` test helper.
fn sample_next_block(
    vm: &VM<CurrentNetwork, CurrentStorage>,
    private_key: &PrivateKey<CurrentNetwork>,
    transactions: &[Transaction<CurrentNetwork>],
    rng: &mut TestRng,
) -> Block<CurrentNetwork> {
    // Get the most recent block.
    let block_hash = vm.block_store().get_block_hash(vm.block_store().max_height().unwrap()).unwrap().unwrap();
    let previous_block = vm.block_store().get_block(&block_hash).unwrap().unwrap();

    // Create the finalize state for the next block height.
    let next_block_height = previous_block.height() + 1;
    let time_since_last_block = CurrentNetwork::BLOCK_TIME as i64;
    let next_block_timestamp = previous_block.timestamp().saturating_add(time_since_last_block);
    let next_timestamp = (next_block_height >= CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V12).unwrap_or_default())
        .then_some(next_block_timestamp);
    let finalize_state =
        FinalizeGlobalState::from(next_block_height as u64, next_block_height, next_timestamp, [0u8; 32]);

    // Speculate on the ratifications, solutions, and transactions.
    let (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations) = vm
        .speculate(finalize_state, time_since_last_block, Some(0u64), vec![], &None.into(), transactions.iter(), rng)
        .unwrap();

    // Construct the metadata associated with the block.
    let metadata = Metadata::new(
        CurrentNetwork::ID,
        previous_block.round() + 1,
        next_block_height,
        0,
        0,
        CurrentNetwork::GENESIS_COINBASE_TARGET,
        CurrentNetwork::GENESIS_PROOF_TARGET,
        previous_block.last_coinbase_target(),
        previous_block.last_coinbase_timestamp(),
        previous_block.timestamp().saturating_add(time_since_last_block),
    )
    .unwrap();

    // Construct the new block header.
    let header = Header::from(
        vm.block_store().current_state_root(),
        transactions.to_transactions_root().unwrap(),
        transactions.to_finalize_root(ratified_finalize_operations).unwrap(),
        ratifications.to_ratifications_root().unwrap(),
        Field::zero(),
        Field::zero(),
        metadata,
    )
    .unwrap();

    // Construct the new block.
    Block::new_beacon(
        private_key,
        previous_block.hash(),
        header,
        ratifications,
        None.into(),
        vec![],
        transactions,
        aborted_transaction_ids,
        rng,
    )
    .unwrap()
}

/// Returns the field representation of `test_token` identifier (used as the token program
/// in dynamic calls).
fn test_token_name_field() -> Field<CurrentNetwork> {
    Identifier::<CurrentNetwork>::from_str("test_token").unwrap().to_field().unwrap()
}

/// Returns the account view-key scalar, which satisfies the AMM's private-identity check
/// `Aleo::generator() * view_key == self.signer` (the address is `generator * view_key`).
fn view_key_scalar(private_key: &PrivateKey<CurrentNetwork>) -> Scalar<CurrentNetwork> {
    *ViewKey::try_from(private_key).unwrap()
}

/// Computes the blinded address exactly as the AMM private functions do, by reusing snarkVM's
/// own opcode evaluators (so the result is bit-identical to the in-circuit computation):
///   r       = hash.bhp256.raw([leo_amm.aleo as field, view_key as field, counter as field]) as scalar
///   blinded = commit.bhp256(self.signer, r) as address
fn derive_blinded_address(
    signer: &Address<CurrentNetwork>,
    view_key: &Scalar<CurrentNetwork>,
    counter: u32,
) -> Address<CurrentNetwork> {
    // Cast the three preimage inputs to field, matching the `cast ... as field` opcodes.
    let program_address = ProgramID::<CurrentNetwork>::from_str("leo_amm.aleo").unwrap().to_address().unwrap();
    let address_field = Literal::Address(program_address).cast(LiteralType::Field).unwrap();
    let view_key_field = Literal::Scalar(*view_key).cast(LiteralType::Field).unwrap();
    let counter_field = Literal::<CurrentNetwork>::from_str(&format!("{counter}u32")).unwrap().cast(LiteralType::Field).unwrap();

    // Build the `[field; 3]` array and hash it to a scalar over the raw bits.
    let preimage =
        Value::<CurrentNetwork>::from_str(&format!("[{address_field}, {view_key_field}, {counter_field}]")).unwrap();
    let randomizer =
        match evaluate_hash(HashVariant::HashBHP256Raw, &preimage, &PlaintextType::Literal(LiteralType::Scalar))
            .unwrap()
        {
            Plaintext::Literal(Literal::Scalar(scalar), _) => scalar,
            other => panic!("expected a scalar hash output, found {other}"),
        };

    // Commit to the signer with the randomizer to obtain the blinded address.
    let signer_value = Value::<CurrentNetwork>::from_str(&signer.to_string()).unwrap();
    match evaluate_commit(CommitVariant::CommitBHP256, &signer_value, &randomizer, LiteralType::Address).unwrap() {
        Literal::Address(address) => address,
        other => panic!("expected an address commit output, found {other}"),
    }
}

/// Mints a private `test_token.aleo` `Token` record owned by the caller by executing
/// `transfer_public_to_private` and adding the transaction to a block, so that the record's
/// commitment exists on-chain. Returns the decrypted plaintext record. This runs once during
/// (untimed) setup so the private swaps have a real, spendable record with a valid inclusion proof.
fn mint_test_token_record(
    vm: &VM<CurrentNetwork, CurrentStorage>,
    caller_private_key: &PrivateKey<CurrentNetwork>,
    amount: u128,
    rng: &mut TestRng,
) -> Record<CurrentNetwork, Plaintext<CurrentNetwork>> {
    let caller_view_key = ViewKey::try_from(caller_private_key).unwrap();
    let address = Address::try_from(caller_private_key).unwrap();

    let inputs = [
        Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
        Value::<CurrentNetwork>::from_str(&format!("{amount}u128")).unwrap(),
    ];
    let transaction = vm
        .execute(caller_private_key, ("test_token.aleo", "transfer_public_to_private"), inputs.into_iter(), None, 0, None, rng)
        .unwrap();

    // Add the mint to a block so the record's commitment exists on-chain.
    let block = sample_next_block(vm, caller_private_key, &[transaction], rng);
    assert_eq!(block.transactions().num_accepted(), 1, "test_token mint was not accepted");
    vm.add_next_block(&block).unwrap();

    // Extract and decrypt the Token record owned by the caller from the block outputs.
    block
        .records()
        .find_map(|(_, record)| record.decrypt(&caller_view_key).ok())
        .expect("expected a decryptable Token record from transfer_public_to_private")
}

/// Formats a SwapRequest struct literal suitable for Value::from_str.
fn swap_request_value(recipient: &Address<CurrentNetwork>) -> String {
    format!(
        "{{ pool: 1field, zero_for_one: true, amount_in: 1000u128, amount_out_min: 0u128, \
         sqrt_price_limit: 19029805711u128, recipient: {recipient}, nonce: 1u64, deadline: 1000000u32 }}"
    )
}

/// Formats a SwapHop struct literal.
fn swap_hop_value(pool: &str) -> String {
    format!("{{ pool: {pool}, zero_for_one: true, sqrt_price_limit: 19029805711u128 }}")
}

/// Formats a SwapMultiHopRequest struct literal suitable for Value::from_str.
fn swap_multi_hop_request_value(recipient: &Address<CurrentNetwork>) -> String {
    let hop = swap_hop_value("1field");
    format!(
        "{{ token_in: {token_in}, token_out: {token_out}, amount_in: 1000u128, \
         amount_out_min: 0u128, recipient: {recipient}, hop0: {hop}, hop1: {hop}, hop2: {hop}, \
         hop_count: 2u8, nonce: 1u64, deadline: 1000000u32, caller: {recipient} }}",
        token_in = test_token_name_field(),
        token_out = test_token_name_field(),
    )
}

/// The number of pre-generated authorizations to cycle through during measurement.
/// Criterion's warmup and sampling consume more than the sample size, so the cycle must be
/// large enough to never exhaust freshly-prepared authorizations within a single sample.
const NUM_AUTHORIZATIONS: u32 = 20;

/// A single benchmark case: its display name and the authorizations to prove.
struct BenchCase {
    name: &'static str,
    authorizations: Vec<Authorization<CurrentNetwork>>,
}

fn build_swap_authorizations(
    vm: &VM<CurrentNetwork, CurrentStorage>,
    caller_private_key: &PrivateKey<CurrentNetwork>,
    rng: &mut TestRng,
) -> Vec<Authorization<CurrentNetwork>> {
    let address = Address::try_from(caller_private_key).unwrap();
    let token_field = test_token_name_field();
    let swap_req = swap_request_value(&address);

    (0..NUM_AUTHORIZATIONS)
        .map(|_| {
            let inputs = [
                Value::<CurrentNetwork>::from_str(&swap_req).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
            ];
            vm.authorize(caller_private_key, "leo_amm.aleo", "swap", inputs, rng).unwrap()
        })
        .collect()
}

fn build_swap_private_authorizations(
    vm: &VM<CurrentNetwork, CurrentStorage>,
    caller_private_key: &PrivateKey<CurrentNetwork>,
    record: &Record<CurrentNetwork, Plaintext<CurrentNetwork>>,
    rng: &mut TestRng,
) -> Vec<Authorization<CurrentNetwork>> {
    let address = Address::try_from(caller_private_key).unwrap();
    let view_key = view_key_scalar(caller_private_key);
    let token_field = test_token_name_field();

    // swap_private(view_key, counter, blinded_address, token_record, pool, zero_for_one,
    //              amount_in, amount_out_min, sqrt_price_limit, nonce, deadline, token0, token1)
    (0..NUM_AUTHORIZATIONS)
        .map(|i| {
            let blinded = derive_blinded_address(&address, &view_key, i);
            let inputs = [
                Value::<CurrentNetwork>::from_str(&format!("{view_key}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{i}u32")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{blinded}")).unwrap(),
                Value::<CurrentNetwork>::Record(record.clone()),
                Value::<CurrentNetwork>::from_str("1field").unwrap(),
                Value::<CurrentNetwork>::from_str("true").unwrap(),
                Value::<CurrentNetwork>::from_str("1000u128").unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
                Value::<CurrentNetwork>::from_str("19029805711u128").unwrap(),
                Value::<CurrentNetwork>::from_str("1u64").unwrap(),
                Value::<CurrentNetwork>::from_str("1000000u32").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
            ];
            vm.authorize(caller_private_key, "leo_amm.aleo", "swap_private", inputs, rng).unwrap()
        })
        .collect()
}

fn build_claim_swap_output_authorizations(
    vm: &VM<CurrentNetwork, CurrentStorage>,
    caller_private_key: &PrivateKey<CurrentNetwork>,
    rng: &mut TestRng,
) -> Vec<Authorization<CurrentNetwork>> {
    let address = Address::try_from(caller_private_key).unwrap();
    let token_field = test_token_name_field();

    (0..NUM_AUTHORIZATIONS)
        .map(|_| {
            let inputs = [
                Value::<CurrentNetwork>::from_str("1field").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str("500u128").unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{address}")).unwrap(),
            ];
            vm.authorize(caller_private_key, "leo_amm.aleo", "claim_swap_output", inputs, rng).unwrap()
        })
        .collect()
}

fn build_claim_swap_output_private_authorizations(
    vm: &VM<CurrentNetwork, CurrentStorage>,
    caller_private_key: &PrivateKey<CurrentNetwork>,
    rng: &mut TestRng,
) -> Vec<Authorization<CurrentNetwork>> {
    let address = Address::try_from(caller_private_key).unwrap();
    let view_key = view_key_scalar(caller_private_key);
    let token_field = test_token_name_field();

    // claim_swap_output_private(view_key, counter, blinded_address, swap_id,
    //                           token_out, token_in, amount_out, amount_remaining)
    (0..NUM_AUTHORIZATIONS)
        .map(|i| {
            let blinded = derive_blinded_address(&address, &view_key, i);
            let inputs = [
                Value::<CurrentNetwork>::from_str(&format!("{view_key}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{i}u32")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{blinded}")).unwrap(),
                Value::<CurrentNetwork>::from_str("1field").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str("500u128").unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
            ];
            vm.authorize(caller_private_key, "leo_amm.aleo", "claim_swap_output_private", inputs, rng).unwrap()
        })
        .collect()
}

fn build_swap_multi_hop_authorizations(
    vm: &VM<CurrentNetwork, CurrentStorage>,
    caller_private_key: &PrivateKey<CurrentNetwork>,
    rng: &mut TestRng,
) -> Vec<Authorization<CurrentNetwork>> {
    let address = Address::try_from(caller_private_key).unwrap();
    let multi_hop_req = swap_multi_hop_request_value(&address);

    (0..NUM_AUTHORIZATIONS)
        .map(|_| {
            let inputs = [Value::<CurrentNetwork>::from_str(&multi_hop_req).unwrap()];
            vm.authorize(caller_private_key, "leo_amm.aleo", "swap_multi_hop", inputs, rng).unwrap()
        })
        .collect()
}

fn build_swap_multi_hop_private_authorizations(
    vm: &VM<CurrentNetwork, CurrentStorage>,
    caller_private_key: &PrivateKey<CurrentNetwork>,
    record: &Record<CurrentNetwork, Plaintext<CurrentNetwork>>,
    rng: &mut TestRng,
) -> Vec<Authorization<CurrentNetwork>> {
    let address = Address::try_from(caller_private_key).unwrap();
    let view_key = view_key_scalar(caller_private_key);
    let token_field = test_token_name_field();
    let hop = swap_hop_value("1field");

    // swap_multi_hop_private(view_key, counter, blinded_address, token_record, token_in, token_out,
    //                        amount_in, amount_out_min, hop0, hop1, hop2, hop_count, nonce, deadline)
    (0..NUM_AUTHORIZATIONS)
        .map(|i| {
            let blinded = derive_blinded_address(&address, &view_key, i);
            let inputs = [
                Value::<CurrentNetwork>::from_str(&format!("{view_key}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{i}u32")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{blinded}")).unwrap(),
                Value::<CurrentNetwork>::Record(record.clone()),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str("1000u128").unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
                Value::<CurrentNetwork>::from_str(&hop).unwrap(),
                Value::<CurrentNetwork>::from_str(&hop).unwrap(),
                Value::<CurrentNetwork>::from_str(&hop).unwrap(),
                Value::<CurrentNetwork>::from_str("2u8").unwrap(),
                Value::<CurrentNetwork>::from_str("1u64").unwrap(),
                Value::<CurrentNetwork>::from_str("1000000u32").unwrap(),
            ];
            vm.authorize(caller_private_key, "leo_amm.aleo", "swap_multi_hop_private", inputs, rng).unwrap()
        })
        .collect()
}

fn build_claim_multi_hop_output_authorizations(
    vm: &VM<CurrentNetwork, CurrentStorage>,
    caller_private_key: &PrivateKey<CurrentNetwork>,
    rng: &mut TestRng,
) -> Vec<Authorization<CurrentNetwork>> {
    let address = Address::try_from(caller_private_key).unwrap();
    let token_field = test_token_name_field();

    (0..NUM_AUTHORIZATIONS)
        .map(|_| {
            let inputs = [
                Value::<CurrentNetwork>::from_str("1field").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str("500u128").unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{address}")).unwrap(),
            ];
            vm.authorize(caller_private_key, "leo_amm.aleo", "claim_multi_hop_output", inputs, rng).unwrap()
        })
        .collect()
}

fn build_claim_multi_hop_output_private_authorizations(
    vm: &VM<CurrentNetwork, CurrentStorage>,
    caller_private_key: &PrivateKey<CurrentNetwork>,
    rng: &mut TestRng,
) -> Vec<Authorization<CurrentNetwork>> {
    let address = Address::try_from(caller_private_key).unwrap();
    let view_key = view_key_scalar(caller_private_key);
    let token_field = test_token_name_field();

    // claim_multi_hop_output_private(view_key, counter, blinded_address, swap_id,
    //                                token_out, token_in, amount_out, amount_remaining,
    //                                token_in_1, amount_remaining_1, token_in_2, amount_remaining_2)
    (0..NUM_AUTHORIZATIONS)
        .map(|i| {
            let blinded = derive_blinded_address(&address, &view_key, i);
            let inputs = [
                Value::<CurrentNetwork>::from_str(&format!("{view_key}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{i}u32")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{blinded}")).unwrap(),
                Value::<CurrentNetwork>::from_str("1field").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str("500u128").unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
            ];
            vm.authorize(caller_private_key, "leo_amm.aleo", "claim_multi_hop_output_private", inputs, rng).unwrap()
        })
        .collect()
}

/// Benchmarks proof generation (`execute_authorization`) for all AMM functions.
///
/// All operations that require *verifying* a cuVaruna proof (deploying `test_token` and minting
/// the spendable records) are performed up front, before any measurement runs. This matters
/// because the heavy GPU proving during measurement leaves cuVaruna's GPU state in a condition
/// where subsequent proof verification is unreliable; doing every verified, on-chain setup step
/// first keeps it on a clean GPU.
fn bench_amm(c: &mut Criterion) {
    let mut rng = TestRng::fixed(42);

    // Prepare a single shared VM (genesis, programs, advance to V14, deploy test_token).
    let (vm, caller_private_key) = prepare_amm_vm(&mut rng);

    // Mint the records the private swaps spend (verified, on-chain, before any measurement).
    let swap_record = mint_test_token_record(&vm, &caller_private_key, 1_000_000u128, &mut rng);
    let multi_hop_record = mint_test_token_record(&vm, &caller_private_key, 1_000_000u128, &mut rng);

    // Build every authorization set (circuit tracing only; no proving/verification yet).
    let cases = vec![
        BenchCase { name: "amm::swap", authorizations: build_swap_authorizations(&vm, &caller_private_key, &mut rng) },
        BenchCase {
            name: "amm::swap_private",
            authorizations: build_swap_private_authorizations(&vm, &caller_private_key, &swap_record, &mut rng),
        },
        BenchCase {
            name: "amm::claim_swap_output",
            authorizations: build_claim_swap_output_authorizations(&vm, &caller_private_key, &mut rng),
        },
        BenchCase {
            name: "amm::claim_swap_output_private",
            authorizations: build_claim_swap_output_private_authorizations(&vm, &caller_private_key, &mut rng),
        },
        BenchCase {
            name: "amm::swap_multi_hop",
            authorizations: build_swap_multi_hop_authorizations(&vm, &caller_private_key, &mut rng),
        },
        BenchCase {
            name: "amm::swap_multi_hop_private",
            authorizations: build_swap_multi_hop_private_authorizations(
                &vm,
                &caller_private_key,
                &multi_hop_record,
                &mut rng,
            ),
        },
        BenchCase {
            name: "amm::claim_multi_hop_output",
            authorizations: build_claim_multi_hop_output_authorizations(&vm, &caller_private_key, &mut rng),
        },
        BenchCase {
            name: "amm::claim_multi_hop_output_private",
            authorizations: build_claim_multi_hop_output_private_authorizations(&vm, &caller_private_key, &mut rng),
        },
    ];

    // Measure proof generation for each case, cycling through its pre-built authorizations.
    for case in &cases {
        let mut index = 0usize;
        c.bench_function(case.name, |b| {
            b.iter(|| {
                let authorization = case.authorizations[index % case.authorizations.len()].clone();
                index += 1;
                std::hint::black_box(vm.execute_authorization(authorization, None, None, &mut rng).unwrap())
            })
        });
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_amm
}
criterion_main!(benches);
