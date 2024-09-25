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

impl<A: Aleo> Equal<Self> for Future<A> {
    type Output = Boolean<A>;

    /// Returns `true` if `self` and `other` are equal.
    fn is_equal(&self, other: &Self) -> Self::Output {
        // Ensure the `arguments` are the same length.
        if self.arguments.len() != other.arguments.len() {
            return Boolean::constant(false);
        }

        // Recursively check each argument for equality.
        let mut equal = Boolean::constant(true);
        for (argument_a, argument_b) in self.arguments.iter().zip_eq(other.arguments.iter()) {
            equal &= argument_a.is_equal(argument_b);
        }

        // Check the `program_id`, `function_name`, and arguments are equal.
        self.program_id.is_equal(&other.program_id) & self.function_name.is_equal(&other.function_name) & equal
    }

    /// Returns `true` if `self` and `other` are *not* equal.
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        // Check the `arguments` lengths.
        if self.arguments.len() != other.arguments.len() {
            return Boolean::constant(true);
        }

        // Recursively check each argument for equality.
        let mut not_equal = Boolean::constant(false);
        for (argument_a, argument_b) in self.arguments.iter().zip_eq(other.arguments.iter()) {
            not_equal |= argument_a.is_not_equal(argument_b);
        }

        // Check the `program_id`, `function_name`, or arguments are not equal.
        self.program_id.is_not_equal(&other.program_id)
            | self.function_name.is_not_equal(&other.function_name)
            | not_equal
    }
}
