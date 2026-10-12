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

//! End-to-end tests for a k-of-n multisig whose owners stay anonymous within the owner set.
//!
//! The program is a variant of `shield_swap_multisig_core.aleo`. Each owner is whitelisted by a
//! shielded identity, derived as in `shield_swap.aleo`:
//! `hash.psd8.raw [program, domain, signer, salt] as address`. The salt is known only to the owner.
//!
//! A wallet stores a commitment to its owner set. To propose or approve, an owner proves in the
//! transition that its identity is in the committed set, and reveals only a nullifier derived from
//! its secret salt and the proposal ID. The nullifier prevents double approvals, but does not
//! reveal which owner approved. A relayer pays every fee, so no owner account appears on chain.
//!
//! A call proposal approves an opaque operation for a given caller. The caller program consumes the
//! approval by calling `execute_call`, as with `assert_signing_completed` in the original program.

use super::*;

const MULTISIG: &str = "private_multisig.aleo";
const TARGET: &str = "multisig_target.aleo";
// The zero address marks an empty owner slot.
const EMPTY_SLOT: &str = "aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc";
const OWNER_DOMAIN: &str = "private_multisig_owner";
const NULLIFIER_DOMAIN: &str = "private_multisig_nullifier";

// Returns the domain separator as a field, in the same encoding as the constant in `shield_swap.aleo`.
fn domain(name: &str) -> Field<CurrentNetwork> {
    Identifier::<CurrentNetwork>::from_str(name).unwrap().to_field().unwrap()
}

fn multisig_program() -> Program<CurrentNetwork> {
    let owner_domain = domain(OWNER_DOMAIN);
    let nullifier_domain = domain(NULLIFIER_DOMAIN);
    Program::from_str(&format!(
        r"
program private_multisig.aleo;

struct Wallet:
    owners_root as field;
    threshold as u8;
    epoch as u32;

struct Proposal:
    wallet_id as field;
    epoch as u32;
    approvals as u8;
    executed as boolean;

struct ProposalKey:
    wallet_id as field;
    kind as u8;
    action as field;
    nonce as u64;

struct CallAction:
    caller as address;
    operation as field;

struct OwnersAction:
    owners as [address; 4u32];
    threshold as u8;

mapping wallets:
    key as field.public;
    value as Wallet.public;

mapping proposals:
    key as field.public;
    value as Proposal.public;

mapping nullifiers:
    key as field.public;
    value as boolean.public;

closure commit_owners:
    input r0 as [address; 4u32];
    input r1 as u8;
    is.neq r0[0u32] {EMPTY_SLOT} into r2;
    is.neq r0[1u32] {EMPTY_SLOT} into r3;
    is.neq r0[2u32] {EMPTY_SLOT} into r4;
    is.neq r0[3u32] {EMPTY_SLOT} into r5;
    ternary r2 1u8 0u8 into r6;
    ternary r3 1u8 0u8 into r7;
    ternary r4 1u8 0u8 into r8;
    ternary r5 1u8 0u8 into r9;
    add r6 r7 into r10;
    add r8 r9 into r11;
    add r10 r11 into r12;
    gt r1 0u8 into r13;
    assert.eq r13 true;
    lte r1 r12 into r14;
    assert.eq r14 true;
    is.eq r0[0u32] r0[1u32] into r15;
    is.eq r0[0u32] r0[2u32] into r16;
    is.eq r0[0u32] r0[3u32] into r17;
    or r15 r16 into r18;
    or r18 r17 into r19;
    and r19 r2 into r20;
    is.eq r0[1u32] r0[2u32] into r21;
    is.eq r0[1u32] r0[3u32] into r22;
    or r21 r22 into r23;
    and r23 r3 into r24;
    is.eq r0[2u32] r0[3u32] into r25;
    and r25 r4 into r26;
    or r20 r24 into r27;
    or r27 r26 into r28;
    assert.eq r28 false;
    hash.bhp256 r0 into r29 as field;
    output r29 as field;

closure authenticate_owner:
    input r0 as address;
    input r1 as address;
    input r2 as field;
    input r3 as [address; 4u32];
    input r4 as field;
    cast r0 into r5 as field;
    cast r1 into r6 as field;
    cast r5 {owner_domain} r6 r2 into r7 as [field; 4u32];
    hash.psd8.raw r7 into r8 as address;
    assert.neq r8 {EMPTY_SLOT};
    is.eq r8 r3[0u32] into r9;
    is.eq r8 r3[1u32] into r10;
    is.eq r8 r3[2u32] into r11;
    is.eq r8 r3[3u32] into r12;
    or r9 r10 into r13;
    or r11 r12 into r14;
    or r13 r14 into r15;
    assert.eq r15 true;
    cast {nullifier_domain} r6 r2 r4 into r16 as [field; 4u32];
    hash.psd8.raw r16 into r17 as field;
    output r17 as field;

function create_wallet:
    input r0 as field.public;
    input r1 as [address; 4u32].public;
    input r2 as u8.public;
    call commit_owners r1 r2 into r3;
    cast r3 r2 0u32 into r4 as Wallet;
    async create_wallet r0 r4 into r5;
    output r5 as private_multisig.aleo/create_wallet.future;

finalize create_wallet:
    input r0 as field.public;
    input r1 as Wallet.public;
    contains wallets[r0] into r2;
    assert.eq r2 false;
    set r1 into wallets[r0];

function propose_call:
    input r0 as field.public;
    input r1 as address.private;
    input r2 as field.private;
    input r3 as u64.private;
    input r4 as [address; 4u32].private;
    input r5 as field.private;
    cast r1 r2 into r6 as CallAction;
    hash.bhp256 r6 into r7 as field;
    cast r0 1u8 r7 r3 into r8 as ProposalKey;
    hash.bhp256 r8 into r9 as field;
    call authenticate_owner private_multisig.aleo self.signer r5 r4 r9 into r10;
    hash.bhp256 r4 into r11 as field;
    async propose_call r0 r9 r11 r10 into r12;
    output r12 as private_multisig.aleo/propose_call.future;

finalize propose_call:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as field.public;
    input r3 as field.public;
    get wallets[r0] into r4;
    assert.eq r2 r4.owners_root;
    contains proposals[r1] into r5;
    assert.eq r5 false;
    set true into nullifiers[r3];
    cast r0 r4.epoch 1u8 false into r6 as Proposal;
    set r6 into proposals[r1];

function propose_owners:
    input r0 as field.public;
    input r1 as [address; 4u32].private;
    input r2 as u8.private;
    input r3 as u64.private;
    input r4 as [address; 4u32].private;
    input r5 as field.private;
    call commit_owners r1 r2 into r6;
    cast r1 r2 into r7 as OwnersAction;
    hash.bhp256 r7 into r8 as field;
    cast r0 2u8 r8 r3 into r9 as ProposalKey;
    hash.bhp256 r9 into r10 as field;
    call authenticate_owner private_multisig.aleo self.signer r5 r4 r10 into r11;
    hash.bhp256 r4 into r12 as field;
    async propose_owners r0 r10 r12 r11 into r13;
    output r13 as private_multisig.aleo/propose_owners.future;

finalize propose_owners:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as field.public;
    input r3 as field.public;
    get wallets[r0] into r4;
    assert.eq r2 r4.owners_root;
    contains proposals[r1] into r5;
    assert.eq r5 false;
    set true into nullifiers[r3];
    cast r0 r4.epoch 1u8 false into r6 as Proposal;
    set r6 into proposals[r1];

function approve:
    input r0 as field.public;
    input r1 as [address; 4u32].private;
    input r2 as field.private;
    call authenticate_owner private_multisig.aleo self.signer r2 r1 r0 into r3;
    hash.bhp256 r1 into r4 as field;
    async approve r0 r4 r3 into r5;
    output r5 as private_multisig.aleo/approve.future;

finalize approve:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as field.public;
    get proposals[r0] into r3;
    get wallets[r3.wallet_id] into r4;
    assert.eq r1 r4.owners_root;
    assert.eq r3.epoch r4.epoch;
    assert.eq r3.executed false;
    contains nullifiers[r2] into r5;
    assert.eq r5 false;
    set true into nullifiers[r2];
    add r3.approvals 1u8 into r6;
    cast r3.wallet_id r3.epoch r6 r3.executed into r7 as Proposal;
    set r7 into proposals[r0];

function execute_call:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as u64.public;
    cast self.caller r1 into r3 as CallAction;
    hash.bhp256 r3 into r4 as field;
    cast r0 1u8 r4 r2 into r5 as ProposalKey;
    hash.bhp256 r5 into r6 as field;
    async execute_call r6 into r7;
    output r7 as private_multisig.aleo/execute_call.future;

finalize execute_call:
    input r0 as field.public;
    get proposals[r0] into r1;
    get wallets[r1.wallet_id] into r2;
    assert.eq r1.epoch r2.epoch;
    assert.eq r1.executed false;
    gte r1.approvals r2.threshold into r3;
    assert.eq r3 true;
    cast r1.wallet_id r1.epoch r1.approvals true into r4 as Proposal;
    set r4 into proposals[r0];

function execute_owners:
    input r0 as field.public;
    input r1 as [address; 4u32].public;
    input r2 as u8.public;
    input r3 as u64.public;
    call commit_owners r1 r2 into r4;
    cast r1 r2 into r5 as OwnersAction;
    hash.bhp256 r5 into r6 as field;
    cast r0 2u8 r6 r3 into r7 as ProposalKey;
    hash.bhp256 r7 into r8 as field;
    async execute_owners r8 r4 r2 into r9;
    output r9 as private_multisig.aleo/execute_owners.future;

finalize execute_owners:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as u8.public;
    get proposals[r0] into r3;
    get wallets[r3.wallet_id] into r4;
    assert.eq r3.epoch r4.epoch;
    assert.eq r3.executed false;
    gte r3.approvals r4.threshold into r5;
    assert.eq r5 true;
    cast r3.wallet_id r3.epoch r3.approvals true into r6 as Proposal;
    set r6 into proposals[r0];
    add r4.epoch 1u32 into r7;
    cast r1 r2 r7 into r8 as Wallet;
    set r8 into wallets[r3.wallet_id];

constructor:
    assert.eq true true;
"
    ))
    .unwrap()
}

// A program that sets a per-wallet value once the wallet approves the operation.
fn target_program() -> Program<CurrentNetwork> {
    Program::from_str(
        r"
import private_multisig.aleo;

program multisig_target.aleo;

mapping values:
    key as field.public;
    value as u64.public;

function set_value:
    input r0 as field.public;
    input r1 as u64.public;
    input r2 as u64.public;
    hash.bhp256 r1 into r3 as field;
    call private_multisig.aleo/execute_call r0 r3 r2 into r4;
    async set_value r4 r0 r1 into r5;
    output r5 as multisig_target.aleo/set_value.future;

finalize set_value:
    input r0 as private_multisig.aleo/execute_call.future;
    input r1 as field.public;
    input r2 as u64.public;
    await r0;
    set r2 into values[r1];

constructor:
    assert.eq true true;
",
    )
    .unwrap()
}

// Returns the result of `hash.bhp256 <plaintext> into <register> as field`.
fn bhp256(plaintext: &str) -> Field<CurrentNetwork> {
    let plaintext = Plaintext::<CurrentNetwork>::from_str(plaintext).unwrap();
    CurrentNetwork::hash_to_group_bhp256(&plaintext.to_bits_le()).unwrap().to_x_coordinate()
}

// Formats the identities as an owner set, with empty slots at the end.
fn owner_set(identities: &[Address<CurrentNetwork>]) -> String {
    let mut slots: Vec<String> = identities.iter().map(ToString::to_string).collect();
    slots.resize(4, EMPTY_SLOT.to_string());
    format!("[{}]", slots.join(", "))
}

// The operation that `multisig_target.aleo/set_value` asks the wallet to approve.
fn set_value_operation(value: u64) -> Field<CurrentNetwork> {
    bhp256(&format!("{value}u64"))
}

fn call_proposal_id(
    wallet_id: Field<CurrentNetwork>,
    operation: Field<CurrentNetwork>,
    nonce: u64,
) -> Field<CurrentNetwork> {
    let caller = ProgramID::<CurrentNetwork>::from_str(TARGET).unwrap().to_address().unwrap();
    let action = bhp256(&format!("{{ caller: {caller}, operation: {operation} }}"));
    bhp256(&format!("{{ wallet_id: {wallet_id}, kind: 1u8, action: {action}, nonce: {nonce}u64 }}"))
}

fn owners_proposal_id(
    wallet_id: Field<CurrentNetwork>,
    owners: &str,
    threshold: u8,
    nonce: u64,
) -> Field<CurrentNetwork> {
    let action = bhp256(&format!("{{ owners: {owners}, threshold: {threshold}u8 }}"));
    bhp256(&format!("{{ wallet_id: {wallet_id}, kind: 2u8, action: {action}, nonce: {nonce}u64 }}"))
}

// An account together with the secret salt that blinds its owner identity.
struct Owner {
    private_key: PrivateKey<CurrentNetwork>,
    salt: Field<CurrentNetwork>,
}

impl Owner {
    fn new(rng: &mut TestRng) -> Self {
        Self { private_key: PrivateKey::new(rng).unwrap(), salt: Uniform::rand(rng) }
    }

    fn address(&self) -> Address<CurrentNetwork> {
        Address::try_from(&self.private_key).unwrap()
    }

    // Returns the shielded identity `hash.psd8.raw [program, domain, signer, salt] as address`.
    fn identity(&self) -> Address<CurrentNetwork> {
        self.identity_with_salt(self.salt)
    }

    fn identity_with_salt(&self, salt: Field<CurrentNetwork>) -> Address<CurrentNetwork> {
        let program = ProgramID::<CurrentNetwork>::from_str(MULTISIG).unwrap().to_address().unwrap();
        let preimage = Plaintext::<CurrentNetwork>::from_str(&format!(
            "[{}, {}, {}, {salt}]",
            program.to_field().unwrap(),
            domain(OWNER_DOMAIN),
            self.address().to_field().unwrap(),
        ))
        .unwrap();
        Address::new(CurrentNetwork::hash_to_group_psd8(&preimage.to_fields_raw().unwrap()).unwrap())
    }
}

struct TestEnv {
    vm: VM<CurrentNetwork, LedgerType>,
    // Creates the wallet, pays every fee, and produces every block.
    relayer: PrivateKey<CurrentNetwork>,
    owners: Vec<Owner>,
    // The initial owner set of `wallet_id`.
    owner_set: String,
    wallet_id: Field<CurrentNetwork>,
}

impl TestEnv {
    const THRESHOLD: u8 = 2;

    // Deploys both programs and creates a 2-of-3 wallet.
    fn new(rng: &mut TestRng) -> Self {
        let relayer = sample_genesis_private_key(rng);
        let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::latest()).unwrap(), rng);
        for program in [multisig_program(), target_program()] {
            let deployment = vm.deploy(&relayer, &program, None, 0, None, rng).unwrap();
            add_and_test_with_costs(&vm, &relayer, None, &[deployment], rng);
        }

        let owners: Vec<_> = (0..3).map(|_| Owner::new(rng)).collect();
        let owner_set = owner_set(&owners.iter().map(Owner::identity).collect::<Vec<_>>());
        let env = Self { vm, relayer, owners, owner_set, wallet_id: Uniform::rand(rng) };

        let transaction = env.create_wallet(env.wallet_id, &env.owner_set, Self::THRESHOLD, rng).unwrap();
        env.assert_accepted(transaction, rng);
        env
    }

    // Executes the function as `signer`, with the fee paid by the relayer.
    fn execute(
        &self,
        signer: &PrivateKey<CurrentNetwork>,
        program: &str,
        function: &str,
        inputs: &[String],
        rng: &mut TestRng,
    ) -> Result<Transaction<CurrentNetwork>> {
        let authorization = self.vm.authorize(signer, program, function, inputs, rng)?;
        let transaction = self.vm.execute_authorization(authorization, None, None, rng)?;
        let execution = transaction.execution().ok_or_else(|| anyhow!("Expected an execution"))?.clone();
        let (cost, _) = execution_cost(self.vm.process(), &execution, ConsensusVersion::latest())?;
        let fee_authorization =
            self.vm.authorize_fee_public(&self.relayer, cost, 0, execution.to_execution_id()?, rng)?;
        let fee = self.vm.execute_fee_authorization(fee_authorization, None, rng)?;
        Transaction::from_execution(execution, Some(fee))
    }

    fn add_block(&self, transaction: Transaction<CurrentNetwork>, rng: &mut TestRng) -> Block<CurrentNetwork> {
        self.vm.check_transaction(&transaction, None, rng).unwrap();
        let block = sample_next_block(&self.vm, &self.relayer, &[transaction], rng).unwrap();
        self.vm.add_next_block(&block).unwrap();
        block
    }

    fn assert_accepted(&self, transaction: Transaction<CurrentNetwork>, rng: &mut TestRng) {
        let block = self.add_block(transaction, rng);
        assert_eq!(block.transactions().num_accepted(), 1, "expected the transaction to be accepted");
        assert_eq!(block.aborted_transaction_ids().len(), 0);
    }

    // Asserts that the transaction is valid, but that its finalize fails.
    fn assert_rejected(&self, transaction: Transaction<CurrentNetwork>, rng: &mut TestRng) {
        let block = self.add_block(transaction, rng);
        assert_eq!(block.transactions().num_rejected(), 1, "expected the finalize to fail");
        assert_eq!(block.aborted_transaction_ids().len(), 0);
    }

    fn create_wallet(
        &self,
        wallet_id: Field<CurrentNetwork>,
        owner_set: &str,
        threshold: u8,
        rng: &mut TestRng,
    ) -> Result<Transaction<CurrentNetwork>> {
        let inputs = [wallet_id.to_string(), owner_set.to_string(), format!("{threshold}u8")];
        self.execute(&self.relayer, MULTISIG, "create_wallet", &inputs, rng)
    }

    fn propose_call(
        &self,
        owner: &Owner,
        owner_set: &str,
        value: u64,
        nonce: u64,
        rng: &mut TestRng,
    ) -> Result<Transaction<CurrentNetwork>> {
        self.propose_call_with_salt(owner, owner.salt, owner_set, value, nonce, rng)
    }

    fn propose_call_with_salt(
        &self,
        owner: &Owner,
        salt: Field<CurrentNetwork>,
        owner_set: &str,
        value: u64,
        nonce: u64,
        rng: &mut TestRng,
    ) -> Result<Transaction<CurrentNetwork>> {
        let caller = ProgramID::<CurrentNetwork>::from_str(TARGET).unwrap().to_address().unwrap();
        let inputs = [
            self.wallet_id.to_string(),
            caller.to_string(),
            set_value_operation(value).to_string(),
            format!("{nonce}u64"),
            owner_set.to_string(),
            salt.to_string(),
        ];
        self.execute(&owner.private_key, MULTISIG, "propose_call", &inputs, rng)
    }

    fn propose_owners(
        &self,
        owner: &Owner,
        owner_set: &str,
        new_owner_set: &str,
        new_threshold: u8,
        nonce: u64,
        rng: &mut TestRng,
    ) -> Result<Transaction<CurrentNetwork>> {
        let inputs = [
            self.wallet_id.to_string(),
            new_owner_set.to_string(),
            format!("{new_threshold}u8"),
            format!("{nonce}u64"),
            owner_set.to_string(),
            owner.salt.to_string(),
        ];
        self.execute(&owner.private_key, MULTISIG, "propose_owners", &inputs, rng)
    }

    fn approve(
        &self,
        owner: &Owner,
        owner_set: &str,
        proposal_id: Field<CurrentNetwork>,
        rng: &mut TestRng,
    ) -> Result<Transaction<CurrentNetwork>> {
        let inputs = [proposal_id.to_string(), owner_set.to_string(), owner.salt.to_string()];
        self.execute(&owner.private_key, MULTISIG, "approve", &inputs, rng)
    }

    // Asks the target program to consume an approved `set_value` operation. Anyone may submit it.
    fn set_value(&self, value: u64, nonce: u64, rng: &mut TestRng) -> Result<Transaction<CurrentNetwork>> {
        let inputs = [self.wallet_id.to_string(), format!("{value}u64"), format!("{nonce}u64")];
        self.execute(&self.relayer, TARGET, "set_value", &inputs, rng)
    }

    fn execute_owners(
        &self,
        new_owner_set: &str,
        new_threshold: u8,
        nonce: u64,
        rng: &mut TestRng,
    ) -> Result<Transaction<CurrentNetwork>> {
        let inputs = [
            self.wallet_id.to_string(),
            new_owner_set.to_string(),
            format!("{new_threshold}u8"),
            format!("{nonce}u64"),
        ];
        self.execute(&self.relayer, MULTISIG, "execute_owners", &inputs, rng)
    }

    fn mapping_value(&self, program: &str, mapping: &str, key: Field<CurrentNetwork>) -> Option<Value<CurrentNetwork>> {
        self.vm
            .finalize_store()
            .get_value_confirmed(
                ProgramID::from_str(program).unwrap(),
                Identifier::from_str(mapping).unwrap(),
                &Plaintext::from(Literal::Field(key)),
            )
            .unwrap()
    }

    fn wallet(&self) -> Value<CurrentNetwork> {
        self.mapping_value(MULTISIG, "wallets", self.wallet_id).unwrap()
    }

    fn proposal(&self, proposal_id: Field<CurrentNetwork>) -> Option<Value<CurrentNetwork>> {
        self.mapping_value(MULTISIG, "proposals", proposal_id)
    }

    fn expected_wallet(&self, owner_set: &str, threshold: u8, epoch: u32) -> Value<CurrentNetwork> {
        Value::from_str(&format!(
            "{{ owners_root: {}, threshold: {threshold}u8, epoch: {epoch}u32 }}",
            bhp256(owner_set)
        ))
        .unwrap()
    }

    fn expected_proposal(&self, epoch: u32, approvals: u8, executed: bool) -> Value<CurrentNetwork> {
        Value::from_str(&format!(
            "{{ wallet_id: {}, epoch: {epoch}u32, approvals: {approvals}u8, executed: {executed} }}",
            self.wallet_id
        ))
        .unwrap()
    }
}

// Asserts that the transaction reveals neither the account nor the shielded identity of the owner.
fn assert_hides_owner(transaction: &Transaction<CurrentNetwork>, owner: &Owner) {
    let transaction = transaction.to_string();
    assert!(!transaction.contains(&owner.address().to_string()), "the transaction reveals the owner account");
    assert!(!transaction.contains(&owner.identity().to_string()), "the transaction reveals the owner identity");
}

#[test]
fn test_setup() {
    let rng = &mut TestRng::default();
    let env = TestEnv::new(rng);

    // The wallet stores only a commitment to the owner set and the threshold.
    assert_eq!(env.wallet(), env.expected_wallet(&env.owner_set, TestEnv::THRESHOLD, 0));

    // A wallet ID can be used once.
    let identities = [env.owners[0].identity()];
    let transaction = env.create_wallet(env.wallet_id, &owner_set(&identities), 1, rng).unwrap();
    env.assert_rejected(transaction, rng);
    assert_eq!(env.wallet(), env.expected_wallet(&env.owner_set, TestEnv::THRESHOLD, 0));

    // The threshold must be between 1 and the number of owners.
    let other_wallet_id = Uniform::rand(rng);
    assert!(env.create_wallet(other_wallet_id, &env.owner_set, 0, rng).is_err());
    assert!(env.create_wallet(other_wallet_id, &env.owner_set, 4, rng).is_err());
    assert!(env.create_wallet(other_wallet_id, &owner_set(&[]), 1, rng).is_err());

    // The owner identities must be distinct.
    let identities = [env.owners[0].identity(), env.owners[1].identity(), env.owners[0].identity()];
    assert!(env.create_wallet(other_wallet_id, &owner_set(&identities), 2, rng).is_err());
}

#[test]
fn test_propose() {
    let rng = &mut TestRng::default();
    let env = TestEnv::new(rng);
    let proposer = &env.owners[0];

    // An owner proposes a call. The proposal counts as the proposer's approval.
    let transaction = env.propose_call(proposer, &env.owner_set, 7, 0, rng).unwrap();
    assert_hides_owner(&transaction, proposer);
    env.assert_accepted(transaction, rng);
    let proposal_id = call_proposal_id(env.wallet_id, set_value_operation(7), 0);
    assert_eq!(env.proposal(proposal_id), Some(env.expected_proposal(0, 1, false)));

    // The same action and nonce cannot be proposed twice.
    let transaction = env.propose_call(&env.owners[1], &env.owner_set, 7, 0, rng).unwrap();
    env.assert_rejected(transaction, rng);
    assert_eq!(env.proposal(proposal_id), Some(env.expected_proposal(0, 1, false)));

    // An owner proposes to change the owners.
    let new_owner_set = owner_set(&[env.owners[1].identity(), env.owners[2].identity()]);
    let transaction = env.propose_owners(proposer, &env.owner_set, &new_owner_set, 2, 0, rng).unwrap();
    assert_hides_owner(&transaction, proposer);
    env.assert_accepted(transaction, rng);
    let proposal_id = owners_proposal_id(env.wallet_id, &new_owner_set, 2, 0);
    assert_eq!(env.proposal(proposal_id), Some(env.expected_proposal(0, 1, false)));

    // The proposed owners must form a valid wallet.
    assert!(env.propose_owners(proposer, &env.owner_set, &new_owner_set, 3, 1, rng).is_err());

    // A non-owner cannot prove membership of the owner set.
    let outsider = Owner::new(rng);
    assert!(env.propose_call(&outsider, &env.owner_set, 7, 1, rng).is_err());

    // An owner cannot prove membership without its salt.
    assert!(env.propose_call_with_salt(proposer, Uniform::rand(rng), &env.owner_set, 7, 1, rng).is_err());

    // A non-owner can prove membership of a forged owner set, but the set does not match the wallet.
    let forged_owner_set = owner_set(&[outsider.identity()]);
    let transaction = env.propose_call(&outsider, &forged_owner_set, 7, 1, rng).unwrap();
    env.assert_rejected(transaction, rng);
    assert_eq!(env.proposal(call_proposal_id(env.wallet_id, set_value_operation(7), 1)), None);
}

#[test]
fn test_submit_approval() {
    let rng = &mut TestRng::default();
    let env = TestEnv::new(rng);
    let (proposer, approver) = (&env.owners[0], &env.owners[1]);

    let transaction = env.propose_call(proposer, &env.owner_set, 7, 0, rng).unwrap();
    env.assert_accepted(transaction, rng);
    let proposal_id = call_proposal_id(env.wallet_id, set_value_operation(7), 0);

    // Another owner approves the proposal.
    let transaction = env.approve(approver, &env.owner_set, proposal_id, rng).unwrap();
    assert_hides_owner(&transaction, approver);
    env.assert_accepted(transaction, rng);
    assert_eq!(env.proposal(proposal_id), Some(env.expected_proposal(0, 2, false)));

    // An owner cannot approve twice, and the proposer has already approved.
    for owner in [approver, proposer] {
        let transaction = env.approve(owner, &env.owner_set, proposal_id, rng).unwrap();
        env.assert_rejected(transaction, rng);
    }
    assert_eq!(env.proposal(proposal_id), Some(env.expected_proposal(0, 2, false)));

    // A non-owner cannot approve.
    let outsider = Owner::new(rng);
    assert!(env.approve(&outsider, &env.owner_set, proposal_id, rng).is_err());

    // An owner cannot approve a proposal that does not exist.
    let unknown_proposal_id = call_proposal_id(env.wallet_id, set_value_operation(8), 0);
    let transaction = env.approve(&env.owners[2], &env.owner_set, unknown_proposal_id, rng).unwrap();
    env.assert_rejected(transaction, rng);
}

#[test]
fn test_execute_call() {
    let rng = &mut TestRng::default();
    let env = TestEnv::new(rng);
    let values = |env: &TestEnv| env.mapping_value(TARGET, "values", env.wallet_id);

    let transaction = env.propose_call(&env.owners[0], &env.owner_set, 42, 0, rng).unwrap();
    env.assert_accepted(transaction, rng);
    let proposal_id = call_proposal_id(env.wallet_id, set_value_operation(42), 0);

    // One approval is below the threshold.
    env.assert_rejected(env.set_value(42, 0, rng).unwrap(), rng);
    assert_eq!(values(&env), None);

    let transaction = env.approve(&env.owners[1], &env.owner_set, proposal_id, rng).unwrap();
    env.assert_accepted(transaction, rng);

    // The approval covers only the proposed operation.
    env.assert_rejected(env.set_value(43, 0, rng).unwrap(), rng);
    assert_eq!(values(&env), None);

    // The approval covers only the proposed caller.
    let inputs = [env.wallet_id.to_string(), set_value_operation(42).to_string(), "0u64".to_string()];
    let transaction = env.execute(&env.relayer, MULTISIG, "execute_call", &inputs, rng).unwrap();
    env.assert_rejected(transaction, rng);

    // With enough approvals, the target program consumes the approval.
    env.assert_accepted(env.set_value(42, 0, rng).unwrap(), rng);
    assert_eq!(values(&env), Some(Value::from_str("42u64").unwrap()));
    assert_eq!(env.proposal(proposal_id), Some(env.expected_proposal(0, 2, true)));

    // An approval can be consumed once.
    env.assert_rejected(env.set_value(42, 0, rng).unwrap(), rng);
}

#[test]
fn test_execute_owners() {
    let rng = &mut TestRng::default();
    let env = TestEnv::new(rng);
    let (removed, new) = (&env.owners[0], Owner::new(rng));

    // A call proposal is pending while the owners change.
    let transaction = env.propose_call(removed, &env.owner_set, 5, 0, rng).unwrap();
    env.assert_accepted(transaction, rng);
    let pending_proposal_id = call_proposal_id(env.wallet_id, set_value_operation(5), 0);

    // The owners propose to replace the first owner, and to require all three approvals.
    let new_owner_set = owner_set(&[env.owners[1].identity(), env.owners[2].identity(), new.identity()]);
    let transaction = env.propose_owners(removed, &env.owner_set, &new_owner_set, 3, 0, rng).unwrap();
    env.assert_accepted(transaction, rng);
    let proposal_id = owners_proposal_id(env.wallet_id, &new_owner_set, 3, 0);

    // One approval is below the threshold.
    env.assert_rejected(env.execute_owners(&new_owner_set, 3, 0, rng).unwrap(), rng);

    let transaction = env.approve(&env.owners[2], &env.owner_set, proposal_id, rng).unwrap();
    env.assert_accepted(transaction, rng);

    // The execution must match the proposed owners and threshold.
    env.assert_rejected(env.execute_owners(&new_owner_set, 2, 0, rng).unwrap(), rng);

    env.assert_accepted(env.execute_owners(&new_owner_set, 3, 0, rng).unwrap(), rng);
    assert_eq!(env.wallet(), env.expected_wallet(&new_owner_set, 3, 1));
    assert_eq!(env.proposal(proposal_id), Some(env.expected_proposal(0, 2, true)));

    // Proposals from the previous owner set can no longer be approved or executed.
    let transaction = env.approve(&env.owners[1], &new_owner_set, pending_proposal_id, rng).unwrap();
    env.assert_rejected(transaction, rng);
    env.assert_rejected(env.set_value(5, 0, rng).unwrap(), rng);

    // The removed owner is no longer a member.
    env.assert_rejected(env.propose_call(removed, &env.owner_set, 5, 1, rng).unwrap(), rng);
    assert!(env.propose_call(removed, &new_owner_set, 5, 1, rng).is_err());

    // The new owner can propose.
    env.assert_accepted(env.propose_call(&new, &new_owner_set, 5, 1, rng).unwrap(), rng);
    let proposal_id = call_proposal_id(env.wallet_id, set_value_operation(5), 1);
    assert_eq!(env.proposal(proposal_id), Some(env.expected_proposal(1, 1, false)));
}
