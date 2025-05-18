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

use super::*;

impl<N: Network> Stack<N> {
    /// Checks that the new program is a valid upgrade.
    /// At a high-level, an upgrade must preserve the existing interfaces of the original program.
    /// An upgrade may add new components, except for constructors, and modify logic **only** in functions and finalize scopes.
    ///
    /// An detailed overview of what an upgrade can and cannot do is given below:
    ///  | Program Component | Delete |    Modify    |  Add  |
    ///  |-------------------|--------|--------------|-------|
    ///  | import            |   ❌   |      ❌      |  ✅   |
    ///  | constructor       |   ❌   |      ❌      |  ❌   |
    ///  | mapping           |   ❌   |      ❌      |  ✅   |
    ///  | struct            |   ❌   |      ❌      |  ✅   |
    ///  | record            |   ❌   |      ❌      |  ✅   |
    ///  | closure           |   ❌   |      ❌      |  ✅   |
    ///  | function          |   ❌   | ✅ (logic)   |  ✅   |
    ///  | finalize          |   ❌   | ✅ (logic)   |  ✅   |
    ///  |-------------------|--------|--------------|-------|
    ///
    #[inline]
    pub fn check_upgrade_is_valid(old_program: &Program<N>, new_program: &Program<N>) -> Result<()> {
        // Get the new program ID.
        let program_id = new_program.id();
        // Ensure the program is not `credits.aleo`.
        ensure!(program_id != &ProgramID::from_str("credits.aleo")?, "Cannot upgrade 'credits.aleo'");
        // Ensure the program ID matches.
        ensure!(old_program.id() == new_program.id(), "Cannot upgrade '{program_id}' with different program ID");
        // Ensure that the old program is not the same as the new program.
        ensure!(old_program != new_program, "Cannot upgrade '{program_id}' with the same program");
        // Ensure that all of the imports in the old program exist in the new program.
        for old_import in old_program.imports().keys() {
            if !new_program.contains_import(old_import) {
                bail!("Cannot upgrade '{program_id}' because it is missing the original import '{old_import}'");
            }
        }
        // Ensure that the constructors in both programs are exactly the same.
        // Note: Programs without constructors are not allowed to be upgraded.
        match (old_program.constructor(), new_program.constructor()) {
            (_, None) => bail!("A program cannot be upgraded to a program without a constructor"),
            (None, _) => bail!("A program without a constructor cannot be upgraded"),
            (Some(old_constructor), Some(new_constructor)) => {
                ensure!(
                    old_constructor == new_constructor,
                    "Cannot upgrade '{program_id}' because the constructor does not match"
                );
            }
        }
        // Ensure that all of the mappings in the old program exist in the new program.
        for (old_mapping_id, old_mapping_type) in old_program.mappings() {
            let new_mapping_type = new_program.get_mapping(old_mapping_id)?;
            ensure!(
                *old_mapping_type == new_mapping_type,
                "Cannot upgrade '{program_id}' because the mapping '{old_mapping_id}' does not match"
            );
        }
        // Ensure that all of the structs in the old program exist in the new program.
        for (old_struct_id, old_struct_type) in old_program.structs() {
            let new_struct_type = new_program.get_struct(old_struct_id)?;
            ensure!(
                old_struct_type == new_struct_type,
                "Cannot upgrade '{program_id}' because the struct '{old_struct_id}' does not match"
            );
        }
        // Ensure that all of the records in the old program exist in the new program.
        for (old_record_id, old_record_type) in old_program.records() {
            let new_record_type = new_program.get_record(old_record_id)?;
            ensure!(
                old_record_type == new_record_type,
                "Cannot upgrade '{program_id}' because the record '{old_record_id}' does not match"
            );
        }
        // Ensure that the old program closures exist in the new program, with the exact same definition.
        for old_closure in old_program.closures().values() {
            let old_closure_name = old_closure.name();
            let new_closure = new_program.get_closure(old_closure_name)?;
            ensure!(
                old_closure == &new_closure,
                "Cannot upgrade '{program_id}' because the closure '{old_closure_name}' does not match"
            );
        }
        // Ensure that the old program functions exist in the new program, with the same input and output types.
        // If the function has an associated `finalize` block, then ensure that the finalize block exists in the new program.
        for old_function in old_program.functions().values() {
            let old_function_name = old_function.name();
            let new_function = new_program.get_function_ref(old_function_name)?;
            ensure!(
                old_function.input_types() == new_function.input_types(),
                "Cannot upgrade '{program_id}' because the inputs to the function '{old_function_name}' do not match"
            );
            ensure!(
                old_function.output_types() == new_function.output_types(),
                "Cannot upgrade '{program_id}' because the outputs of the function '{old_function_name}' do not match"
            );
            match (old_function.finalize_logic(), new_function.finalize_logic()) {
                (None, None) => {} // Do nothing
                (None, Some(_)) => bail!(
                    "Cannot upgrade '{program_id}' because the function '{old_function_name}' should not have a finalize block"
                ),
                (Some(_), None) => bail!(
                    "Cannot upgrade '{program_id}' because the function '{old_function_name}' should have a finalize block"
                ),
                (Some(old_finalize), Some(new_finalize)) => {
                    ensure!(
                        old_finalize.input_types() == new_finalize.input_types(),
                        "Cannot upgrade '{program_id}' because the finalize inputs to the function '{old_function_name}' do not match"
                    );
                }
            }
        }
        Ok(())
    }
}
