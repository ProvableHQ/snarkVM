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

impl<N: Network, Private: Visibility<Boolean = Boolean<N>>> Eq for Entry<N, Private> {}

impl<N: Network, Private: Visibility<Boolean = Boolean<N>>> PartialEq for Entry<N, Private> {
    /// Returns `true` if `self` and `other` are equal.
    fn eq(&self, other: &Self) -> bool {
        *self.is_equal(other)
    }
}

impl<N: Network, Private: Visibility<Boolean = Boolean<N>>> Equal<Self> for Entry<N, Private> {
    type Output = Boolean<N>;

    /// Returns `true` if `self` and `other` are equal.
    fn is_equal(&self, other: &Self) -> Self::Output {
        match (self, other) {
            (Self::Constant(a), Self::Constant(b)) => a.is_equal(b),
            (Self::Public(a), Self::Public(b)) => a.is_equal(b),
            (Self::Private(a), Self::Private(b)) => a.is_equal(b),
            (Self::Constant(_), _) | (Self::Public(_), _) | (Self::Private(_), _) => Boolean::new(false),
        }
    }

    /// Returns `true` if `self` and `other` are *not* equal.
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        match (self, other) {
            (Self::Constant(a), Self::Constant(b)) => a.is_not_equal(b),
            (Self::Public(a), Self::Public(b)) => a.is_not_equal(b),
            (Self::Private(a), Self::Private(b)) => a.is_not_equal(b),
            (Self::Constant(_), _) | (Self::Public(_), _) | (Self::Private(_), _) => Boolean::new(true),
        }
    }
}
