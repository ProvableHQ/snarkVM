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

use crate::Stack;
use console::{
    prelude::{Network, cfg_iter},
    program::{ArrayType, FinalizeType, Identifier, Locator, PlaintextType, RegisterType, ValueType},
};
use snarkvm_synthesizer_program::{CastType, Command, Instruction, Operand, Program, StackTrait, types_equivalent};

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

/// Returns `true` if the program contains a `cast` to an array that relies on array-flattening, i.e.
/// a cast that would be rejected under the pre-V16 strict rule (exactly `length` operands, each
/// equal to the element type). Array-flattening is enabled in `ConsensusVersion::V16`.
///
/// Note: this requires type inference, so it takes the program's `Stack` (which has already been
/// type-checked). Programs are accepted permissively at type-check time; this check is what gates
/// the relaxed behavior to V16 at deployment.
pub fn program_uses_array_flatten<N: Network>(program: &Program<N>, stack: &Stack<N>) -> Result<bool> {
    // If `instruction` is a `cast` to an array, returns its operands and the target array type.
    fn cast_to_array<N: Network>(instruction: &Instruction<N>) -> Option<(&[Operand<N>], &ArrayType<N>)> {
        match instruction {
            Instruction::Cast(cast) => match cast.cast_type() {
                CastType::Plaintext(PlaintextType::Array(array_type)) => Some((cast.operands(), array_type)),
                _ => None,
            },
            _ => None,
        }
    }

    // Returns `true` if the cast relies on flattening, given each operand's resolved plaintext type
    // (`None` for non-plaintext operands, which are only valid under the flattening rule).
    let cast_flattens = |operand_types: &[Option<PlaintextType<N>>], array_type: &ArrayType<N>| -> Result<bool> {
        let element_type = array_type.next_element_type();
        // The strict rule requires exactly `length` operands.
        if operand_types.len() != **array_type.length() as usize {
            return Ok(true);
        }
        // The strict rule requires every operand to equal the element type.
        for operand_type in operand_types {
            match operand_type {
                Some(plaintext_type) if types_equivalent(stack, plaintext_type, stack, element_type)? => {}
                _ => return Ok(true),
            }
        }
        Ok(false)
    };

    // Check the closures and function bodies, which use register types.
    for (name, instructions) in program
        .closures()
        .iter()
        .map(|(name, closure)| (name, closure.instructions()))
        .chain(program.functions().iter().map(|(name, function)| (name, function.instructions())))
    {
        let register_types = stack.get_register_types(name)?;
        for instruction in instructions {
            if let Some((operands, array_type)) = cast_to_array(instruction) {
                let operand_types = operands
                    .iter()
                    .map(|operand| match register_types.get_type_from_operand(stack, operand)? {
                        RegisterType::Plaintext(plaintext_type) => Ok(Some(plaintext_type)),
                        _ => Ok(None),
                    })
                    .collect::<Result<Vec<_>>>()?;
                if cast_flattens(&operand_types, array_type)? {
                    return Ok(true);
                }
            }
        }
    }

    // Check a sequence of commands that share a finalize-type context (finalize blocks, the
    // constructor, and views). Returns `true` if any contained cast-to-array relies on flattening.
    let check_finalize_commands =
        |finalize_types: &snarkvm_synthesizer_process::FinalizeTypes<N>, commands: &[Command<N>]| -> Result<bool> {
            for command in commands {
                if let Command::Instruction(instruction) = command
                    && let Some((operands, array_type)) = cast_to_array(instruction)
                {
                    let operand_types = operands
                        .iter()
                        .map(|operand| match finalize_types.get_type_from_operand(stack, operand)? {
                            FinalizeType::Plaintext(plaintext_type) => Ok(Some(plaintext_type)),
                            _ => Ok(None),
                        })
                        .collect::<Result<Vec<_>>>()?;
                    if cast_flattens(&operand_types, array_type)? {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        };

    // Check the function finalize blocks.
    for (name, function) in program.functions() {
        if let Some(finalize) = function.finalize_logic()
            && check_finalize_commands(&stack.get_finalize_types(name)?, finalize.commands())?
        {
            return Ok(true);
        }
    }
    // Check the constructor.
    if let Some(constructor) = program.constructor()
        && check_finalize_commands(&stack.get_constructor_types()?, constructor.commands())?
    {
        return Ok(true);
    }
    // Check the views.
    for (name, view) in program.views() {
        if check_finalize_commands(&stack.get_view_types(name)?, view.commands())? {
            return Ok(true);
        }
    }

    Ok(false)
}
