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

use crate::PlaintextType;
use snarkvm_console_network::prelude::*;

use enum_index::EnumIndex;

#[derive(Clone, PartialEq, Eq, Hash, EnumIndex)]
pub enum EntryType<N: Network> {
    /// A constant type.
    Constant(PlaintextType<N>),
    /// A publicly-visible type.
    Public(PlaintextType<N>),
    /// A private type.
    Private(PlaintextType<N>),
}

impl<N: Network> EntryType<N> {
    /// Returns the plaintext type.
    pub const fn plaintext_type(&self) -> &PlaintextType<N> {
        match self {
            EntryType::Constant(plaintext_type) => plaintext_type,
            EntryType::Public(plaintext_type) => plaintext_type,
            EntryType::Private(plaintext_type) => plaintext_type,
        }
    }
}
