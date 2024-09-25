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

impl<E: Environment, I: IntegerType> ToBits for Integer<E, I> {
    /// Outputs the little-endian bit representation of `self` *without* trailing zeros.
    fn write_bits_le(&self, vec: &mut Vec<bool>) {
        (**self).write_bits_le(vec);
    }

    /// Outputs the big-endian bit representation of `self` *without* leading zeros.
    fn write_bits_be(&self, vec: &mut Vec<bool>) {
        (**self).write_bits_be(vec);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 10_000;

    fn check_to_bits_le<I: IntegerType>(rng: &mut TestRng) {
        for _ in 0..ITERATIONS {
            // Sample a random value.
            let integer: Integer<CurrentEnvironment, I> = Uniform::rand(rng);

            let candidate = integer.to_bits_le();
            assert_eq!(Integer::<CurrentEnvironment, I>::size_in_bits(), candidate.len());

            for (expected, candidate) in (*integer).to_bits_le().iter().zip_eq(&candidate) {
                assert_eq!(expected, candidate);
            }
        }
    }

    fn check_to_bits_be<I: IntegerType>(rng: &mut TestRng) {
        for _ in 0..ITERATIONS {
            // Sample a random value.
            let integer: Integer<CurrentEnvironment, I> = Uniform::rand(rng);

            let candidate = integer.to_bits_be();
            assert_eq!(Integer::<CurrentEnvironment, I>::size_in_bits(), candidate.len());

            for (expected, candidate) in (*integer).to_bits_be().iter().zip_eq(&candidate) {
                assert_eq!(expected, candidate);
            }
        }
    }

    #[test]
    fn test_to_bits_le() {
        let mut rng = TestRng::default();

        check_to_bits_le::<u8>(&mut rng);
        check_to_bits_le::<u16>(&mut rng);
        check_to_bits_le::<u32>(&mut rng);
        check_to_bits_le::<u64>(&mut rng);
        check_to_bits_le::<u128>(&mut rng);

        check_to_bits_le::<i8>(&mut rng);
        check_to_bits_le::<i16>(&mut rng);
        check_to_bits_le::<i32>(&mut rng);
        check_to_bits_le::<i64>(&mut rng);
        check_to_bits_le::<i128>(&mut rng);
    }

    #[test]
    fn test_to_bits_be() {
        let mut rng = TestRng::default();

        check_to_bits_be::<u8>(&mut rng);
        check_to_bits_be::<u16>(&mut rng);
        check_to_bits_be::<u32>(&mut rng);
        check_to_bits_be::<u64>(&mut rng);
        check_to_bits_be::<u128>(&mut rng);

        check_to_bits_be::<i8>(&mut rng);
        check_to_bits_be::<i16>(&mut rng);
        check_to_bits_be::<i32>(&mut rng);
        check_to_bits_be::<i64>(&mut rng);
        check_to_bits_be::<i128>(&mut rng);
    }
}
