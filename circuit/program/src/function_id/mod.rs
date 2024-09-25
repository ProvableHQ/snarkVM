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

use crate::{Identifier, ProgramID};
use snarkvm_circuit_network::Aleo;
use snarkvm_circuit_types::{environment::prelude::*, Field, U16};

/// Compute the function ID as `Hash(network_id, program_id.len(), program_id, function_name.len(), function_name)`.
pub fn compute_function_id<A: Aleo>(
    network_id: &U16<A>,
    program_id: &ProgramID<A>,
    function_name: &Identifier<A>,
) -> Field<A> {
    A::hash_bhp1024(
        &(
            network_id,
            program_id.name().size_in_bits(),
            program_id.name(),
            program_id.network().size_in_bits(),
            program_id.network(),
            function_name.size_in_bits(),
            function_name,
        )
            .to_bits_le(),
    )
}
