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

impl<N: Network> Eq for Future<N> {}

impl<N: Network> PartialEq for Future<N> {
    /// Returns `true` if `self` and `other` are equal.
    fn eq(&self, other: &Self) -> bool {
        *self.is_equal(other)
    }
}

impl<N: Network> Equal<Self> for Future<N> {
    type Output = Boolean<N>;

    /// Returns `true` if `self` and `other` are equal.
    fn is_equal(&self, other: &Self) -> Self::Output {
        // Ensure the `arguments` are the same length.
        if self.arguments.len() != other.arguments.len() {
            return Boolean::new(false);
        }

        // Check the `program_id`, and `function_name`.
        if !(*self.program_id.is_equal(&other.program_id) && *self.function_name.is_equal(&other.function_name)) {
            return Boolean::new(false);
        }

        // Recursively check each argument for equality.
        if self
            .arguments
            .iter()
            .zip_eq(other.arguments.iter())
            .all(|(argument_a, argument_b)| *argument_a.is_equal(argument_b))
        {
            Boolean::new(true)
        } else {
            Boolean::new(false)
        }
    }

    /// Returns `true` if `self` and `other` are *not* equal.
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        !self.is_equal(other)
    }
}
