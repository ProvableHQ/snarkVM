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
    type Boolean = Boolean<E>;

    fn one() -> Self {
        Self::constant(console::Scalar::one())
    }

    fn is_one(&self) -> Self::Boolean {
        self.is_equal(&Self::one())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    #[test]
    fn test_one() {
        let one = console::Scalar::<<Circuit as Environment>::Network>::one();

        Circuit::scope("One", || {
            assert_scope!(0, 0, 0, 0);
            let candidate = Scalar::<Circuit>::one();
            assert_eq!(one, candidate.eject_value());
            assert_scope!(1, 0, 0, 0);
        });
    }

    #[test]
    fn test_is_one() {
        let candidate = Scalar::<Circuit>::one();
        // Should equal 1.
        assert!(candidate.is_one().eject_value());
        // Should not equal 0.
        assert!(!candidate.is_zero().eject_value());
    }
}
