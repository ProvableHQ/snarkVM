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
    program::{Identifier, Locator, ValueType},
};
use snarkvm_synthesizer_program::{Program, StackTrait};

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
