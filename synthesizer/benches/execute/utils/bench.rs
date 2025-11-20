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

use std::str::FromStr;

use aleo_std::StorageMode;
use rand::rngs::ThreadRng;
use snarkvm_circuit::AleoV0;
use snarkvm_console::{
    account::{Address, PrivateKey, ViewKey},
    network::MainnetV0,
    program::{Ciphertext, Identifier, Value},
    types::Field,
};
use snarkvm_ledger_block::Transition;
use snarkvm_ledger_query::Query;
use snarkvm_ledger_store::{ConsensusStore, helpers::memory::BlockMemory};
use snarkvm_synthesizer::{Process, Program, VM};
use snarkvm_synthesizer_process::Authorization;
use snarkvm_utilities::TestRng;

use super::sample::{LedgerType, sample_genesis_block, sample_genesis_private_key};

pub type CurrentAleo = AleoV0;
pub type CurrentNetwork = MainnetV0;

/// Type alias for the complex HashMap used in record-based operations
pub type RecordMap = std::collections::HashMap<
    Field<CurrentNetwork>,
    snarkvm_console::program::Record<CurrentNetwork, Ciphertext<CurrentNetwork>>,
>;

/// Type alias for the return type of setup_record_context function
pub type RecordContextSetup = (TestRng, PrivateKey<CurrentNetwork>, ViewKey<CurrentNetwork>, RecordMap);

/// Common struct for execution inputs with generic RNG
pub struct ExecInputs<R> {
    pub authorization: Authorization<CurrentNetwork>,
    pub fee_authorization: Authorization<CurrentNetwork>,
    pub query: Query<CurrentNetwork, BlockMemory<CurrentNetwork>>,
    pub rng: R,
}

/// Type alias for private transfers (using TestRng)
pub type PrivateExecInputs = ExecInputs<TestRng>;

/// Type alias for public transfers (using ThreadRng)
pub type PublicExecInputs = ExecInputs<ThreadRng>;

/// Build a VM with production storage (for public operations)
pub fn build_vm() -> VM<CurrentNetwork, LedgerType> {
    let store = ConsensusStore::open(StorageMode::Production).unwrap();
    VM::from(store).unwrap()
}

/// Initialize process and program (shared across all setup functions)
fn init_process_and_program() -> (Process<CurrentNetwork>, Program<CurrentNetwork>) {
    let process = Process::load().unwrap();
    let program = Program::<CurrentNetwork>::credits().unwrap();
    (process, program)
}

/// Setup common state for private transfer functions
pub fn setup_private_transfer_state(
    vm: &VM<CurrentNetwork, LedgerType>,
    function_name: &str,
    base_fee: u64,
    priority_fee: u64,
) -> PrivateExecInputs {
    let (mut rng, caller_private_key, caller_view_key, records) = setup_record_context(vm);

    let recipient_private_key = PrivateKey::<CurrentNetwork>::new(&mut rng).unwrap();
    let recipient_address = Address::try_from(&recipient_private_key).unwrap();

    // Pick an unspent record to use as an input.
    let record = records.values().next().unwrap().decrypt(&caller_view_key).unwrap();

    let (process, program) = init_process_and_program();

    // Set the inputs for the execution.
    let r0 = Value::<CurrentNetwork>::Record(record.clone());
    let r1 = Value::<CurrentNetwork>::from_str(&recipient_address.to_string()).unwrap();
    let r2 = Value::<CurrentNetwork>::from_str("1u64").unwrap();

    // Compute the execution authorization.
    let authorization = process
        .authorize::<CurrentAleo, _>(
            &caller_private_key,
            program.id(),
            Identifier::from_str(function_name).unwrap(),
            [r0.clone(), r1.clone(), r2.clone()].iter(),
            &mut rng,
        )
        .unwrap();

    // Compute the fee authorization.
    let execution_id = authorization.to_execution_id().unwrap();
    let fee_authorization = process
        .authorize_fee_private::<CurrentAleo, _>(
            &caller_private_key,
            record,
            base_fee,
            priority_fee,
            execution_id,
            &mut rng,
        )
        .unwrap();

    let query = Query::from(vm.block_store());

    PrivateExecInputs { authorization, fee_authorization, query, rng }
}

/// Setup common state for transfer_public function
pub fn setup_transfer_public_state(
    vm: &VM<CurrentNetwork, LedgerType>,
    base_fee: u64,
    priority_fee: u64,
) -> PublicExecInputs {
    let mut rng = rand::thread_rng();

    // Set up state.
    let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng).unwrap();
    let caller = Address::try_from(&private_key).unwrap();
    let (process, program) = init_process_and_program();
    let r0 = Value::<CurrentNetwork>::from_str(&format!("{caller}")).unwrap();
    let r1 = Value::<CurrentNetwork>::from_str("1u64").unwrap();

    // Compute the execution authorization.
    let authorization = process
        .authorize::<CurrentAleo, _>(
            &private_key,
            program.id(),
            Identifier::from_str("transfer_public").unwrap(),
            [r0.clone(), r1.clone()].iter(),
            &mut rng,
        )
        .unwrap();

    // Compute the fee authorization.
    let execution_id = authorization.to_execution_id().unwrap();
    let fee_authorization = process
        .authorize_fee_public::<CurrentAleo, _>(&private_key, base_fee, priority_fee, execution_id, &mut rng)
        .unwrap();

    let query = Query::from(vm.block_store());

    PublicExecInputs { authorization, fee_authorization, query, rng }
}

/// Common setup for record-based credits functions (returns setup components)
fn setup_record_context(_vm: &VM<CurrentNetwork, LedgerType>) -> RecordContextSetup {
    let mut rng = TestRng::default();

    // Set up state.
    let caller_private_key = sample_genesis_private_key(&mut rng);
    let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();

    // Fetch the unspent records.
    let genesis = sample_genesis_block(&mut rng);
    let records = genesis.transitions().cloned().flat_map(Transition::into_records).collect::<RecordMap>();

    (rng, caller_private_key, caller_view_key, records)
}
