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

impl<A: Aleo> FromField for Identifier<A> {
    type Field = Field<A>;

    /// Initializes a new identifier from a field element.
    fn from_field(field: Self::Field) -> Self {
        // Convert the field to a list of little-endian bits.
        Self::from_bits_le(&field.to_bits_le())
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use crate::{data::identifier::tests::sample_console_identifier, Circuit};

    use anyhow::Result;

    const ITERATIONS: u64 = 100;

    fn check_from_field(num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) -> Result<()> {
        for _ in 0..ITERATIONS {
            // Initialize the console identifier.
            let console_identifier = sample_console_identifier::<Circuit>()?;
            // Initialize the circuit list of bits.
            let circuit_field = Field::constant(console::ToField::to_field(&console_identifier)?);

            Circuit::scope("Identifier FromField", || {
                let candidate = Identifier::<Circuit>::from_field(circuit_field);
                assert_eq!(Mode::Constant, candidate.eject_mode());
                assert_eq!(console_identifier, candidate.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_from_field() -> Result<()> {
        check_from_field(253, 0, 0, 0)
    }
}
