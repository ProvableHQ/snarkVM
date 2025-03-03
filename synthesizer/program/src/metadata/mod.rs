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

use super::*;

#[derive(Clone, PartialEq, Eq)]
pub struct ProgramMetadata<N: Network> {
    /// The name.
    name: Identifier<N>,
    /// The value.
    value: Plaintext<N>,
}

impl<N: Network> ProgramMetadata<N> {
    /// Initializes a new metadata declaration with the given name and value.
    pub fn new(name: Identifier<N>, value: Plaintext<N>) -> Self {
        Self { name, value }
    }

    /// Returns the name.
    pub const fn name(&self) -> &Identifier<N> {
        &self.name
    }

    /// Returns the value.
    pub const fn value(&self) -> &Plaintext<N> {
        &self.value
    }
}

impl<N: Network> TypeName for ProgramMetadata<N> {
    /// Returns the type name as a string.
    #[inline]
    fn type_name() -> &'static str {
        "$metadata"
    }
}
