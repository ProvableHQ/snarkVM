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

impl<E: Environment> Equal<Self> for StringType<E> {
    type Output = Boolean<E>;

    /// Returns `true` if `self` and `other` are equal.
    fn is_equal(&self, other: &Self) -> Self::Output {
        // Convert each string type into fields.
        let this = self.to_fields();
        let that = other.to_fields();

        // Check that the size in bytes of the two strings are equal.
        self.size_in_bytes.is_equal(&other.size_in_bytes)
            // Check that the string contents are equal.
            & this.iter().zip(&that).fold(Boolean::constant(true), |acc, (a, b)| acc & a.is_equal(b))
    }

    /// Returns `true` if `self` and `other` are *not* equal.
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        !self.is_equal(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    fn sample_string(mode: Mode, rng: &mut TestRng) -> StringType<Circuit> {
        // Sample a random string. Take 1/4th to ensure we fit for all code points.
        let given = rng.next_string(Circuit::MAX_STRING_BYTES / 4, true);
        StringType::<Circuit>::new(mode, console::StringType::new(&given))
    }

    fn check_is_equal(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        let mut rng = TestRng::default();

        // Sample two strings.
        let string_a = sample_string(mode, &mut rng);
        let string_b = sample_string(mode, &mut rng);

        Circuit::scope(format!("{mode}"), || {
            let candidate = string_a.is_equal(&string_a);
            assert!(candidate.eject_value());
            assert_scope!(<=num_constants, <=num_public, <=num_private, <=num_constraints);
        });

        Circuit::scope(format!("{mode}"), || {
            let candidate = string_a.is_equal(&string_b);
            assert!(!candidate.eject_value());
            assert_scope!(<=num_constants, <=num_public, <=num_private, <=num_constraints);
        });

        Circuit::reset();
        Ok(())
    }

    fn check_is_not_equal(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        let mut rng = TestRng::default();

        // Sample two strings.
        let string_a = sample_string(mode, &mut rng);
        let string_b = sample_string(mode, &mut rng);

        Circuit::scope(format!("{mode}"), || {
            let candidate = string_a.is_not_equal(&string_b);
            assert!(candidate.eject_value());
            assert_scope!(<=num_constants, <=num_public, <=num_private, <=num_constraints);
        });

        Circuit::scope(format!("{mode}"), || {
            let candidate = string_a.is_not_equal(&string_a);
            assert!(!candidate.eject_value());
            assert_scope!(<=num_constants, <=num_public, <=num_private, <=num_constraints);
        });

        Circuit::reset();
        Ok(())
    }

    #[test]
    fn test_is_equal_constant() -> Result<()> {
        check_is_equal(Mode::Constant, 9, 0, 0, 0)
    }

    #[test]
    fn test_is_equal_public() -> Result<()> {
        check_is_equal(Mode::Public, 9, 0, 26, 26)
    }

    #[test]
    fn test_is_equal_private() -> Result<()> {
        check_is_equal(Mode::Private, 9, 0, 26, 26)
    }

    #[test]
    fn test_is_not_equal_constant() -> Result<()> {
        check_is_not_equal(Mode::Constant, 9, 0, 0, 0)
    }

    #[test]
    fn test_is_not_equal_public() -> Result<()> {
        check_is_not_equal(Mode::Public, 9, 0, 26, 26)
    }

    #[test]
    fn test_is_not_equal_private() -> Result<()> {
        check_is_not_equal(Mode::Private, 9, 0, 26, 36)
    }
}
