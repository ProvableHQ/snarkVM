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

impl<E: Environment> ToBits for Boolean<E> {
    /// Outputs `self` in a vector.
    fn write_bits_le(&self, vec: &mut Vec<bool>) {
        vec.push(**self);
    }

    /// Outputs `self` in a vector.
    fn write_bits_be(&self, vec: &mut Vec<bool>) {
        vec.push(**self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 10_000;

    #[test]
    fn test_to_bits_le() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random value.
            let boolean: Boolean<CurrentEnvironment> = Uniform::rand(&mut rng);

            let candidate = boolean.to_bits_le();
            assert_eq!(vec![*boolean], candidate);
            assert_eq!(Boolean::<CurrentEnvironment>::size_in_bits(), candidate.len());
        }
    }

    #[test]
    fn test_to_bits_be() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random value.
            let boolean: Boolean<CurrentEnvironment> = Uniform::rand(&mut rng);

            let candidate = boolean.to_bits_be();
            assert_eq!(vec![*boolean], candidate);
            assert_eq!(Boolean::<CurrentEnvironment>::size_in_bits(), candidate.len());
        }
    }
}
