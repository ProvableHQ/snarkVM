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
    program::{Register, RegisterType},
};

/// An input statement defines an input argument to a function, and is of the form
/// `input {register} as {register_type}`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Input<N: Network> {
    /// The input register.
    register: Register<N>,
    /// The input register type.
    register_type: RegisterType<N>,
}

impl<N: Network> Input<N> {
    /// Returns the input register.
    #[inline]
    pub const fn register(&self) -> &Register<N> {
        &self.register
    }

    /// Returns the input register type.
    #[inline]
    pub const fn register_type(&self) -> &RegisterType<N> {
        &self.register_type
    }
}

impl<N: Network> TypeName for Input<N> {
    /// Returns the type name as a string.
    #[inline]
    fn type_name() -> &'static str {
        "input"
    }
}

impl<N: Network> Ord for Input<N> {
    /// Ordering is determined by the register (the register type is ignored).
    fn cmp(&self, other: &Self) -> Ordering {
        self.register().cmp(other.register())
    }
}

impl<N: Network> PartialOrd for Input<N> {
    /// Ordering is determined by the register (the register type is ignored).
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
    fn test_input_type_name() -> Result<()> {
        assert_eq!(Input::<CurrentNetwork>::type_name(), "input");
        Ok(())
    }

    #[test]
    fn test_input_partial_ord() -> Result<()> {
        let input1 = Input::<CurrentNetwork>::from_str("input r0 as field;")?;
        let input2 = Input::<CurrentNetwork>::from_str("input r1 as field;")?;

        let input3 = Input::<CurrentNetwork>::from_str("input r0 as signature;")?;
        let input4 = Input::<CurrentNetwork>::from_str("input r1 as signature;")?;

        assert_eq!(input1.partial_cmp(&input1), Some(Ordering::Equal));
        assert_eq!(input1.partial_cmp(&input2), Some(Ordering::Less));
        assert_eq!(input1.partial_cmp(&input3), Some(Ordering::Equal));
        assert_eq!(input1.partial_cmp(&input4), Some(Ordering::Less));

        assert_eq!(input2.partial_cmp(&input1), Some(Ordering::Greater));
        assert_eq!(input2.partial_cmp(&input2), Some(Ordering::Equal));
        assert_eq!(input2.partial_cmp(&input3), Some(Ordering::Greater));
        assert_eq!(input2.partial_cmp(&input4), Some(Ordering::Equal));

        assert_eq!(input3.partial_cmp(&input1), Some(Ordering::Equal));
        assert_eq!(input3.partial_cmp(&input2), Some(Ordering::Less));
        assert_eq!(input3.partial_cmp(&input3), Some(Ordering::Equal));
        assert_eq!(input3.partial_cmp(&input4), Some(Ordering::Less));

        assert_eq!(input4.partial_cmp(&input1), Some(Ordering::Greater));
        assert_eq!(input4.partial_cmp(&input2), Some(Ordering::Equal));
        assert_eq!(input4.partial_cmp(&input3), Some(Ordering::Greater));
        assert_eq!(input4.partial_cmp(&input4), Some(Ordering::Equal));

        Ok(())
    }
}
