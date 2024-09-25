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

impl<E: Environment> One for Scalar<E> {
    /// Returns the `1` element of the scalar.
    fn one() -> Self {
        Self::new(E::Scalar::one())
    }

    /// Returns `true` if the element is one.
    fn is_one(&self) -> bool {
        self.scalar.is_one()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 100;

    #[test]
    fn test_one() {
        let one = Scalar::<CurrentEnvironment>::one();

        for (index, bit) in one.to_bits_le().iter().enumerate() {
            match index == 0 {
                true => assert!(bit),
                false => assert!(!bit),
            }
        }
    }

    #[test]
    fn test_is_one() {
        assert!(Scalar::<CurrentEnvironment>::one().is_one());

        let mut rng = TestRng::default();

        // Note: This test technically has a `1 / MODULUS` probability of being flaky.
        for _ in 0..ITERATIONS {
            let scalar: Scalar<CurrentEnvironment> = Uniform::rand(&mut rng);
            assert!(!scalar.is_one());
        }
    }
}
