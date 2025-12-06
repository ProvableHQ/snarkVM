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

use console::{network::Network, program::ValueType};
use snarkvm_synthesizer_program::Program;

use anyhow::{Result, bail, ensure};

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
