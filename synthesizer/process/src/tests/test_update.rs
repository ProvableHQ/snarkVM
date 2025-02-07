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

use crate::Process;
use console::{
    network::{MainnetV0, prelude::*},
    program::{Identifier, RecordType, StructType},
};
use synthesizer_program::{Closure, Function, Import, Mapping, Program, StackProgram};

type CurrentNetwork = MainnetV0;

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
fn test_add_import() -> Result<()> {
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

#[test]
fn test_add_struct() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Get the default program.
    let mut new_program = default_program();
    // Modify the program to add a new struct.
    new_program.add_struct(StructType::from_str("struct foo:data as u8;")?)?;
    // Add the new program to the process.
    process.add_program(&new_program)?;
    // Check that the updated program is edition 1.
    assert_eq!(process.get_stack("basic.aleo")?.edition(), 1);
    // Check that the update was successful.
    let stack = process.get_stack("basic.aleo")?;
    assert_eq!(stack.program().structs().len(), 2);
    Ok(())
}

#[test]
fn test_add_record() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Get the default program.
    let mut new_program = default_program();
    // Modify the program to add a new record.
    new_program.add_record(RecordType::from_str("record foo:owner as address.private;data as u8.private;")?)?;
    // Add the new program to the process.
    process.add_program(&new_program)?;
    // Check that the updated program is edition 1.
    assert_eq!(process.get_stack("basic.aleo")?.edition(), 1);
    // Check that the update was successful.
    let stack = process.get_stack("basic.aleo")?;
    assert_eq!(stack.program().records().len(), 2);
    Ok(())
}

#[test]
fn test_add_mapping() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Get the default program.
    let mut new_program = default_program();
    // Modify the program to add a new mapping.
    new_program.add_mapping(Mapping::from_str("mapping foo:key as u8.public;value as u8.public;")?)?;
    // Add the new program to the process.
    process.add_program(&new_program)?;
    // Check that the updated program is edition 1.
    assert_eq!(process.get_stack("basic.aleo")?.edition(), 1);
    // Check that the update was successful.
    let stack = process.get_stack("basic.aleo")?;
    assert_eq!(stack.program().mappings().len(), 2);
    Ok(())
}

#[test]
fn test_add_closure() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Get the default program.
    let mut new_program = default_program();
    // Modify the program to add a new closure.
    new_program.add_closure(Closure::from_str(
        "closure foo:input r0 as u8;input r1 as u8;add r0 r1 into r2;output r2 as u8;",
    )?)?;
    // Add the new program to the process.
    process.add_program(&new_program)?;
    // Check that the updated program is edition 1.
    assert_eq!(process.get_stack("basic.aleo")?.edition(), 1);
    // Check that the update was successful.
    let stack = process.get_stack("basic.aleo")?;
    assert_eq!(stack.program().closures().len(), 2);
    Ok(())
}

#[test]
fn test_add_function() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Get the default program.
    let mut new_program = default_program();
    // Modify the program to add a new function.
    new_program.add_function(Function::from_str(
        "function foo:input r0 as u8.private;input r1 as u8.private;add r0 r1 into r2;output r2 as u8.private;",
    )?)?;
    // Add the new program to the process.
    process.add_program(&new_program)?;
    // Check that the updated program is edition 1.
    assert_eq!(process.get_stack("basic.aleo")?.edition(), 1);
    // Check that the update was successful.
    let stack = process.get_stack("basic.aleo")?;
    assert_eq!(stack.program().functions().len(), 4);
    Ok(())
}

#[test]
fn test_modify_function() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Get the default program.
    let mut new_program = default_program();
    // Remove the `adder` function and add a new `adder` function.
    new_program.remove_function(&Identifier::from_str("adder")?)?;
    let new_function = Function::from_str(
        "function adder:input r0 as u8.private;input r1 as u8.private;sub r0 r1 into r2;output r2 as u8.private;",
    )?;
    new_program.add_function(new_function.clone())?;
    // Add the new program to the process.
    process.add_program(&new_program)?;
    // Check that the updated program is edition 1.
    assert_eq!(process.get_stack("basic.aleo")?.edition(), 1);
    // Check that the update was successful.
    let stack = process.get_stack("basic.aleo")?;
    assert_eq!(stack.program().functions().len(), 3);
    let updated_function = stack.program().get_function(&new_function.name())?;
    assert_eq!(updated_function, new_function);
    Ok(())
}

#[test]
fn test_modify_finalize() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Get the default program.
    let mut new_program = default_program();
    // Remove the `store_data` function and add a new `store_data` function.
    new_program.remove_function(&Identifier::from_str("store_data")?)?;
    let new_function = Function::from_str(
        r"
function store_data:
    input r0 as u8.public;
    input r1 as u8.public;
    async store_data r0 r1 into r2;
    output r2 as basic.aleo/store_data.future;

finalize store_data:
    input r0 as u8.public;
    input r1 as u8.public;
    assert.eq r0 r1;",
    )?;
    new_program.add_function(new_function.clone())?;
    // Add the new program to the process.
    process.add_program(&new_program)?;
    // Check that the updated program is edition 1.
    assert_eq!(process.get_stack("basic.aleo")?.edition(), 1);
    // Check that the update was successful.
    let stack = process.get_stack("basic.aleo")?;
    assert_eq!(stack.program().functions().len(), 3);
    let updated_function = stack.program().get_function(&new_function.name())?;
    assert_eq!(updated_function, new_function);
    Ok(())
}

#[test]
fn test_add_call_to_non_async_transition() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add a program with a non-async transition.
    let new_program = Program::from_str(
        r"
program non_async.aleo;

function foo:
    input r0 as u8.private;
    input r1 as u8.private;
    add r0 r1 into r2;
    output r2 as u8.private;",
    )?;
    process.add_program(&new_program)?;
    // Get the default program.
    let mut new_program = default_program();
    // Add an import of `non_async.aleo` to the default program.
    new_program.add_import(Import::from_str("import non_async.aleo;")?)?;
    // Remove the `adder` function and add a new `adder` function.
    new_program.remove_function(&Identifier::from_str("adder")?)?;
    let new_function = Function::from_str(
        "function adder:input r0 as u8.private;input r1 as u8.private;call non_async.aleo/foo r0 r1 into r2;output r2 as u8.private;",
    )?;
    new_program.add_function(new_function.clone())?;
    // Add the new program to the process.
    process.add_program(&new_program)?;
    // Check that the updated program is edition 1.
    assert_eq!(process.get_stack("basic.aleo")?.edition(), 1);
    // Check that the update was successful.
    let stack = process.get_stack("basic.aleo")?;
    assert_eq!(stack.program().functions().len(), 3);
    let updated_function = stack.program().get_function(&new_function.name())?;
    assert_eq!(updated_function, new_function);
    Ok(())
}
