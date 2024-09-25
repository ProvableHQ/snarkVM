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
mod serialize;
mod string;

use snarkvm_console_network::Network;
use snarkvm_console_types::prelude::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum InputID<N: Network> {
    /// The hash of the constant input.
    Constant(Field<N>),
    /// The hash of the public input.
    Public(Field<N>),
    /// The ciphertext hash of the private input.
    Private(Field<N>),
    /// The commitment, gamma, serial number, and tag of the record input.
    Record(Field<N>, Group<N>, Field<N>, Field<N>),
    /// The hash of the external record input.
    ExternalRecord(Field<N>),
}

impl<N: Network> InputID<N> {
    /// Returns the (primary) input ID.
    pub const fn id(&self) -> &Field<N> {
        match self {
            InputID::Constant(id) => id,
            InputID::Public(id) => id,
            InputID::Private(id) => id,
            InputID::Record(id, ..) => id,
            InputID::ExternalRecord(id) => id,
        }
    }
}
