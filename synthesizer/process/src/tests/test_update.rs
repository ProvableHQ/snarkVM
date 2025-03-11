// Copyright 2024-2025 Aleo Network Foundation
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

/// The purpose of these tests are to ensure that an update made to a program is correct.
/// The logic governing these updates are defined in `check_update_is_valid`.
/// These tests *DO NOT*:
///   - check the enforcement rules of the upgrades.
///   - check that the program contains the required metadata.
use crate::Process;
use console::network::{MainnetV0, prelude::*};
use synthesizer_program::{Program, ProgramVersion, StackProgram};

type CurrentNetwork = MainnetV0;

// A helper function to sample the default process.
fn sample_process() -> Result<Process<CurrentNetwork>, Error> {
    let mut process = Process::load()?;
    // Add the default V2 program to the process.
    let default_program = Program::from_str(
        r"
program$2 test.aleo;
function foo:
    ",
    )?;
    process.add_program(&default_program)?;
    // Return the process.
    Ok(process)
}

#[test]
fn test_add_simple_program() -> Result<()> {
    // Sample the default process.
    let mut process = Process::<CurrentNetwork>::load()?;
    // Add a simple V2 program to the process.
    let initial_program = Program::from_str(
        r"
program$2 test.aleo;
function foo:
    ",
    )?;
    // Assert that the program is a V2 program.
    assert_eq!(initial_program.version(), ProgramVersion::V2);
    // Add the new program to the process.
    process.add_program(&initial_program)?;
    // Get the program from the process.
    let stack = process.get_stack("test.aleo")?;
    let program = stack.program();
    // Check that the program is the same as the initial program.
    assert_eq!(program, &initial_program);
    Ok(())
}

#[test]
fn test_update_with_different_version() -> Result<()> {
    // Sample the default process.
    let mut process = Process::<CurrentNetwork>::load()?;
    // Add a simple V1 program to the process.
    let initial_program = Program::from_str(
        r"
program test.aleo;
function foo:
    ",
    )?;
    process.add_program(&initial_program)?;
    // Attempt to update the program.
    let new_program = Program::from_str(
        r"
program$2 test.aleo;
function foo:
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_update_with_same_version() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 test.aleo;
function foo:
    ",
    )?;
    // Verify that the update was successful.
    process.add_program(&new_program)?;
    Ok(())
}

#[test]
fn test_add_import() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Update the program.
    let new_program = Program::from_str(
        r"
import credits.aleo;
program$2 test.aleo;
function foo:
    ",
    )?;
    process.add_program(&new_program)?;
    // Verify that the update was successful.
    let stack = process.get_stack("test.aleo")?;
    let program = stack.program();
    assert_eq!(program, &new_program);
    Ok(())
}

#[test]
fn test_add_struct() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 test.aleo;
struct bar:
    data as u8;
function foo:
    ",
    )?;
    process.add_program(&new_program)?;
    // Verify that the update was successful.
    let stack = process.get_stack("test.aleo")?;
    let program = stack.program();
    assert_eq!(program, &new_program);
    Ok(())
}

#[test]
fn test_add_record() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 test.aleo;
record bar:
    owner as address.private;
    data as u8.private;
function foo:
    ",
    )?;
    process.add_program(&new_program)?;
    // Verify that the update was successful.
    let stack = process.get_stack("test.aleo")?;
    let program = stack.program();
    assert_eq!(program, &new_program);
    Ok(())
}

#[test]
fn test_add_mapping() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 test.aleo;
mapping onchain:
    key as u8.public;
    value as u16.public;
function foo:
    ",
    )?;
    process.add_program(&new_program)?;
    // Verify that the update was successful.
    let stack = process.get_stack("test.aleo")?;
    let program = stack.program();
    assert_eq!(program, &new_program);
    Ok(())
}

#[test]
fn test_add_closure() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 test.aleo;
closure sum:
    input r0 as u8;
    input r1 as u8;
    add r0 r1 into r2;
    output r2 as u8;
function foo:
    ",
    )?;
    process.add_program(&new_program)?;
    // Verify that the update was successful.
    let stack = process.get_stack("test.aleo")?;
    let program = stack.program();
    assert_eq!(program, &new_program);
    Ok(())
}

#[test]
fn test_add_function() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 test.aleo;
function adder:
    input r0 as u8.private;
    input r1 as u8.private;
    add r0 r1 into r2;
    output r2 as u8.private;
function foo:
    ",
    )?;
    process.add_program(&new_program)?;
    // Verify that the update was successful.
    let stack = process.get_stack("test.aleo")?;
    let program = stack.program();
    assert_eq!(program, &new_program);
    Ok(())
}

#[test]
fn test_modify_function_logic() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
function adder:
    input r0 as u8.private;
    input r1 as u8.private;
    add r0 r1 into r2;
    output r2 as u8.private;
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
function adder:
    input r0 as u8.private;
    input r1 as u8.private;
    sub r0 r1 into r2;
    output r2 as u8.private;
    ",
    )?;
    process.add_program(&new_program)?;
    // Verify that the update was successful.
    let stack = process.get_stack("basic.aleo")?;
    let program = stack.program();
    assert_eq!(program, &new_program);
    Ok(())
}

#[test]
fn test_modify_function_signature() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
function adder:
    input r0 as u8.private;
    input r1 as u8.private;
    add r0 r1 into r2;
    output r2 as u8.private;
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
function adder:
    input r0 as u16.private;
    input r1 as u16.private;
    add r0 r1 into r2;
    output r2 as u16.private;
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_modify_finalize_logic() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
function assert_on_chain:
    input r0 as u8.public;
    input r1 as u8.public;
    async assert_on_chain r0 r1 into r2;
    output r2 as basic.aleo/assert_on_chain.future;
finalize assert_on_chain:
    input r0 as u8.public;
    input r1 as u8.public;
    assert.eq r0 r1;
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
function assert_on_chain:
    input r0 as u8.public;
    input r1 as u8.public;
    async assert_on_chain r0 r1 into r2;
    output r2 as basic.aleo/assert_on_chain.future;
finalize assert_on_chain:
    input r0 as u8.public;
    input r1 as u8.public;
    assert.neq r0 r1;
    ",
    )?;
    process.add_program(&new_program)?;
    // Verify that the update was successful.
    let stack = process.get_stack("basic.aleo")?;
    let program = stack.program();
    assert_eq!(program, &new_program);
    Ok(())
}

#[test]
fn test_modify_finalize_signature() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
function assert_on_chain:
    input r0 as u8.public;
    input r1 as u8.public;
    async assert_on_chain r0 r1 into r2;
    output r2 as basic.aleo/assert_on_chain.future;
finalize assert_on_chain:
    input r0 as u8.public;
    input r1 as u8.public;
    assert.eq r0 r1;
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
function assert_on_chain:
    input r0 as u8.public;
    input r1 as u8.public;
    async assert_on_chain 0u16 1u16 into r2;
    output r2 as basic.aleo/assert_on_chain.future;
finalize assert_on_chain:
    input r0 as u16.public;
    input r1 as u16.public;
    assert.eq r0 r1;
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_modify_struct() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
struct bar:
    data as u8;
function foo:
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
struct bar:
    data as u16;
function foo:
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_modify_record() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
record bar:
    owner as address.private;
    data as u8.private;
function foo:
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
record bar:
    owner as address.private;
    data as u16.private;
function foo:
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_modify_mapping() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
mapping onchain:
    key as u8.public;
    value as u16.public;
function foo:
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
mapping onchain:
    key as u8.public;
    value as u8.public;
function foo:
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_modify_closure_logic() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
closure sum:
    input r0 as u8;
    input r1 as u8;
    add r0 r1 into r2;
    output r2 as u8;
function foo:
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
closure sum:
    input r0 as u8;
    input r1 as u8;
    sub r0 r1 into r2;
    output r2 as u8;
function foo:
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_modify_closure_signature() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
closure sum:
    input r0 as u8;
    input r1 as u8;
    add r0 r1 into r2;
    output r2 as u8;
function foo:
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
closure sum:
    input r0 as u16;
    input r1 as u16;
    add r0 r1 into r2;
    output r2 as u16;
function foo:
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_remove_import() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
import credits.aleo;
program$2 basic.aleo;
function foo:
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
function foo:
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_remove_struct() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
struct bar:
    data as u8;
function foo:
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
function foo:
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_remove_record() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
record bar:
    owner as address.private;
    data as u8.private;
function foo:
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
function foo:
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_remove_mapping() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
mapping onchain:
    key as u8.public;
    value as u16.public;
function foo:
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
function foo:
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_remove_closure() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
closure sum:
    input r0 as u8;
    input r1 as u8;
    add r0 r1 into r2;
    output r2 as u8;
function foo:
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
function foo:
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_remove_function() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
function adder:
    input r0 as u8.private;
    input r1 as u8.private;
    add r0 r1 into r2;
    output r2 as u8.private;
function foo:
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
function foo:
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
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
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
import non_async.aleo;
program$2 basic.aleo;
function adder:
    input r0 as u8.private;
    input r1 as u8.private;
    add r0 r1 into r2;
    output r2 as u8.private;
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
import non_async.aleo;
program$2 basic.aleo;
function adder:
    input r0 as u8.private;
    input r1 as u8.private;
    call non_async.aleo/foo r0 r1 into r2;
    output r2 as u8.private;
    ",
    )?;
    process.add_program(&new_program)?;
    // Verify that the update was successful.
    let stack = process.get_stack("basic.aleo")?;
    let program = stack.program();
    assert_eq!(program, &new_program);
    Ok(())
}

#[test]
fn test_add_call_to_async_transition() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add a program with an async transition.
    let new_program = Program::from_str(
        r"
program async_example.aleo;

function foo:
    input r0 as u8.private;
    input r1 as u8.private;
    async foo r0 r1 into r2;
    add r0 r1 into r3;
    output r3 as u8.private;
    output r2 as async_example.aleo/foo.future;
finalize foo:
    input r0 as u8.public;
    input r1 as u8.public;
    assert.eq r0 r1;",
    )?;
    process.add_program(&new_program)?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
import async_example.aleo;
program$2 basic.aleo;
function adder:
    input r0 as u8.private;
    input r1 as u8.private;
    add r0 r1 into r2;
    output r2 as u8.private;
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
import async_example.aleo;
program$2 basic.aleo;
function adder:
    input r0 as u8.private;
    input r1 as u8.private;
    call async_example.aleo/foo r0 r1 into r2 r3;
    async adder r3 into r4;
    output r2 as u8.private;
    output r4 as basic.aleo/adder.future;
finalize adder:
    input r0 as async_example.aleo/foo.future;
    await r0;",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}

#[test]
fn test_add_import_cycle() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;

    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
function adder:
    input r0 as u8.private;
    input r1 as u8.private;
    add r0 r1 into r2;
    output r2 as u8.private;
    ",
    )?;
    process.add_program(&initial_program)?;

    // Verify that self-import cycles are not allowed.
    let new_program = Program::from_str(
        r"
import basic.aleo;
program$2 basic.aleo;
function adder:
    input r0 as u8.private;
    input r1 as u8.private;
    add r0 r1 into r2;
    output r2 as u8.private;
    ",
    )?;
    assert!(process.add_program(&new_program).is_err());

    // Add a program dependent on `basic.aleo`.
    let dependent_program = Program::from_str(
        r"
import basic.aleo;
program dependent.aleo;
function foo:
    input r0 as u8.private;
    input r1 as u8.private;
    call basic.aleo/adder r0 r1 into r2;
    output r2 as u8.private;",
    )?;
    process.add_program(&dependent_program)?;
    // Verify that the update was successful.
    let stack = process.get_stack("dependent.aleo")?;
    let program = stack.program();
    assert_eq!(program, &dependent_program);

    // Update basic.aleo to import dependent.aleo.
    // This is allowed since we do not do cycle detection across programs.
    let new_program = Program::from_str(
        r"
import dependent.aleo;
program$2 basic.aleo;
function adder:
    input r0 as u8.private;
    input r1 as u8.private;
    add r0 r1 into r2;
    output r2 as u8.private;
    ",
    )?;
    // Verify that the update was successful.
    process.add_program(&new_program)?;
    Ok(())
}

#[test]
fn test_constructor_update() -> Result<()> {
    // Sample the default process.
    let mut process = sample_process()?;
    // Add the initial program to the process.
    let initial_program = Program::from_str(
        r"
program$2 basic.aleo;
function foo:
_init:
    assert.eq 1u8 1u8;
    ",
    )?;
    process.add_program(&initial_program)?;
    // Update the program.
    let new_program = Program::from_str(
        r"
program$2 basic.aleo;
function foo:
_init:
    assert.eq 2u8 2u8;
    ",
    )?;
    // Verify that the update was not successful.
    assert!(process.add_program(&new_program).is_err());
    Ok(())
}
