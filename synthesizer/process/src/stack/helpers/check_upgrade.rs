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

use super::*;

impl<N: Network> Stack<N> {
    /// Checks that the new program is a valid upgrade.
    /// At a high-level, an upgrade must preserve the existing interfaces of the original program.
    /// An upgrade may add new components, except for constructors, and modify logic **only** in functions and finalize scopes.
    /// An upgrade may also be exactly the same as the original program.
    ///
    /// The order of the components in the new program may be modified, as long as the interfaces remain the same.
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
    /// There is one important caveat in that output register indices **MUST** remain the same.
    /// For example, changing `output r10 as <NAME>.record` into `output r11 as <NAME>.record` would not be a valid upgrade.
    /// This restriction is necessary because the output register index is instantiated as a constant in the caller circuit.
    /// This check is enforced in `check_transaction` in `synthesizer/src/vm/verify.rs`.
    #[inline]
    pub fn check_upgrade_is_valid(
        old_program: &Program<N>,
        new_program: &Program<N>,
    ) -> Result<(), ProgramUpgradeError> {
        // Get the new program ID.
        let program_id = new_program.id();
        // Ensure the program is not `credits.aleo`.
        if program_id == &ProgramID::from_str("credits.aleo").unwrap() {
            return Err(ProgramUpgradeError::CreditsUpgrade);
        }
        // Ensure the program ID matches.
        if old_program.id() != new_program.id() {
            return Err(ProgramUpgradeError::DifferentProgramId(old_program.id().to_string()));
        }
        // Ensure that all of the imports in the old program exist in the new program.
        for old_import in old_program.imports().keys() {
            if !new_program.contains_import(old_import) {
                return Err(ProgramUpgradeError::MissingOriginalImport(program_id.to_string(), old_import.to_string()));
            }
        }
        // Ensure that the constructors in both programs are exactly the same.
        // Note: Programs without constructors are not allowed to be upgraded.
        match (old_program.constructor(), new_program.constructor()) {
            (_, None) => {
                return Err(ProgramUpgradeError::ConstructorMissingOriginal);
            }
            (None, _) => {
                return Err(ProgramUpgradeError::ConstructorMissingFinal);
            }
            (Some(old_constructor), Some(new_constructor)) => {
                if old_constructor != new_constructor {
                    return Err(ProgramUpgradeError::ConstructorMismatch(old_program.id().to_string()));
                }
            }
        }
        // Ensure that all of the mappings in the old program exist in the new program.
        for (old_mapping_id, old_mapping_type) in old_program.mappings() {
            let new_mapping_type = new_program.get_mapping(old_mapping_id).map_err(|_| {
                ProgramUpgradeError::MappingMissing(old_program.id().to_string(), old_mapping_id.to_string())
            })?;
            if *old_mapping_type != new_mapping_type {
                return Err(ProgramUpgradeError::MappingMismatch(
                    old_program.id().to_string(),
                    old_mapping_id.to_string(),
                ));
            }
        }
        // Ensure that all of the structs in the old program exist in the new program.
        for (old_struct_id, old_struct_type) in old_program.structs() {
            let new_struct_type = new_program.get_struct(old_struct_id).map_err(|_| {
                ProgramUpgradeError::StructMissing(old_program.id().to_string(), old_struct_id.to_string())
            })?;
            if old_struct_type != new_struct_type {
                return Err(ProgramUpgradeError::StructMismatch(
                    old_program.id().to_string(),
                    old_struct_id.to_string(),
                ));
            }
        }
        // Ensure that all of the records in the old program exist in the new program.
        for (old_record_id, old_record_type) in old_program.records() {
            let new_record_type = new_program.get_record(old_record_id).map_err(|_| {
                ProgramUpgradeError::RecordMissing(old_program.id().to_string(), old_record_id.to_string())
            })?;
            if old_record_type != new_record_type {
                return Err(ProgramUpgradeError::RecordMismatch(
                    old_program.id().to_string(),
                    old_record_id.to_string(),
                ));
            }
        }
        // Ensure that the old program closures exist in the new program, with the exact same definition.
        for old_closure in old_program.closures().values() {
            let old_closure_name = old_closure.name();
            let new_closure = new_program.get_closure(old_closure_name).map_err(|_| {
                ProgramUpgradeError::ClosureMissing(old_program.id().to_string(), old_closure_name.to_string())
            })?;
            if old_closure != &new_closure {
                return Err(ProgramUpgradeError::ClosureMismatch(
                    old_program.id().to_string(),
                    old_closure_name.to_string(),
                ));
            }
        }
        // Ensure that the old program functions exist in the new program, with the same input and output types.
        // If the function has an associated `finalize` block, then ensure that the finalize block exists in the new program.
        for old_function in old_program.functions().values() {
            let old_function_name = old_function.name();
            let new_function = new_program.get_function_ref(old_function_name).map_err(|_| {
                ProgramUpgradeError::FunctionMissing(old_program.id().to_string(), old_function_name.to_string())
            })?;
            if old_function.input_types() != new_function.input_types() {
                return Err(ProgramUpgradeError::FunctionInputMismatch(
                    old_program.id().to_string(),
                    old_function_name.to_string(),
                ));
            }
            if old_function.output_types() != new_function.output_types() {
                return Err(ProgramUpgradeError::FunctionOutputMismatch(
                    old_program.id().to_string(),
                    old_function_name.to_string(),
                ));
            }
            match (old_function.finalize_logic(), new_function.finalize_logic()) {
                (None, None) => {} // Do nothing
                (None, Some(_)) => {
                    return Err(ProgramUpgradeError::FunctionFinalizeBlockUnexpected(
                        old_program.id().to_string(),
                        old_function_name.to_string(),
                    ));
                }
                (Some(_), None) => {
                    return Err(ProgramUpgradeError::FunctionFinalizeBlockExpected(
                        old_program.id().to_string(),
                        old_function_name.to_string(),
                    ));
                }
                (Some(old_finalize), Some(new_finalize)) => {
                    if old_finalize.input_types() != new_finalize.input_types() {
                        return Err(ProgramUpgradeError::FunctionFinalizeInputMismatch(
                            old_program.id().to_string(),
                            old_function_name.to_string(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
