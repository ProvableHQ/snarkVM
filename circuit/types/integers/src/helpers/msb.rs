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

impl<E: Environment, I: IntegerType> MSB for Integer<E, I> {
    type Boolean = Boolean<E>;

    /// Returns the MSB of the integer.
    fn msb(&self) -> &Self::Boolean {
        match self.bits_le.last() {
            Some(msb) => msb,
            // Note: `E::halt` should never be invoked as `self.bits_le.len()` is greater than zero.
            None => E::halt("Malformed integer detected while retrieving the MSB"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    fn check_msb<I: IntegerType>() {
        // Set the value to check to console::Integer::MAX.
        let value = Integer::<Circuit, I>::new(Mode::Private, console::Integer::MAX);

        // Prepare the expected outputs.
        let expected_signed = false;
        let expected_unsigned = true;

        Circuit::scope("MSB", || {
            assert_scope!(0, 0, 0, 0);
            match I::is_signed() {
                true => assert_eq!(expected_signed, value.msb().eject_value()),
                false => assert_eq!(expected_unsigned, value.msb().eject_value()),
            }
            assert_scope!(0, 0, 0, 0);
        });
    }

    #[test]
    fn test_u8_msb() {
        check_msb::<u8>();
    }

    #[test]
    fn test_i8_msb() {
        check_msb::<i8>();
    }

    #[test]
    fn test_u16_msb() {
        check_msb::<u16>();
    }

    #[test]
    fn test_i16_msb() {
        check_msb::<i16>();
    }

    #[test]
    fn test_u32_msb() {
        check_msb::<u32>();
    }

    #[test]
    fn test_i32_msb() {
        check_msb::<i32>();
    }

    #[test]
    fn test_u64_msb() {
        check_msb::<u64>();
    }

    #[test]
    fn test_i64_msb() {
        check_msb::<i64>();
    }

    #[test]
    fn test_u128_msb() {
        check_msb::<u128>();
    }

    #[test]
    fn test_i128_msb() {
        check_msb::<i128>();
    }
}
