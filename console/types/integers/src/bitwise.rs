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

impl<E: Environment, I: IntegerType> Equal for Integer<E, I> {
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

impl<E: Environment, I: IntegerType> Compare<Self> for Integer<E, I> {
    type Output = Boolean<E>;

    /// Returns `true` if `self` is less than `other`.
    fn is_less_than(&self, other: &Self) -> Self::Output {
        Boolean::new(self.integer < other.integer)
    }

    /// Returns `true` if `self` is greater than `other`.
    fn is_greater_than(&self, other: &Self) -> Self::Output {
        other.is_less_than(self)
    }

    /// Returns `true` if `self` is less than or equal to `other`.
    fn is_less_than_or_equal(&self, other: &Self) -> Self::Output {
        other.is_greater_than_or_equal(self)
    }

    /// Returns `true` if `self` is greater than or equal to `other`.
    fn is_greater_than_or_equal(&self, other: &Self) -> Self::Output {
        !self.is_less_than(other)
    }
}

impl<E: Environment, I: IntegerType> Not for Integer<E, I> {
    type Output = Self;

    /// Returns the bitwise `NOT` of `self`.
    #[inline]
    fn not(self) -> Self::Output {
        Integer::new(self.integer.not())
    }
}

impl<E: Environment, I: IntegerType> BitAnd for Integer<E, I> {
    type Output = Self;

    /// Returns the bitwise `AND` of `self` and `other`.
    #[inline]
    fn bitand(self, other: Self) -> Self::Output {
        Integer::new(self.integer & other.integer)
    }
}

impl<E: Environment, I: IntegerType> BitAnd<&Integer<E, I>> for Integer<E, I> {
    type Output = Self;

    /// Returns the bitwise `AND` of `self` and `other`.
    #[inline]
    fn bitand(self, other: &Integer<E, I>) -> Self::Output {
        Integer::new(self.integer & other.integer)
    }
}

impl<E: Environment, I: IntegerType> BitAndAssign for Integer<E, I> {
    /// Performs the bitwise `AND` of `self` and `other` and assigns the result to `self`.
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.integer = self.integer & other.integer;
    }
}

impl<E: Environment, I: IntegerType> BitOr for Integer<E, I> {
    type Output = Self;

    /// Returns the bitwise `OR` of `self` and `other`.
    #[inline]
    fn bitor(self, other: Self) -> Self::Output {
        Integer::new(self.integer | other.integer)
    }
}

impl<E: Environment, I: IntegerType> BitOr<&Integer<E, I>> for Integer<E, I> {
    type Output = Self;

    /// Returns the bitwise `OR` of `self` and `other`.
    #[inline]
    fn bitor(self, other: &Integer<E, I>) -> Self::Output {
        Integer::new(self.integer | other.integer)
    }
}

impl<E: Environment, I: IntegerType> BitOrAssign for Integer<E, I> {
    /// Performs the bitwise `OR` of `self` and `other` and assigns the result to `self`.
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.integer = self.integer | other.integer;
    }
}

impl<E: Environment, I: IntegerType> BitXor for Integer<E, I> {
    type Output = Self;

    /// Returns the bitwise `XOR` of `self` and `other`.
    #[inline]
    fn bitxor(self, other: Self) -> Self::Output {
        Integer::new(self.integer ^ other.integer)
    }
}

impl<E: Environment, I: IntegerType> BitXor<&Integer<E, I>> for Integer<E, I> {
    type Output = Self;

    /// Returns the bitwise `XOR` of `self` and `other`.
    #[inline]
    fn bitxor(self, other: &Integer<E, I>) -> Self::Output {
        Integer::new(self.integer ^ other.integer)
    }
}

impl<E: Environment, I: IntegerType> BitXorAssign for Integer<E, I> {
    /// Performs the bitwise `XOR` of `self` and `other` and assigns the result to `self`.
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.integer = self.integer ^ other.integer;
    }
}

impl<E: Environment, I: IntegerType, M: Magnitude> Shl<Integer<E, M>> for Integer<E, I> {
    type Output = Self;

    /// Shifts `self` to the left by `n` bits.
    #[inline]
    fn shl(self, n: Integer<E, M>) -> Self::Output {
        // Unwrap is safe as we only cast up.
        match self.integer.checked_shl(&n.integer.to_u32().unwrap()) {
            Some(shifted) => Integer::new(shifted),
            None => E::halt(format!("Failed to shift {self} left by {n} bits")),
        }
    }
}

impl<E: Environment, I: IntegerType, M: Magnitude> Shl<&Integer<E, M>> for Integer<E, I> {
    type Output = Self;

    /// Shifts `self` to the left by `n` bits.
    #[inline]
    fn shl(self, n: &Integer<E, M>) -> Self::Output {
        // Unwrap is safe as we only cast up.
        match self.integer.checked_shl(&n.integer.to_u32().unwrap()) {
            Some(shifted) => Integer::new(shifted),
            None => E::halt(format!("Failed to shift {self} left by {n} bits")),
        }
    }
}

impl<E: Environment, I: IntegerType, M: Magnitude> ShlChecked<Integer<E, M>> for Integer<E, I> {
    type Output = Self;

    /// Shifts `self` to the left by `n` bits.
    #[inline]
    fn shl_checked(&self, n: &Integer<E, M>) -> Self::Output {
        // Unwrap is safe as we only cast up.
        match self.integer.checked_shl(&n.integer.to_u32().unwrap()) {
            Some(shifted) => Integer::new(shifted),
            None => E::halt(format!("Failed to shift {self} left by {n} bits")),
        }
    }
}

impl<E: Environment, I: IntegerType, M: Magnitude> ShlWrapped<Integer<E, M>> for Integer<E, I> {
    type Output = Self;

    /// Shifts `self` to the left by `n` bits, continuing past the boundary.
    #[inline]
    fn shl_wrapped(&self, n: &Integer<E, M>) -> Self::Output {
        Integer::new(self.integer.wrapping_shl(n.integer.to_u32().unwrap()))
    }
}

impl<E: Environment, I: IntegerType, M: Magnitude> ShlAssign<Integer<E, M>> for Integer<E, I> {
    /// Shifts `self` to the left by `n` bits and assigns the result to `self`.
    #[inline]
    fn shl_assign(&mut self, n: Integer<E, M>) {
        match self.integer.checked_shl(&n.integer.to_u32().unwrap()) {
            // Unwrap is safe as we only cast up.
            Some(shifted) => {
                self.integer = shifted;
            }
            None => E::halt(format!("Failed to shift {self} left by {n} bits")),
        }
    }
}

impl<E: Environment, I: IntegerType, M: Magnitude> Shr<Integer<E, M>> for Integer<E, I> {
    type Output = Self;

    /// Shifts `self` to the right by `n` bits.
    #[inline]
    fn shr(self, n: Integer<E, M>) -> Self::Output {
        match self.integer.checked_shr(n.integer.to_u32().unwrap()) {
            // Unwrap is safe as we only cast up.
            Some(shifted) => Integer::new(shifted),
            None => E::halt(format!("Failed to shift {self} right by {n} bits")),
        }
    }
}

impl<E: Environment, I: IntegerType, M: Magnitude> Shr<&Integer<E, M>> for Integer<E, I> {
    type Output = Self;

    /// Shifts `self` to the right by `n` bits.
    #[inline]
    fn shr(self, n: &Integer<E, M>) -> Self::Output {
        match self.integer.checked_shr(n.integer.to_u32().unwrap()) {
            // Unwrap is safe as we only cast up.
            Some(shifted) => Integer::new(shifted),
            None => E::halt(format!("Failed to shift {self} right by {n} bits")),
        }
    }
}

impl<E: Environment, I: IntegerType, M: Magnitude> ShrChecked<Integer<E, M>> for Integer<E, I> {
    type Output = Self;

    /// Shifts `self` to the right by `n` bits.
    #[inline]
    fn shr_checked(&self, n: &Integer<E, M>) -> Self::Output {
        match self.integer.checked_shr(n.integer.to_u32().unwrap()) {
            Some(shifted) => Integer::new(shifted),
            None => E::halt(format!("Failed to shift {self} right by {n} bits")),
        }
    }
}

impl<E: Environment, I: IntegerType, M: Magnitude> ShrWrapped<Integer<E, M>> for Integer<E, I> {
    type Output = Self;

    /// Shifts `self` to the right by `n` bits, continuing past the boundary.
    #[inline]
    fn shr_wrapped(&self, n: &Integer<E, M>) -> Self::Output {
        Integer::new(self.integer.wrapping_shr(n.integer.to_u32().unwrap()))
    }
}

impl<E: Environment, I: IntegerType, M: Magnitude> ShrAssign<Integer<E, M>> for Integer<E, I> {
    /// Shifts `self` to the right by `n` bits and assigns the result to `self`.
    #[inline]
    fn shr_assign(&mut self, n: Integer<E, M>) {
        match self.integer.checked_shr(n.integer.to_u32().unwrap()) {
            // Unwrap is safe as we only cast up.
            Some(shifted) => {
                self.integer = shifted;
            }
            None => E::halt(format!("Failed to shift {self} right by {n} bits")),
        }
    }
}

impl<E: Environment, I: IntegerType> Ternary for Integer<E, I> {
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
