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

use console::{network::prelude::*, program::PlaintextType};

/// A key statement is of the form `key as {plaintext_type}.public`.
#[derive(Clone, PartialEq, Eq)]
pub struct MapKey<N: Network> {
    /// The key plaintext type.
    plaintext_type: PlaintextType<N>,
}

impl<N: Network> MapKey<N> {
    /// Returns the key plaintext type.
    #[inline]
    pub const fn plaintext_type(&self) -> &PlaintextType<N> {
        &self.plaintext_type
    }
}

impl<N: Network> TypeName for MapKey<N> {
    /// Returns the type name as a string.
    #[inline]
    fn type_name() -> &'static str {
        "key"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_key_type_name() -> Result<()> {
        assert_eq!(MapKey::<CurrentNetwork>::type_name(), "key");
        Ok(())
    }
}
