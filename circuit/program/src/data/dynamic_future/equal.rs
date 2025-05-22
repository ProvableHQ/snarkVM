// Copyright (c) 2019-2025 Provable Inc.
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

impl<A: Aleo> Equal<Self> for DynamicFuture<A> {
    type Output = Boolean<A>;

    /// Returns `true` if `self` and `other` are equal.
    fn is_equal(&self, other: &Self) -> Self::Output {
        // Check the `program_id`, `function_name`, and `commitment` are equal.
        self.program_id.is_equal(&other.program_id)
            & self.function_name.is_equal(&other.function_name)
            & self.commitment.is_equal(&other.commitment)
    }

    /// Returns `true` if `self` and `other` are *not* equal.
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        // Check the `program_id`, `function_name`, or `commitment` are not equal.
        self.program_id.is_not_equal(&other.program_id)
            | self.function_name.is_not_equal(&other.function_name)
            | self.commitment.is_not_equal(&other.commitment)
    }
}
