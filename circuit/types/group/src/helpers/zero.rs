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

impl<E: Environment> Zero for Group<E> {
    type Boolean = Boolean<E>;

    fn zero() -> Self {
        Group { x: Field::zero(), y: Field::one() }
    }

    fn is_zero(&self) -> Self::Boolean {
        let is_x_zero = self.x.is_equal(&Field::zero());
        let is_y_one = self.y.is_equal(&Field::one());
        is_x_zero & is_y_one
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    #[test]
    fn test_zero() {
        let zero = console::Field::zero();
        let one = console::Field::one();

        Circuit::scope("Zero", || {
            assert_scope!(0, 0, 0, 0);
            let candidate = Group::<Circuit>::zero().eject_value();
            assert_eq!(zero, candidate.to_x_coordinate());
            assert_eq!(one, candidate.to_y_coordinate());
            assert_scope!(0, 0, 0, 0);
        });
    }

    #[test]
    fn test_is_zero() {
        let candidate = Group::<Circuit>::zero();
        // Should equal 0.
        assert!(candidate.is_zero().eject_value());
    }
}
