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

use console::{
    account::{ComputeKey, PrivateKey},
    network::Network,
    prelude::{ToBits, ToFieldsRaw},
    program::{Address, Literal, Plaintext, ProgramID, Value},
    types::{Field, Scalar, U32},
};
use std::sync::OnceLock;

// A program that contains `test_blinded_address`, a function that verifies a BHP256
// commitment of `self.signer` whose randomizer is derived by hashing four fields:
//   [cast(leo_amm.aleo), r1, cast(r0), cast(r2)]
// It also checks that `self.signer` equals `aleo::GENERATOR * r0`, so the caller
// must supply the view-key scalar as `r0`.
const BLINDED_ADDRESS_PROGRAM: &str = r"
program leo_amm_test.aleo;

function test_blinded_address:
    input r0 as scalar.private;
    input r1 as field.private;
    input r2 as u32.private;
    input r3 as address.private;
    cast leo_amm.aleo into r4 as field;
    cast r0 into r5 as field;
    cast r2 into r6 as field;
    cast r4 r1 r5 r6 into r7 as [field; 4u32];
    hash.psd4.raw r7 into r8 as scalar;
    commit.bhp256 self.signer r8 into r9 as address;
    assert.eq r9 r3;
    mul aleo::GENERATOR r0 into r10;
    cast r10 into r11 as address;
    is.eq r11 self.signer into r12;
    assert.eq r12 true;

constructor:
    assert.eq true true;
";

// Deploys `leo_amm_test.aleo` at V15 and executes `test_blinded_address` with
// inputs that are computed natively to satisfy all the in-function assertions.
#[test]
fn test_blinded_address_deploy_and_execute() {


    let private_key = PrivateKey::<TestnetV0>::from_str(
        "APrivateKey1zkp8CZNn3yeCseEtxuVPbDCwSyhGW6yZKUYKfgXmcpoGPWH").unwrap();
    let view_key = ViewKey::<TestnetV0>::try_from(&private_key).unwrap();
    let signer_address = Address::try_from(&private_key).unwrap();

    // ── Inputs ────────────────────────────────────────────────────────────────────
    // Paste hardcoded values from the JS side here.
    let contract_address: &str = "aleo1nqgg0aj6ruk9w67gx4ehg4278uj3sgjlgxlmaf7jdwl4amxy5spq06psk5";            // JS: contractAddress (program address, "aleo1...")
    let counter_value: u32 = 100;                 // JS: counterValue    (u32 integer)
    let expected_blinded_address: &str = "aleo1x8y7kew7upx5vr9sy44h9usq5cts6pd2jd5vuqwlrt4lvze7rq8q3jkew9";    // JS: return value of deriveBlindedAddress

    // ── Step 1: contract address → group → x-coordinate (field) ──────────────────
    // JS: Address.from_string(contractAddress).toGroup().toXCoordinate()
    let contract_addr_group = *Address::<TestnetV0>::from_str(contract_address).unwrap().to_group();
    let contract_addr_field = contract_addr_group.to_x_coordinate();
    println!("contract_addr_field : {contract_addr_field}");

    // ── Step 2: view key scalar → field ──────────────────────────────────────────
    // JS: Scalar.fromString(viewKeyScalar).toField()
    let view_key_field = view_key.deref().to_field().unwrap();
    println!("view_key_field      : {view_key_field}");

    println!("signer_address      : {signer_address}");

    // ── Step 3: counter (u32) → field ────────────────────────────────────────────
    // Program: `cast r2 into r6 as field` (integer directly to field, not via scalar).
    let counter_field = U32::<TestnetV0>::from_str(&format!("{counter_value}u32")).unwrap().to_field().unwrap();
    println!("counter_field       : {counter_field}");

    // ── Step 4: domain separator ──────────────────────────────────────────────────
    // JS: domainSeparator (already a Field)
    let domain_sep = Field::<TestnetV0>::one();
    println!("domain_sep          : {domain_sep}");

    // ── Step 5: hash.psd4.raw on [field; 4u32] → scalar ──────────────────────────
    // Program: `hash.psd4.raw r7 into r8 as scalar`.
    // `.raw` calls `to_fields_raw()` on the Plaintext::Array (no type tags, raw bits packed
    // into 252-bit chunks), feeds them to `hash_psd4`, then casts via `from_field_lossy`.
    let hash_input = Plaintext::<TestnetV0>::Array(
        vec![
            Plaintext::from(Literal::Field(contract_addr_field)),
            Plaintext::from(Literal::Field(domain_sep)),
            Plaintext::from(Literal::Field(view_key_field)),
            Plaintext::from(Literal::Field(counter_field)),
        ],
        OnceLock::new(),
    );
    let r_scalar =
        Scalar::<TestnetV0>::from_field_lossy(&TestnetV0::hash_psd4(&hash_input.to_fields_raw().unwrap()).unwrap());
    println!("r_scalar            : {r_scalar}");

    // ── Step 6: BHP256::commit_to_group ───────────────────────────────────────────
    // JS: bhp256.commitToGroup(signerValue.toBitsLe(), rScalar)
    let signer_value = Value::<TestnetV0>::try_from(signer_address.to_string()).unwrap();
    let blinded_group = TestnetV0::commit_to_group_bhp256(&signer_value.to_bits_le(), &r_scalar).unwrap();
    println!("blinded_group       : {blinded_group}");

    // ── Step 7: group → address ───────────────────────────────────────────────────
    // JS: Address.fromGroup(blindedGroup).toString()
    let blinded_address = Address::<TestnetV0>::new(blinded_group).to_string();
    println!("blinded_address     : {blinded_address}");

    // contractAddress:  aleo1nqgg0aj6ruk9w67gx4ehg4278uj3sgjlgxlmaf7jdwl4amxy5spq06psk5
    // contractAddrField:  1195747730599673728983299172212634796511534717895854557016861533415619891352field
    // View Key:  334926304971763782347498121479281870911723639068413954564748091722770623877scalar
    // View Key Field: 334926304971763782347498121479281870911723639068413954564748091722770623877field
    // Signer Address:  aleo1rhgdu77hgyqd3xjj8ucu3jj9r2krwz6mnzyd80gncr5fxcwlh5rsvzp9px
    // Counter value:  100
    // Counter Field:  100field
    // rScalar:  309168300562102088075840658801013900727390842867613304070511786695956920830scalar
    // blindedGroup:  2832556485304577360329315942759290450535541855923054498006983058988958402821group
    // Derived blinded address:  aleo1qhymv5ac33x2feem5nzq3sus20t9k273yzwdgp6c5rn9lretgvrqwgttrp


    // Execution error: Stack evaluation failed: Instruction (assert.eq r21 r3;) at index 6 failed: 'assert.eq' failed: 'aleo1x8y7kew7upx5vr9sy44h9usq5cts6pd2jd5vuqwlrt4lvze7rq8q3jkew9' is not equal to 'aleo1qhymv5ac33x2feem5nzq3sus20t9k273yzwdgp6c5rn9lretgvrqwgttrp' (should be equal)



    // ------------------------------------------------------------------------------------------------------------------------------------------------------------

    let rng = &mut TestRng::default();
    let deployer_private_key = sample_genesis_private_key(rng);

    // `aleo::GENERATOR` (V14 syntax) requires at least V14; deploy at V15.
    let v15_height = CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap();
    let vm = sample_vm_at_height(v15_height, rng);

    // Deploy the program.
    let program = Program::from_str(BLINDED_ADDRESS_PROGRAM).unwrap();
    let deployment = vm.deploy(&deployer_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(&vm, &deployer_private_key, &[deployment], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "Deployment should succeed at V15");
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    // Fund the new caller with 10M credits.
    let transaction = vm
        .execute(
            &deployer_private_key,
            ("credits.aleo", "transfer_public"),
            vec![
                Value::from_str(&format!("{signer_address}")).unwrap(),
                Value::from_str("10_000_000_000_000u64").unwrap(),
            ]
                .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();
    let block = sample_next_block(&vm, &deployer_private_key, &[transaction], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1);
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
    vm.add_next_block(&block).unwrap();

    // Use the same private key as `test_derive_blinded_address` in verify.rs so that both
    // tests validate the same expected blinded address.
    // Safe: this key is a hardcoded test vector, not a secret.
    let exec_private_key = PrivateKey::<CurrentNetwork>::from_str(
        "APrivateKey1zkp8CZNn3yeCseEtxuVPbDCwSyhGW6yZKUYKfgXmcpoGPWH",
    )
    .unwrap();

    // Derive r0 = view_key = sk_sig + r_sig + sk_prf.
    // The function asserts `self.signer == aleo::GENERATOR * r0`, which holds when r0
    // is the caller's view-key scalar (since address = G * view_key).
    let compute_key = ComputeKey::try_from(&exec_private_key).unwrap();
    let r0: Scalar<CurrentNetwork> =
        exec_private_key.sk_sig() + exec_private_key.r_sig() + compute_key.sk_prf();

    // The signer address that the VM will use for self.signer during execution.
    let signer = Address::<CurrentNetwork>::try_from(&exec_private_key).unwrap();

    // Fixed inputs matching test_derive_blinded_address: domain separator = 1field, counter = 100.
    let r1 = Field::<CurrentNetwork>::one();
    let r2 = U32::<CurrentNetwork>::new(100u32);

    // r4 = cast(leo_amm.aleo) as field.
    // The ProgramID operand loads as Address(hash_to_group_psd4([name, network]));
    // casting Address → Field takes the x-coordinate of the underlying group element.
    let r4 = ProgramID::<CurrentNetwork>::from_str("leo_amm.aleo")
        .unwrap()
        .to_address()
        .unwrap()
        .to_group()
        .to_x_coordinate();

    // r5 = cast(r0) as field  →  scalar.to_field().
    let r5 = r0.to_field().unwrap();

    // r6 = cast(r2) as field  →  integer.to_field().
    let r6 = r2.to_field().unwrap();

    // r8 = hash.psd4.raw([r4, r1, r5, r6]) as scalar.
    // The `.raw` variant calls `to_fields_raw()` on the plaintext value then feeds the
    // resulting field elements into `hash_psd4`, and finally casts the output to scalar.
    let array_plaintext = Plaintext::<CurrentNetwork>::Array(
        vec![
            Plaintext::from(Literal::Field(r4)),
            Plaintext::from(Literal::Field(r1)),
            Plaintext::from(Literal::Field(r5)),
            Plaintext::from(Literal::Field(r6)),
        ],
        OnceLock::new(),
    );
    let raw_fields = array_plaintext.to_fields_raw().unwrap();
    let r8 = Scalar::<CurrentNetwork>::from_field_lossy(&CurrentNetwork::hash_psd4(&raw_fields).unwrap());

    // r3 = commit.bhp256(self.signer, r8) as address.
    // `commit.bhp256` (non-raw) uses the structured `to_bits_le()` representation of the input.
    let signer_bits =
        Value::<CurrentNetwork>::Plaintext(Plaintext::from(Literal::Address(signer))).to_bits_le();
    let r3 = Address::<CurrentNetwork>::new(
        CurrentNetwork::commit_to_group_bhp256(&signer_bits, &r8).unwrap(),
    );

    // Verify the natively-computed blinded address matches the JS SDK expected value.
    assert_eq!(
        r3.to_string(),
        "aleo1x8y7kew7upx5vr9sy44h9usq5cts6pd2jd5vuqwlrt4lvze7rq8q3jkew9",
        "native blinded address must match JS SDK expected value"
    );

    // Execute with the natively-computed inputs.
    let execution = vm
        .execute(
            &exec_private_key,
            ("leo_amm_test.aleo", "test_blinded_address"),
            [
                Value::Plaintext(Plaintext::from(Literal::Scalar(r0))),
                Value::Plaintext(Plaintext::from(Literal::Field(r1))),
                Value::Plaintext(Plaintext::from(Literal::U32(r2))),
                Value::Plaintext(Plaintext::from(Literal::Address(r3))),
            ]
            .into_iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();

    let block = sample_next_block(&vm, &deployer_private_key, &[execution], rng).unwrap();
    assert_eq!(block.transactions().num_accepted(), 1, "test_blinded_address execution should succeed");
    assert_eq!(block.transactions().num_rejected(), 0);
    assert_eq!(block.aborted_transaction_ids().len(), 0);
}
