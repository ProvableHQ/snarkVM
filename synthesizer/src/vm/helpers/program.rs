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

use crate::{Stack, process::FinalizeTypes};
use console::{
    prelude::{Network, cfg_iter},
    program::{FinalizeType, Identifier, LiteralType, Locator, PlaintextType, RegisterType, ValueType},
};
use snarkvm_synthesizer_program::{Command, Instruction, Program, StackTrait};

use anyhow::{Result, anyhow, bail, ensure};

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

/// Verifies that the existing output register indices are not changed in a new version of the program.
// Note. This function is public so that depednent crates can cleanly surface this error to users.
pub fn check_output_register_indices_unchanged<N: Network>(
    old_program: &Program<N>,
    new_program: &Program<N>,
) -> Result<()> {
    for (id, function) in old_program.functions() {
        // Get the corresponding function in the new program.
        let Ok(new_function) = new_program.get_function(id) else { bail!("Missing function '{id}'") };
        // Ensure the record output registers match.
        let existing_output_registers =
            function.outputs().iter().filter(|output| matches!(output.value_type(), ValueType::Record(_)));
        let new_output_registers =
            new_function.outputs().iter().filter(|output| matches!(output.value_type(), ValueType::Record(_)));
        ensure!(
            existing_output_registers.eq(new_output_registers),
            "Function '{id}' has mismatched record output registers"
        );
    }
    Ok(())
}

// TODO (raychu86): Unify this logic with other usages of `size_in_bits`.
/// Checks that all future argument bit sizes in the program do not exceed the specified maximum.
pub fn check_future_argument_bit_size<N: Network>(
    program: &Program<N>,
    stack: &Stack<N>,
    max_future_argument_bit_size: usize,
) -> Result<()> {
    // Helper to get a struct declaration.
    let get_struct = |id: &Identifier<N>| program.get_struct(id).cloned();

    // Helper to get an external struct declaration.
    let get_external_struct = |locator: &Locator<N>| {
        stack.get_external_stack(locator.program_id())?.program().get_struct(locator.resource()).cloned()
    };

    // A helper to get the argument types of a future.
    let get_future = |locator: &Locator<N>| {
        Ok(match stack.program_id() == locator.program_id() {
            true => stack
                .program()
                .get_function_ref(locator.resource())?
                .finalize_logic()
                .ok_or_else(|| anyhow!("'{locator}' does not have a finalize scope"))?
                .input_types(),
            false => stack
                .get_external_stack(locator.program_id())?
                .program()
                .get_function_ref(locator.resource())?
                .finalize_logic()
                .ok_or_else(|| anyhow!("Failed to find function '{locator}'"))?
                .input_types(),
        })
    };

    // Check each function's finalize inputs in parallel.
    cfg_iter!(program.functions()).try_for_each(|(_, function)| {
        // If there is no finalize logic, skip.
        let Some(finalize) = function.finalize_logic() else { return Ok(()) };

        // Check each input type.
        let input_types = finalize.input_types();
        let program_id = program.id();
        let function_name = *function.name();
        cfg_iter!(input_types).enumerate().try_for_each(|(i, input_type)| {
            // If the finalize type is a future, check the argument sizes.
            let argument_num_bits =
                input_type.size_in_bits_internal(&get_struct, &get_external_struct, &get_future, 0)?;
            ensure!(
                        argument_num_bits <= max_future_argument_bit_size,
                        "Future argument {i} in {program_id}/{function_name} exceeds the maximum allowed size in bits ({argument_num_bits} > {max_future_argument_bit_size})."
                    );
            Ok(())
        })
    })
}

/// Ensures every `ternary` instruction in the program operates on a branch operand whose type was
/// supported by `ternary` prior to `ConsensusVersion::V16`. V16 added support for the `identifier`
/// literal type plus arrays and structs; those are rejected here so a program containing them does
/// not deploy before V16 and diverge from the pre-PR network.
pub fn check_no_non_literal_ternary<N: Network>(program: &Program<N>, stack: &Stack<N>) -> Result<()> {
    // The error message used when an unsupported ternary operand type is found.
    const ERROR_MSG: &str = "ternary on this operand type is not allowed before `ConsensusVersion::V16`";

    // Returns whether the given plaintext type was a valid ternary branch operand before V16.
    let is_pre_v16_supported = |plaintext_type: &PlaintextType<N>| -> bool {
        matches!(
            plaintext_type,
            PlaintextType::Literal(
                LiteralType::Address
                    | LiteralType::Boolean
                    | LiteralType::Field
                    | LiteralType::Group
                    | LiteralType::I8
                    | LiteralType::I16
                    | LiteralType::I32
                    | LiteralType::I64
                    | LiteralType::I128
                    | LiteralType::U8
                    | LiteralType::U16
                    | LiteralType::U32
                    | LiteralType::U64
                    | LiteralType::U128
                    | LiteralType::Scalar
                    | LiteralType::Signature
            )
        )
    };

    // Checks the ternary instructions within the given function or closure instruction sequence.
    let check_instructions = |name: &Identifier<N>, instructions: &[Instruction<N>]| -> Result<()> {
        let register_types = stack.get_register_types(name)?;
        for instruction in instructions {
            if let Instruction::Ternary(ternary) = instruction {
                // The ternary operands are `[condition, first, second]`.
                for operand in &ternary.operands()[1..3] {
                    let reg_type = register_types.get_type_from_operand(stack, operand)?;
                    match reg_type {
                        RegisterType::Plaintext(plaintext_type) => {
                            ensure!(is_pre_v16_supported(&plaintext_type), "{ERROR_MSG}");
                        }
                        _ => bail!("{ERROR_MSG}"),
                    }
                }
            }
        }
        Ok(())
    };

    // Checks the ternary instructions within the given finalize or constructor command sequence.
    let check_commands = |finalize_types: &FinalizeTypes<N>, commands: &[Command<N>]| -> Result<()> {
        for command in commands {
            if let Command::Instruction(Instruction::Ternary(ternary)) = command {
                // The ternary operands are `[condition, first, second]`.
                for operand in &ternary.operands()[1..3] {
                    let finalize_type = finalize_types.get_type_from_operand(stack, operand)?;
                    match finalize_type {
                        FinalizeType::Plaintext(plaintext_type) => {
                            ensure!(is_pre_v16_supported(&plaintext_type), "{ERROR_MSG}");
                        }
                        _ => bail!("{ERROR_MSG}"),
                    }
                }
            }
        }
        Ok(())
    };

    // Check every function body and its finalize scope (if any).
    for (name, function) in program.functions() {
        check_instructions(name, function.instructions())?;
        if let Some(finalize) = function.finalize_logic() {
            let finalize_types = stack.get_finalize_types(name)?;
            check_commands(&finalize_types, finalize.commands())?;
        }
    }
    // Check every closure body.
    for (name, closure) in program.closures() {
        check_instructions(name, closure.instructions())?;
    }
    // Check the program's constructor, if any.
    if let Some(constructor) = program.constructor() {
        let constructor_types = stack.get_constructor_types()?;
        check_commands(&constructor_types, constructor.commands())?;
    }

    Ok(())
}
