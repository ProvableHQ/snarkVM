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

use crate::{
    CallStack,
    Process,
    Stack,
    Trace,
    traits::{StackEvaluate, StackExecute},
};
use circuit::{Aleo, network::AleoV0};
use console::{
    account::{Address, PrivateKey, ViewKey},
    network::{MainnetV0, prelude::*},
    program::{Identifier, Literal, Plaintext, ProgramID, Record, Value},
    types::{Field, U64},
};
use ledger_block::{Fee, Transaction};
use ledger_query::Query;
use ledger_store::{
    BlockStorage,
    BlockStore,
    FinalizeStorage,
    FinalizeStore,
    helpers::memory::{BlockMemory, FinalizeMemory},
};
use synthesizer_program::{FinalizeGlobalState, FinalizeStoreTrait, Import, Program, StackProgram};
use synthesizer_snark::UniversalSRS;

use indexmap::IndexMap;
use parking_lot::RwLock;
use std::sync::Arc;

type CurrentNetwork = MainnetV0;
type CurrentAleo = AleoV0;

/// Samples the default program to test updates on.
fn default_program() -> Program<CurrentNetwork> {
    Program::from_str(
        "
    import credits.aleo;

    program basic.aleo;

    struct bundle:
        first as u8;
        second as u8;

    record data:
        owner as address.private;
        data as bundle.private;

    mapping onchain:
        key as u8.public;
        value as u8.public;

    closure sum:
        input r0 as u8;
        input r1 as u8;
        add r0 r1 into r2;
        output r2 as u8;

    function adder:
        input r0 as u8.private;
        input r1 as u8.private;
        call sum r0 r1 into r2;
        output r2 as u8.private;

    function create_data:
        input r0 as u8.private;
        input r1 as u8.private;
        cast r0 r1 into r2 as bundle;
        cast self.caller r2 into r3 as data.record;
        output r3 as data.record;

    function store_data:
        input r0 as u8.public;
        input r1 as u8.public;
        async store_data r0 r1 into r2;
        output r2 as basic.aleo/store_data.future;

    finalize store_data:
        input r0 as u8.public;
        input r1 as u8.public;
        set r1 into onchain[r0];
    ",
    )
    .unwrap()
}

/// Samples a `Process` with a default program to test updates on.
fn sample_process() -> Result<Process<CurrentNetwork>> {
    // Sample the process.
    let mut process = Process::load()?;
    // Add the default program to the process.
    process.add_program(&default_program())?;
    // Check that the edition of program is 0.
    assert_eq!(process.get_stack("basic.aleo")?.edition(), 0);
    Ok(process)
}

#[test]
fn test_update_with_additional_import() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add a dummy program to the process.
    let dummy_program = Program::from_str("program dummy.aleo;function foo:")?;
    process.add_program(&dummy_program)?;
    // Get the default program.
    let mut new_program = default_program();
    // Modify the program to add a new import.
    new_program.add_import(Import::from_str("import dummy.aleo;")?)?;
    // Add the new program to the process.
    process.add_program(&new_program)?;
    // Check that the updated program is edition 1.
    assert_eq!(process.get_stack("basic.aleo")?.edition(), 1);
    // Check that the update was successful.
    let stack = process.get_stack("basic.aleo")?;
    assert_eq!(stack.program().imports().len(), 2);
    Ok(())
}
