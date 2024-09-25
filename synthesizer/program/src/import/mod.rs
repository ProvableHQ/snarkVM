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

use console::{
    network::prelude::*,
    program::{Identifier, ProgramID},
};

/// An import statement defines an imported program, and is of the form `import {name}.{network};`.
/// If no `network`-level domain is specified, the default network is used.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Import<N: Network> {
    /// The imported program ID.
    program_id: ProgramID<N>,
}

impl<N: Network> Import<N> {
    /// Returns the imported program ID.
    #[inline]
    pub const fn program_id(&self) -> &ProgramID<N> {
        &self.program_id
    }

    /// Returns the imported program name.
    #[inline]
    pub const fn name(&self) -> &Identifier<N> {
        self.program_id.name()
    }

    /// Returns the imported network-level domain (NLD).
    #[inline]
    pub const fn network(&self) -> &Identifier<N> {
        self.program_id.network()
    }
}

impl<N: Network> TypeName for Import<N> {
    /// Returns the type name as a string.
    #[inline]
    fn type_name() -> &'static str {
        "import"
    }
}

impl<N: Network> Ord for Import<N> {
    /// Ordering is determined by the NLD first, then the program name second.
    fn cmp(&self, other: &Self) -> Ordering {
        self.program_id.cmp(&other.program_id)
    }
}

impl<N: Network> PartialOrd for Import<N> {
    /// Ordering is determined by the NLD first, then the program name second.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_import_type_name() -> Result<()> {
        assert_eq!(Import::<CurrentNetwork>::type_name(), "import");
        Ok(())
    }

    #[test]
    fn test_import_partial_ord() -> Result<()> {
        let import1 = Import::<CurrentNetwork>::from_str("import bar.aleo;")?;
        let import2 = Import::<CurrentNetwork>::from_str("import foo.aleo;")?;

        let import3 = Import::<CurrentNetwork>::from_str("import bar.aleo;")?;
        let import4 = Import::<CurrentNetwork>::from_str("import foo.aleo;")?;

        assert_eq!(import1.partial_cmp(&import1), Some(Ordering::Equal));
        assert_eq!(import1.partial_cmp(&import2), Some(Ordering::Less));
        assert_eq!(import1.partial_cmp(&import3), Some(Ordering::Equal));
        assert_eq!(import1.partial_cmp(&import4), Some(Ordering::Less));

        assert_eq!(import2.partial_cmp(&import1), Some(Ordering::Greater));
        assert_eq!(import2.partial_cmp(&import2), Some(Ordering::Equal));
        assert_eq!(import2.partial_cmp(&import3), Some(Ordering::Greater));
        assert_eq!(import2.partial_cmp(&import4), Some(Ordering::Equal));

        assert_eq!(import3.partial_cmp(&import1), Some(Ordering::Equal));
        assert_eq!(import3.partial_cmp(&import2), Some(Ordering::Less));
        assert_eq!(import3.partial_cmp(&import3), Some(Ordering::Equal));
        assert_eq!(import3.partial_cmp(&import4), Some(Ordering::Less));

        assert_eq!(import4.partial_cmp(&import1), Some(Ordering::Greater));
        assert_eq!(import4.partial_cmp(&import2), Some(Ordering::Equal));
        assert_eq!(import4.partial_cmp(&import3), Some(Ordering::Greater));
        assert_eq!(import4.partial_cmp(&import4), Some(Ordering::Equal));

        Ok(())
    }
}
