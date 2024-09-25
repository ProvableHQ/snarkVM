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
    /// Initializes a new group from the `(x, y)` affine coordinates.
    pub fn from_xy_coordinates(x: Field<E>, y: Field<E>) -> Self {
        match E::Affine::from_coordinates((*x, *y)) {
            Some(point) => Self { group: point.into() },
            None => E::halt("Attempted to recover an invalid group element from (x, y) coordinates"),
        }
    }

    /// Initializes a new group from the `(x, y)` affine coordinates.
    /// Note: The resulting point is **not** enforced to be on the curve or in the subgroup.
    pub fn from_xy_coordinates_unchecked(x: Field<E>, y: Field<E>) -> Self {
        Self { group: E::Affine::from_coordinates_unchecked((*x, *y)).into() }
    }
}
