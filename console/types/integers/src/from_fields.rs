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

use super::*;

impl<E: Environment, I: IntegerType> FromFields for Integer<E, I> {
    type Field = Field<E>;

    /// Initializes a new integer by recovering the **x-coordinate** of an affine group from a field element.
    fn from_fields(fields: &[Self::Field]) -> Result<Self> {
        // Ensure there is exactly one field element in the vector.
        ensure!(fields.len() == 1, "Integer must be recovered from a single field element");
        // Recover the integer from the field element.
        Self::from_field(&fields[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 10_000;

    fn check_from_fields<I: IntegerType>() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random integer.
            let expected = Integer::<CurrentEnvironment, I>::rand(&mut rng);

            // Perform the operation.
            let candidate = Integer::from_fields(&expected.to_fields()?)?;
            assert_eq!(expected, candidate);
        }
        Ok(())
    }

    #[test]
    fn test_u8_from_fields() -> Result<()> {
        type I = u8;
        check_from_fields::<I>()
    }

    #[test]
    fn test_i8_from_fields() -> Result<()> {
        type I = i8;
        check_from_fields::<I>()
    }

    #[test]
    fn test_u16_from_fields() -> Result<()> {
        type I = u16;
        check_from_fields::<I>()
    }

    #[test]
    fn test_i16_from_fields() -> Result<()> {
        type I = i16;
        check_from_fields::<I>()
    }

    #[test]
    fn test_u32_from_fields() -> Result<()> {
        type I = u32;
        check_from_fields::<I>()
    }

    #[test]
    fn test_i32_from_fields() -> Result<()> {
        type I = i32;
        check_from_fields::<I>()
    }

    #[test]
    fn test_u64_from_fields() -> Result<()> {
        type I = u64;
        check_from_fields::<I>()
    }

    #[test]
    fn test_i64_from_fields() -> Result<()> {
        type I = i64;
        check_from_fields::<I>()
    }

    #[test]
    fn test_u128_from_fields() -> Result<()> {
        type I = u128;
        check_from_fields::<I>()
    }

    #[test]
    fn test_i128_from_fields() -> Result<()> {
        type I = i128;
        check_from_fields::<I>()
    }
}
