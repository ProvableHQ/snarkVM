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

use console::{
    network::Network,
    program::{Identifier, ProgramID},
};

#[derive(Copy, Clone, Debug)]
pub struct CallMetrics<N: Network> {
    pub program_id: ProgramID<N>,
    pub function_name: Identifier<N>,
    pub num_instructions: usize,
    pub num_request_constraints: u64,
    pub num_function_constraints: u64,
    pub num_response_constraints: u64,
}
