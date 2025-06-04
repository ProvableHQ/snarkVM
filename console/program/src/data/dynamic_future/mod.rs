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

mod bytes;
mod equal;
mod find;
mod from_future;
mod parse;
mod serialize;
mod to_bits;
mod to_fields;

use crate::{Access, Identifier, ProgramID, Value};
use snarkvm_console_network::Network;
use snarkvm_console_types::prelude::*;

// TODO (@d0cd). Implement `FromBytes` and `FromBits` for `DynamicFuture`.

/// A future.
#[derive(Clone)]
pub struct DynamicFuture<N: Network> {
    /// The program ID.
    program_id: ProgramID<N>,
    /// The name of the function.
    function_name: Identifier<N>,
    /// The commitment.
    commitment: Field<N>,
    // TODO (@d0cd). The length of the arguments? The optional arguments?
    // TODO (@d0cd). Should the `program_id` and `function_name` of the dynamic future be accessible
}

impl<N: Network> DynamicFuture<N> {
    /// Initializes a new future.
    #[inline]
    pub const fn new(program_id: ProgramID<N>, function_name: Identifier<N>, commitment: Field<N>) -> Self {
        Self { program_id, function_name, commitment }
    }

    /// Returns the program ID.
    #[inline]
    pub const fn program_id(&self) -> &ProgramID<N> {
        &self.program_id
    }

    /// Returns the name of the function.
    #[inline]
    pub const fn function_name(&self) -> &Identifier<N> {
        &self.function_name
    }

    /// Returns the commitment.
    #[inline]
    pub fn commitment(&self) -> &Field<N> {
        &self.commitment
    }
}
