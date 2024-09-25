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

#[cfg(test)]
use snarkvm_circuit_types::environment::assert_scope;

mod to_address;
mod to_bits;
mod to_fields;

use crate::Identifier;
use snarkvm_circuit_network::Aleo;
use snarkvm_circuit_types::{environment::prelude::*, Address, Boolean, Field};

/// A program ID is of the form `{name}.{network}`.
/// If no `network`-level domain is specified, the default network is used.
#[derive(Clone)]
pub struct ProgramID<A: Aleo> {
    /// The program name.
    name: Identifier<A>,
    /// The network-level domain (NLD).
    network: Identifier<A>,
}

#[cfg(console)]
impl<A: Aleo> Inject for ProgramID<A> {
    type Primitive = console::ProgramID<A::Network>;

    /// Injects a program ID with the given primitive.
    fn new(_: Mode, id: Self::Primitive) -> Self {
        Self {
            name: Identifier::new(Mode::Constant, *id.name()),
            network: Identifier::new(Mode::Constant, *id.network()),
        }
    }
}

impl<A: Aleo> ProgramID<A> {
    /// Returns the program name.
    #[inline]
    pub const fn name(&self) -> &Identifier<A> {
        &self.name
    }

    /// Returns the network-level domain (NLD).
    #[inline]
    pub const fn network(&self) -> &Identifier<A> {
        &self.network
    }
}

#[cfg(console)]
impl<A: Aleo> Eject for ProgramID<A> {
    type Primitive = console::ProgramID<A::Network>;

    /// Ejects the mode of the program ID.
    fn eject_mode(&self) -> Mode {
        Mode::Constant
    }

    /// Ejects a program ID into a primitive.
    fn eject_value(&self) -> Self::Primitive {
        match console::ProgramID::try_from((self.name.eject_value(), self.network.eject_value())) {
            Ok(id) => id,
            Err(error) => A::halt(format!("Failed to eject program ID: {error}")),
        }
    }
}

impl<A: Aleo> Equal<Self> for ProgramID<A> {
    type Output = Boolean<A>;

    /// Returns `true` if `self` and `other` are equal.
    fn is_equal(&self, other: &Self) -> Self::Output {
        self.name.is_equal(&other.name) & (self.network.is_equal(&other.network))
    }

    /// Returns `true` if `self` and `other` are **not** equal.
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        self.name.is_not_equal(&other.name) | (self.network.is_not_equal(&other.network))
    }
}
