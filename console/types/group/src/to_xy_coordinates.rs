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
    /// Returns the *x-coordinate* and *y-coordinate* in the affine coordinates of the group.
    pub fn to_xy_coordinates(&self) -> (Field<E>, Field<E>) {
        // Convert to affine.
        let affine = self.group.to_affine();
        // Returns the (x, y) coordinates.
        (Field::new(affine.to_x_coordinate()), Field::new(affine.to_y_coordinate()))
    }
}
