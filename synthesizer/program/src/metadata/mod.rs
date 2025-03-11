// Copyright 2024-2025 Aleo Network Foundation
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

/// A metadata declaration of the form `$metadata <identifier>: <literal>;`
/// For example, `$metadata foo: 42u32;`
#[derive(Clone, PartialEq, Eq)]
pub struct ProgramMetadata<N: Network> {
    /// The name.
    name: Identifier<N>,
    /// The value.
    value: Literal<N>,
}

impl<N: Network> ProgramMetadata<N> {
    /// Initializes a new metadata declaration with the given name and value.
    pub fn new(name: Identifier<N>, value: Literal<N>) -> Self {
        Self { name, value }
    }

    /// Returns the name.
    pub const fn name(&self) -> &Identifier<N> {
        &self.name
    }

    /// Returns the value.
    pub const fn value(&self) -> &Literal<N> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use console::{
        network::MainnetV0,
        types::{Boolean, U32},
    };

    type CurrentNetwork = MainnetV0;

    fn test_serde(expected_string: &str) -> Result<()> {
        let expected = ProgramMetadata::<CurrentNetwork>::from_str(expected_string)?;
        // Check that the metadata can be serialized and deserialized from bytes correctly.
        let expected_bytes = expected.to_bytes_le()?;
        let candidate = ProgramMetadata::<CurrentNetwork>::from_bytes_le(&expected_bytes)?;
        assert_eq!(expected, candidate);
        assert_eq!(expected_bytes, candidate.to_bytes_le()?);
        // Check that the metadata can be serialized and deserialized from a string correctly.
        let candidate = ProgramMetadata::<CurrentNetwork>::from_str(expected_string)?;
        assert_eq!(expected, candidate);
        assert_eq!(expected_string, &candidate.to_string());
        Ok(())
    }

    #[test]
    fn test_metadata_serde() -> Result<()> {
        test_serde("$metadata bar: true;")?;
        test_serde("$metadata foo: 1u64;")?;
        test_serde("$metadata admin: aleo1rhgdu77hgyqd3xjj8ucu3jj9r2krwz6mnzyd80gncr5fxcwlh5rsvzp9px;")?;
        Ok(())
    }

    #[test]
    fn test_metadata_parse() -> Result<()> {
        // Failing cases.
        assert!(ProgramMetadata::<CurrentNetwork>::from_str("foo: u64;").is_err()); // Missing `$metadata` keyword.
        assert!(ProgramMetadata::<CurrentNetwork>::from_str("$metadata foo: u64").is_err()); // Missing semicolon.
        assert!(ProgramMetadata::<CurrentNetwork>::from_str("$metadata foo 1u64;").is_err()); // Missing colon.
        assert!(ProgramMetadata::<CurrentNetwork>::from_str("$metadata foo: [1u32, 2u32];").is_err()); //  Not a literal.

        // Passing cases.
        let metadata = ProgramMetadata::<CurrentNetwork>::from_str("$metadata foo: 42u32;")?;
        assert_eq!(metadata.name(), &Identifier::from_str("foo")?);
        assert_eq!(metadata.value(), &Literal::U32(U32::new(42)));

        let metadata = ProgramMetadata::<CurrentNetwork>::from_str("$metadata bar: true;")?;
        assert_eq!(metadata.name(), &Identifier::from_str("bar")?);
        assert_eq!(metadata.value(), &Literal::Boolean(Boolean::new(true)));

        Ok(())
    }
}
