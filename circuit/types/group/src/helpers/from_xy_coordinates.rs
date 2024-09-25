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

impl<E: Environment> Group<E> {
    /// Initializes an affine group element from a given x- and y-coordinate field element.
    /// For safety, the resulting point is always enforced to be on the curve and in the subgroup.
    pub fn from_xy_coordinates(x: Field<E>, y: Field<E>) -> Self {
        let point = Self { x, y };
        point.enforce_in_group();
        point
    }

    /// Initializes an affine group element from a given x- and y-coordinate field element.
    /// Note: The resulting point is **not** enforced to be on the curve or in the subgroup.
    #[doc(hidden)]
    pub fn from_xy_coordinates_unchecked(x: Field<E>, y: Field<E>) -> Self {
        Self { x, y }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 100;

    fn check_from_xy_coordinates(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let point: console::Group<<Circuit as Environment>::Network> = Uniform::rand(&mut rng);

            // Inject the x- and y-coordinates.
            let x_coordinate = Field::new(mode, point.to_x_coordinate());
            let y_coordinate = Field::new(mode, point.to_y_coordinate());

            Circuit::scope(format!("{mode} {i}"), || {
                let affine = Group::<Circuit>::from_xy_coordinates(x_coordinate, y_coordinate);
                assert_eq!(point, affine.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
        }
    }

    fn check_from_xy_coordinates_unchecked(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random element.
            let point: console::Group<<Circuit as Environment>::Network> = Uniform::rand(&mut rng);

            // Inject the x- and y-coordinates.
            let x_coordinate = Field::new(mode, point.to_x_coordinate());
            let y_coordinate = Field::new(mode, point.to_y_coordinate());

            Circuit::scope(format!("{mode} {i}"), || {
                let affine = Group::<Circuit>::from_xy_coordinates_unchecked(x_coordinate, y_coordinate);
                assert_eq!(point, affine.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
        }
    }

    #[test]
    fn test_from_xy_coordinates_constant() {
        check_from_xy_coordinates(Mode::Constant, 8, 0, 0, 0);
    }

    #[test]
    fn test_from_xy_coordinates_public() {
        check_from_xy_coordinates(Mode::Public, 4, 0, 12, 13);
    }

    #[test]
    fn test_from_xy_coordinates_private() {
        check_from_xy_coordinates(Mode::Private, 4, 0, 12, 13);
    }

    #[test]
    fn test_from_xy_coordinates_unchecked_constant() {
        check_from_xy_coordinates_unchecked(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_from_xy_coordinates_unchecked_public() {
        check_from_xy_coordinates_unchecked(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_from_xy_coordinates_unchecked_private() {
        check_from_xy_coordinates_unchecked(Mode::Private, 0, 0, 0, 0);
    }
}
