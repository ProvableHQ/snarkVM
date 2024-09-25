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

mod bytes;
mod parse;
mod serialize;
mod to_fields;

use crate::{Identifier, ProgramID};
use snarkvm_console_network::prelude::*;
use snarkvm_console_types::Field;

/// A locator is of the form `{program_id}/{resource}` (i.e. `howard.aleo/notify`).
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Locator<N: Network> {
    /// The program ID.
    id: ProgramID<N>,
    /// The program resource.
    resource: Identifier<N>,
}

impl<N: Network> Locator<N> {
    /// Initializes a locator from a program ID and resource.
    pub const fn new(program_id: ProgramID<N>, resource: Identifier<N>) -> Self {
        Self { id: program_id, resource }
    }
}

impl<N: Network> Locator<N> {
    /// Returns the program ID.
    #[inline]
    pub const fn program_id(&self) -> &ProgramID<N> {
        &self.id
    }

    /// Returns the program name.
    #[inline]
    pub const fn name(&self) -> &Identifier<N> {
        self.id.name()
    }

    /// Returns the network-level domain (NLD).
    #[inline]
    pub const fn network(&self) -> &Identifier<N> {
        self.id.network()
    }

    /// Returns the resource name.
    #[inline]
    pub const fn resource(&self) -> &Identifier<N> {
        &self.resource
    }
}
