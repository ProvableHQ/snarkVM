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

mod equal;
mod find;
mod to_bits;
mod to_fields;

use crate::{Access, Identifier, ProgramID, Value};
use snarkvm_circuit_network::Aleo;
use snarkvm_circuit_types::{Boolean, Field, environment::prelude::*};

/// A dynamic future.
#[derive(Clone)]
pub struct DynamicFuture<A: Aleo> {
    /// The program ID.
    program_id: ProgramID<A>,
    /// The name of the function.
    function_name: Identifier<A>,
    /// The commitment.
    commitment: Field<A>,
}

impl<A: Aleo> Inject for DynamicFuture<A> {
    type Primitive = console::DynamicFuture<A::Network>;

    /// Initializes a circuit of the given mode and future.
    fn new(mode: Mode, value: Self::Primitive) -> Self {
        Self::from(
            ProgramID::new_unchecked(mode, *value.program_id()),
            Identifier::new_unchecked(mode, *value.function_name()),
            Inject::new(mode, *value.commitment()),
        )
    }
}

impl<A: Aleo> Eject for DynamicFuture<A> {
    type Primitive = console::DynamicFuture<A::Network>;

    /// Ejects the mode of the circuit future.
    fn eject_mode(&self) -> Mode {
        let program_id_mode = Eject::eject_mode(self.program_id());
        let function_name_mode = Eject::eject_mode(self.function_name());
        let commitment_mode = Eject::eject_mode(self.commitment());
        Mode::combine(Mode::combine(program_id_mode, function_name_mode), commitment_mode)
    }

    /// Ejects the circuit value.
    fn eject_value(&self) -> Self::Primitive {
        Self::Primitive::new(
            Eject::eject_value(self.program_id()),
            Eject::eject_value(self.function_name()),
            Eject::eject_value(self.commitment()),
        )
    }
}

impl<A: Aleo> DynamicFuture<A> {
    /// Returns a future from the given program ID, function name, and arguments.
    #[inline]
    pub const fn from(program_id: ProgramID<A>, function_name: Identifier<A>, commitment: Field<A>) -> Self {
        Self { program_id, function_name, commitment }
    }

    /// Returns the program ID.
    #[inline]
    pub const fn program_id(&self) -> &ProgramID<A> {
        &self.program_id
    }

    /// Returns the name of the function.
    #[inline]
    pub const fn function_name(&self) -> &Identifier<A> {
        &self.function_name
    }

    /// Returns the commitment.
    #[inline]
    pub fn commitment(&self) -> &Field<A> {
        &self.commitment
    }
}
