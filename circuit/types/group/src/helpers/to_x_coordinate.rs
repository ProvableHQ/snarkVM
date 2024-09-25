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
    /// Returns the x-coordinate of the group element.
    pub fn to_x_coordinate(&self) -> Field<E> {
        self.x.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;

    const ITERATIONS: u64 = 100;

    fn check_to_x_coordinate(mode: Mode) {
        for i in 0..ITERATIONS {
            // Sample a random element.
            let expected = Uniform::rand(&mut TestRng::default());
            let candidate = Group::<Circuit>::new(mode, expected);

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = candidate.to_x_coordinate();
                assert_eq!(expected.to_x_coordinate(), candidate.eject_value());
                assert_scope!(0, 0, 0, 0);
            });
        }
    }

    #[test]
    fn test_to_x_coordinate_constant() {
        check_to_x_coordinate(Mode::Constant);
    }

    #[test]
    fn test_to_x_coordinate_public() {
        check_to_x_coordinate(Mode::Public);
    }

    #[test]
    fn test_to_x_coordinate_private() {
        check_to_x_coordinate(Mode::Private);
    }
}
