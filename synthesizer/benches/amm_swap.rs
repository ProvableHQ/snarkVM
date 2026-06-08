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
    prelude::ToField,
    program::{Ciphertext, Identifier, Value},
    types::Field,
};
use snarkvm_ledger_block::Transition;
use snarkvm_ledger_store::{ConsensusStore, helpers::memory::ConsensusMemory};
use snarkvm_synthesizer::VM;
use snarkvm_synthesizer_program::Program;
use snarkvm_utilities::TestRng;

use indexmap::IndexMap;
use std::str::FromStr;

type CurrentNetwork = MainnetV0;
type CurrentStorage = ConsensusMemory<CurrentNetwork>;

const TOKEN_REGISTRY_PROGRAM: &str = include_str!("amm/token_registry.aleo");
const AMM_PROGRAM: &str = include_str!("amm/leo_amm.aleo");
const TEST_TOKEN_PROGRAM: &str = include_str!("amm/test_token.aleo");

/// Sets up the VM with genesis, then adds token_registry and leo_amm programs
/// directly to the process (bypassing on-chain deployment to avoid fee proof
/// version mismatches under cuvaruna).
#[allow(clippy::type_complexity)]
fn prepare_amm_vm(
    rng: &mut TestRng,
) -> (
    VM<CurrentNetwork, CurrentStorage>,
    PrivateKey<CurrentNetwork>,
    IndexMap<Field<CurrentNetwork>, snarkvm_console::program::Record<CurrentNetwork, Ciphertext<CurrentNetwork>>>,
) {
    let caller_private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();

    let vm = VM::from(ConsensusStore::open(StorageMode::new_test(None)).unwrap()).unwrap();

    let genesis = vm.genesis_beacon(&caller_private_key, rng).unwrap();
    let records = genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
    vm.add_next_block(&genesis).unwrap();

    // Add programs directly to the process (token_registry must come first as it's imported by leo_amm).
    let token_registry = Program::<CurrentNetwork>::from_str(TOKEN_REGISTRY_PROGRAM).unwrap();
    vm.process().lock().add_program(&token_registry).unwrap();

    let test_token = Program::<CurrentNetwork>::from_str(TEST_TOKEN_PROGRAM).unwrap();
    vm.process().lock().add_program(&test_token).unwrap();

    let amm = Program::<CurrentNetwork>::from_str(AMM_PROGRAM).unwrap();
    vm.process().lock().add_program(&amm).unwrap();

    (vm, caller_private_key, records)
}

/// Returns the field representation of `test_token` identifier (used as the token program
/// in dynamic calls).
fn test_token_name_field() -> Field<CurrentNetwork> {
    Identifier::<CurrentNetwork>::from_str("test_token").unwrap().to_field().unwrap()
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

fn bench_swap(c: &mut Criterion) {
    let mut rng = TestRng::fixed(42);
    let (vm, caller_private_key, _records) = prepare_amm_vm(&mut rng);
    let address = Address::try_from(&caller_private_key).unwrap();

    let token_field = test_token_name_field();
    let swap_req = swap_request_value(&address);

    let authorizations: Vec<_> = (0..20)
        .map(|_| {
            let inputs = [
                Value::<CurrentNetwork>::from_str(&swap_req).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
            ];
            vm.authorize(&caller_private_key, "leo_amm.aleo", "swap", inputs, &mut rng).unwrap()
        })
        .collect();
    let mut auth_iter = authorizations.into_iter().cycle();

    c.bench_function("amm::swap", |b| {
        b.iter(|| {
            let authorization = auth_iter.next().unwrap();
            std::hint::black_box(vm.execute_authorization(authorization, None, None, &mut rng).unwrap())
        })
    });
}

fn bench_swap_private(c: &mut Criterion) {
    let mut rng = TestRng::fixed(42);
    let (vm, caller_private_key, records) = prepare_amm_vm(&mut rng);
    let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();
    let address = Address::try_from(&caller_private_key).unwrap();

    // The private swap requires a Token record from token_registry.aleo. Attempt to get one
    // by decrypting records; if none are available, skip this benchmark.
    let Some(record) = records.values().find_map(|r| r.decrypt(&caller_view_key).ok()) else {
        eprintln!("SKIP amm::swap_private - no decryptable record available");
        return;
    };

    let token_field = test_token_name_field();

    let auth_result = {
        let inputs = [
            Value::<CurrentNetwork>::from_str("1scalar").unwrap(),
            Value::<CurrentNetwork>::from_str("0u32").unwrap(),
            Value::<CurrentNetwork>::from_str(&format!("{address}")).unwrap(),
            Value::<CurrentNetwork>::Record(record.clone()),
            Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
            Value::<CurrentNetwork>::from_str("true").unwrap(),
            Value::<CurrentNetwork>::from_str("1000u128").unwrap(),
            Value::<CurrentNetwork>::from_str("19029805711u128").unwrap(),
            Value::<CurrentNetwork>::from_str("0u128").unwrap(),
            Value::<CurrentNetwork>::from_str("1u64").unwrap(),
            Value::<CurrentNetwork>::from_str("1000000u32").unwrap(),
            Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
            Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
        ];
        vm.authorize(&caller_private_key, "leo_amm.aleo", "swap_private", inputs, &mut rng)
    };

    let Ok(first_auth) = auth_result else {
        eprintln!("SKIP amm::swap_private - authorization failed (record type mismatch): {}", auth_result.unwrap_err());
        return;
    };

    let authorizations: Vec<_> = std::iter::once(first_auth)
        .chain((1..20).map(|i| {
            let inputs = [
                Value::<CurrentNetwork>::from_str("1scalar").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{i}u32")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{address}")).unwrap(),
                Value::<CurrentNetwork>::Record(record.clone()),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str("true").unwrap(),
                Value::<CurrentNetwork>::from_str("1000u128").unwrap(),
                Value::<CurrentNetwork>::from_str("19029805711u128").unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
                Value::<CurrentNetwork>::from_str("1u64").unwrap(),
                Value::<CurrentNetwork>::from_str("1000000u32").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
            ];
            vm.authorize(&caller_private_key, "leo_amm.aleo", "swap_private", inputs, &mut rng).unwrap()
        }))
        .collect();
    let mut auth_iter = authorizations.into_iter().cycle();

    c.bench_function("amm::swap_private", |b| {
        b.iter(|| {
            let authorization = auth_iter.next().unwrap();
            std::hint::black_box(vm.execute_authorization(authorization, None, None, &mut rng).unwrap())
        })
    });
}

fn bench_claim_swap_output(c: &mut Criterion) {
    let mut rng = TestRng::fixed(42);
    let (vm, caller_private_key, _records) = prepare_amm_vm(&mut rng);
    let address = Address::try_from(&caller_private_key).unwrap();

    let token_field = test_token_name_field();

    let authorizations: Vec<_> = (0..20)
        .map(|_| {
            let inputs = [
                Value::<CurrentNetwork>::from_str("1field").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str("500u128").unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{address}")).unwrap(),
            ];
            vm.authorize(&caller_private_key, "leo_amm.aleo", "claim_swap_output", inputs, &mut rng).unwrap()
        })
        .collect();
    let mut auth_iter = authorizations.into_iter().cycle();

    c.bench_function("amm::claim_swap_output", |b| {
        b.iter(|| {
            let authorization = auth_iter.next().unwrap();
            std::hint::black_box(vm.execute_authorization(authorization, None, None, &mut rng).unwrap())
        })
    });
}

fn bench_claim_swap_output_private(c: &mut Criterion) {
    let mut rng = TestRng::fixed(42);
    let (vm, caller_private_key, _records) = prepare_amm_vm(&mut rng);
    let address = Address::try_from(&caller_private_key).unwrap();

    let token_field = test_token_name_field();

    let auth_result = {
        let inputs = [
            Value::<CurrentNetwork>::from_str("1scalar").unwrap(),
            Value::<CurrentNetwork>::from_str("0u32").unwrap(),
            Value::<CurrentNetwork>::from_str(&format!("{address}")).unwrap(),
            Value::<CurrentNetwork>::from_str("1field").unwrap(),
            Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
            Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
            Value::<CurrentNetwork>::from_str("500u128").unwrap(),
            Value::<CurrentNetwork>::from_str("0u128").unwrap(),
        ];
        vm.authorize(&caller_private_key, "leo_amm.aleo", "claim_swap_output_private", inputs, &mut rng)
    };

    let Ok(first_auth) = auth_result else {
        eprintln!("SKIP amm::claim_swap_output_private - authorization failed: {}", auth_result.unwrap_err());
        return;
    };

    // Pre-test execution: private functions require a valid blinded address which
    // we cannot easily compute in the benchmark setup.
    if vm.execute_authorization(first_auth, None, None, &mut rng).is_err() {
        eprintln!("SKIP amm::claim_swap_output_private - execution requires valid blinded address inputs");
        return;
    }

    let authorizations: Vec<_> = (0..20)
        .map(|i| {
            let inputs = [
                Value::<CurrentNetwork>::from_str("1scalar").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{i}u32")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{address}")).unwrap(),
                Value::<CurrentNetwork>::from_str("1field").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{token_field}")).unwrap(),
                Value::<CurrentNetwork>::from_str("500u128").unwrap(),
                Value::<CurrentNetwork>::from_str("0u128").unwrap(),
            ];
            vm.authorize(&caller_private_key, "leo_amm.aleo", "claim_swap_output_private", inputs, &mut rng).unwrap()
        })
        .collect();
    let mut auth_iter = authorizations.into_iter().cycle();

    c.bench_function("amm::claim_swap_output_private", |b| {
        b.iter(|| {
            let authorization = auth_iter.next().unwrap();
            std::hint::black_box(vm.execute_authorization(authorization, None, None, &mut rng).unwrap())
        })
    });
}

fn bench_swap_multi_hop(c: &mut Criterion) {
    let mut rng = TestRng::fixed(42);
    let (vm, caller_private_key, _records) = prepare_amm_vm(&mut rng);
    let address = Address::try_from(&caller_private_key).unwrap();

    let multi_hop_req = swap_multi_hop_request_value(&address);

    let authorizations: Vec<_> = (0..20)
        .map(|_| {
            let inputs = [Value::<CurrentNetwork>::from_str(&multi_hop_req).unwrap()];
            vm.authorize(&caller_private_key, "leo_amm.aleo", "swap_multi_hop", inputs, &mut rng).unwrap()
        })
        .collect();
    let mut auth_iter = authorizations.into_iter().cycle();

    c.bench_function("amm::swap_multi_hop", |b| {
        b.iter(|| {
            let authorization = auth_iter.next().unwrap();
            std::hint::black_box(vm.execute_authorization(authorization, None, None, &mut rng).unwrap())
        })
    });
}

fn bench_swap_multi_hop_private(c: &mut Criterion) {
    let mut rng = TestRng::fixed(42);
    let (vm, caller_private_key, records) = prepare_amm_vm(&mut rng);
    let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();
    let address = Address::try_from(&caller_private_key).unwrap();

    let Some(record) = records.values().find_map(|r| r.decrypt(&caller_view_key).ok()) else {
        eprintln!("SKIP amm::swap_multi_hop_private - no decryptable record available");
        return;
    };

    let token_field = test_token_name_field();
    let hop = swap_hop_value("1field");

    let auth_result = {
        let inputs = [
            Value::<CurrentNetwork>::from_str("1scalar").unwrap(),
            Value::<CurrentNetwork>::from_str("0u32").unwrap(),
            Value::<CurrentNetwork>::from_str(&format!("{address}")).unwrap(),
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
        vm.authorize(&caller_private_key, "leo_amm.aleo", "swap_multi_hop_private", inputs, &mut rng)
    };

    let Ok(first_auth) = auth_result else {
        eprintln!("SKIP amm::swap_multi_hop_private - authorization failed: {}", auth_result.unwrap_err());
        return;
    };

    let authorizations: Vec<_> = std::iter::once(first_auth)
        .chain((1..20).map(|i| {
            let inputs = [
                Value::<CurrentNetwork>::from_str("1scalar").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{i}u32")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{address}")).unwrap(),
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
            vm.authorize(&caller_private_key, "leo_amm.aleo", "swap_multi_hop_private", inputs, &mut rng).unwrap()
        }))
        .collect();
    let mut auth_iter = authorizations.into_iter().cycle();

    c.bench_function("amm::swap_multi_hop_private", |b| {
        b.iter(|| {
            let authorization = auth_iter.next().unwrap();
            std::hint::black_box(vm.execute_authorization(authorization, None, None, &mut rng).unwrap())
        })
    });
}

fn bench_claim_multi_hop_output(c: &mut Criterion) {
    let mut rng = TestRng::fixed(42);
    let (vm, caller_private_key, _records) = prepare_amm_vm(&mut rng);
    let address = Address::try_from(&caller_private_key).unwrap();

    let token_field = test_token_name_field();

    let authorizations: Vec<_> = (0..20)
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
            vm.authorize(&caller_private_key, "leo_amm.aleo", "claim_multi_hop_output", inputs, &mut rng).unwrap()
        })
        .collect();
    let mut auth_iter = authorizations.into_iter().cycle();

    c.bench_function("amm::claim_multi_hop_output", |b| {
        b.iter(|| {
            let authorization = auth_iter.next().unwrap();
            std::hint::black_box(vm.execute_authorization(authorization, None, None, &mut rng).unwrap())
        })
    });
}

fn bench_claim_multi_hop_output_private(c: &mut Criterion) {
    let mut rng = TestRng::fixed(42);
    let (vm, caller_private_key, _records) = prepare_amm_vm(&mut rng);
    let address = Address::try_from(&caller_private_key).unwrap();

    let token_field = test_token_name_field();

    let auth_result = {
        let inputs = [
            Value::<CurrentNetwork>::from_str("1scalar").unwrap(),
            Value::<CurrentNetwork>::from_str("0u32").unwrap(),
            Value::<CurrentNetwork>::from_str(&format!("{address}")).unwrap(),
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
        vm.authorize(&caller_private_key, "leo_amm.aleo", "claim_multi_hop_output_private", inputs, &mut rng)
    };

    let Ok(first_auth) = auth_result else {
        eprintln!("SKIP amm::claim_multi_hop_output_private - authorization failed: {}", auth_result.unwrap_err());
        return;
    };

    // Pre-test execution: private functions require a valid blinded address which
    // we cannot easily compute in the benchmark setup.
    if vm.execute_authorization(first_auth, None, None, &mut rng).is_err() {
        eprintln!("SKIP amm::claim_multi_hop_output_private - execution requires valid blinded address inputs");
        return;
    }

    let authorizations: Vec<_> = (0..20)
        .map(|i| {
            let inputs = [
                Value::<CurrentNetwork>::from_str("1scalar").unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{i}u32")).unwrap(),
                Value::<CurrentNetwork>::from_str(&format!("{address}")).unwrap(),
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
            vm.authorize(&caller_private_key, "leo_amm.aleo", "claim_multi_hop_output_private", inputs, &mut rng)
                .unwrap()
        })
        .collect();
    let mut auth_iter = authorizations.into_iter().cycle();

    c.bench_function("amm::claim_multi_hop_output_private", |b| {
        b.iter(|| {
            let authorization = auth_iter.next().unwrap();
            std::hint::black_box(vm.execute_authorization(authorization, None, None, &mut rng).unwrap())
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets =
        bench_swap,
        bench_swap_private,
        bench_claim_swap_output,
        bench_claim_swap_output_private,
        bench_swap_multi_hop,
        bench_swap_multi_hop_private,
        bench_claim_multi_hop_output,
        bench_claim_multi_hop_output_private
}
criterion_main!(benches);
