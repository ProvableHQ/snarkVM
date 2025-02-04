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

use super::*;

impl<N: Network> Stack<N> {
    /// Updates an existing stack, given the process and program.
    #[inline]
    pub(crate) fn check_update(process: &Process<N>, program: &Program<N>) -> Result<()> {
        // Get the existing stack.
        let stack = process.get_stack(program.id())?;
        // Get the old program.
        let old_program = stack.program();
        // Ensure the program ID matches.
        ensure!(old_program.id() == program.id(), "Cannot update program with different program ID");

        // Ensure that all of the structs in the old program exist in the new program.
        for (struct_id, struct_type) in old_program.structs() {
            let new_struct_type = program.get_struct(struct_id)?;
            ensure!(
                struct_type == new_struct_type,
                "Cannot update program because the struct '{struct_id}' has different types"
            );
        }
        // Ensure that all of the records in the old program exist in the new program.
        for (record_id, record_type) in old_program.records() {
            let new_record_type = program.get_record(record_id)?;
            ensure!(
                record_type == new_record_type,
                "Cannot update program because the record '{record_id}' has different types"
            );
        }
        // Ensure that all of the mappings in the old program exist in the new program.
        for (mapping_id, mapping_type) in old_program.mappings() {
            let new_mapping_type = program.get_mapping(mapping_id)?;
            ensure!(
                *mapping_type == new_mapping_type,
                "Cannot update program because the mapping '{mapping_id}' has different types"
            );
        }
        // Ensure that all of the imports in the old program exist in the new program.
        for import in old_program.imports().keys() {
            if !program.contains_import(import) {
                bail!("Cannot update program because it is missing the import '{import}'");
            }
        }
        // Ensure that the old program closures exist in the new program, with the same input and output types.
        for closure in old_program.closures().values() {
            if !program.contains_closure(closure.name()) {
                bail!("Cannot update program because it is missing the closure '{closure}'");
            }
            let new_closure = program.get_closure(closure.name())?;
            ensure!(
                closure.inputs() == new_closure.inputs(),
                "Cannot update program because the closure '{closure}' has different input types"
            );
            ensure!(
                closure.outputs() == new_closure.outputs(),
                "Cannot update program because the closure '{closure}' has different output types"
            );
        }
        // Ensure that the old program functions exist in the new program, with the same input and output types.
        // If the function has an associated `finalize` block, then ensure that the finalize block exists in the new program.
        for function in old_program.functions().values() {
            if !program.contains_function(function.name()) {
                bail!("Cannot update program because it is missing the function '{function}'");
            }
            let new_function = program.get_function(function.name())?;
            ensure!(
                function.inputs() == new_function.inputs(),
                "Cannot update program because the function '{function}' has different input types"
            );
            ensure!(
                function.outputs() == new_function.outputs(),
                "Cannot update program because the function '{function}' has different output types"
            );
            if let Some(finalize) = function.finalize_logic() {
                match new_function.finalize_logic() {
                    Some(new_finalize) => {
                        ensure!(
                            finalize.inputs() == new_finalize.inputs(),
                            "Cannot update program because the finalize block '{finalize}' has different input types"
                        );
                    }
                    None => {
                        bail!("Cannot update program because the function '{function}' is missing a finalize block")
                    }
                }
            }
        }

        Ok(())
    }
}
