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

impl<E: Environment> Equal for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns `true` if `self` and `other` are equal.
    fn is_equal(&self, other: &Self) -> Self::Output {
        Boolean::new(self == other)
    }

    /// Returns `true` if `self` and `other` are *not* equal.
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        Boolean::new(self != other)
    }
}

impl<E: Environment> Not for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns the `negation` of `self`.
    #[inline]
    fn not(self) -> Self::Output {
        Boolean::new(!self.boolean)
    }
}

impl<E: Environment> BitAnd<Boolean<E>> for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns the bitwise `AND`` of `self` and `other`.
    #[inline]
    fn bitand(self, other: Self) -> Self::Output {
        Boolean::new(self.boolean & other.boolean)
    }
}

impl<E: Environment> BitAnd<&Boolean<E>> for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns the bitwise `AND` of `self` and `other`.
    #[inline]
    fn bitand(self, other: &Boolean<E>) -> Self::Output {
        Boolean::new(self.boolean & other.boolean)
    }
}

impl<E: Environment> BitAndAssign for Boolean<E> {
    /// Sets `self` as the bitwise `AND` of `self` and `other`.
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        *self = Boolean::new(self.boolean & other.boolean)
    }
}

impl<E: Environment> BitOr for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns the bitwise `OR` of `self` and `other`.
    #[inline]
    fn bitor(self, other: Self) -> Self::Output {
        Boolean::new(self.boolean | other.boolean)
    }
}

impl<E: Environment> BitOr<&Boolean<E>> for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns the bitwise `OR` of `self` and `other`.
    #[inline]
    fn bitor(self, other: &Boolean<E>) -> Self::Output {
        Boolean::new(self.boolean | other.boolean)
    }
}

impl<E: Environment> BitOrAssign for Boolean<E> {
    /// Sets `self` as the bitwise `OR` of `self` and `other`.
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        *self = Boolean::new(self.boolean | other.boolean)
    }
}

impl<E: Environment> BitXor for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns the bitwise `XOR` of `self` and `other`.
    #[inline]
    fn bitxor(self, other: Self) -> Self::Output {
        Boolean::new(self.boolean ^ other.boolean)
    }
}

impl<E: Environment> BitXor<&Boolean<E>> for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns the bitwise `XOR` of `self` and `other`.
    #[inline]
    fn bitxor(self, other: &Boolean<E>) -> Self::Output {
        Boolean::new(self.boolean ^ other.boolean)
    }
}

impl<E: Environment> BitXorAssign for Boolean<E> {
    /// Sets `self` as the bitwise `XOR` of `self` and `other`.
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        *self = Boolean::new(self.boolean ^ other.boolean)
    }
}

impl<E: Environment> Nand for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns the bitwise `NAND` of `self` and `other`.
    #[inline]
    fn nand(&self, other: &Self) -> Self::Output {
        Boolean::new(!(self.boolean & other.boolean))
    }
}

impl<E: Environment> Nor for Boolean<E> {
    type Output = Boolean<E>;

    /// Returns the bitwise `NOR` of `self` and `other`.
    #[inline]
    fn nor(&self, other: &Self) -> Self::Output {
        Boolean::new(!(self.boolean | other.boolean))
    }
}

impl<E: Environment> Ternary for Boolean<E> {
    type Boolean = Boolean<E>;
    type Output = Self;

    /// Returns `first` if `condition` is `true`, otherwise returns `second`.
    fn ternary(condition: &Self::Boolean, first: &Self, second: &Self) -> Self::Output {
        match **condition {
            true => *first,
            false => *second,
        }
    }
}
