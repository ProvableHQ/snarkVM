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

mod argument;
pub use argument::Argument;

mod bytes;
mod equal;
mod find;
mod parse;
mod serialize;
mod to_bits;
mod to_fields;

use crate::{Access, DynamicFuture, Identifier, Plaintext, ProgramID, Value};
use snarkvm_console_network::Network;
use snarkvm_console_types::prelude::*;

/// A future.
#[derive(Clone)]
pub struct Future<N: Network> {
    /// The program ID.
    program_id: ProgramID<N>,
    /// The name of the function.
    function_name: Identifier<N>,
    /// The arguments.
    arguments: Vec<Argument<N>>,
}

impl<N: Network> Future<N> {
    /// Initializes a new future.
    #[inline]
    pub const fn new(program_id: ProgramID<N>, function_name: Identifier<N>, arguments: Vec<Argument<N>>) -> Self {
        Self { program_id, function_name, arguments }
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

    /// Returns the arguments.
    #[inline]
    pub fn arguments(&self) -> &[Argument<N>] {
        &self.arguments
    }

    /// Checks that the bit encoding of this future's arguments fits its size prefixes.
    ///
    /// A future writes the size of each argument in front of it, falling back to a wider
    /// size for large arguments, so the argument sizes themselves cannot halt the process.
    /// However, a plaintext argument is bit-encoded like a plaintext, with a `u16` size in
    /// front of every nested struct member and array element, and writing a larger value
    /// halts the process. Callers must run this check on untrusted futures before
    /// bit-encoding them, so that oversized plaintext arguments are rejected instead of
    /// halting.
    pub fn check_bit_lengths(&self) -> Result<()> {
        // Check the bit encoding of each argument.
        for argument in &self.arguments {
            match argument {
                // A plaintext argument is bit-encoded like a plaintext.
                Argument::Plaintext(plaintext) => plaintext.check_bit_lengths()?,
                // A future argument is bit-encoded argument by argument.
                Argument::Future(future) => future.check_bit_lengths()?,
                // A dynamic future argument has a fixed-size bit encoding.
                Argument::DynamicFuture(..) => {}
            }
        }
        Ok(())
    }
}
