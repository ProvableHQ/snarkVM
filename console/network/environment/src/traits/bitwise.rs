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

/// Trait for equality comparisons.
pub trait Equal<Rhs: ?Sized = Self> {
    type Output;

    /// Returns `true` if `self` and `other` are equal.
    fn is_equal(&self, other: &Rhs) -> Self::Output;

    /// Returns `true` if `self` and `other` are *not* equal.
    fn is_not_equal(&self, other: &Rhs) -> Self::Output;
}

/// Trait for comparator operations.
pub trait Compare<Rhs: ?Sized = Self> {
    type Output;

    /// Returns `true` if `self` is less than `other`.
    fn is_less_than(&self, other: &Rhs) -> Self::Output;

    /// Returns `true` if `self` is greater than `other`.
    fn is_greater_than(&self, other: &Rhs) -> Self::Output;

    /// Returns `true` if `self` is less than or equal to `other`.
    fn is_less_than_or_equal(&self, other: &Rhs) -> Self::Output;

    /// Returns `true` if `self` is greater than or equal to `other`.
    fn is_greater_than_or_equal(&self, other: &Rhs) -> Self::Output;
}

/// Binary operator for performing `NOT (a AND b)`.
pub trait Nand<Rhs: ?Sized = Self> {
    type Output;

    /// Returns `NOT (a AND b)`.
    fn nand(&self, other: &Rhs) -> Self::Output;
}

/// Binary operator for performing `(NOT a) AND (NOT b)`.
pub trait Nor<Rhs: ?Sized = Self> {
    type Output;

    /// Returns `(NOT a) AND (NOT b)`.
    fn nor(&self, other: &Rhs) -> Self::Output;
}

/// Trait for ternary operations.
pub trait Ternary {
    type Boolean;
    type Output;

    /// Returns `first` if `condition` is `true`, otherwise returns `second`.
    fn ternary(condition: &Self::Boolean, first: &Self, second: &Self) -> Self::Output
    where
        Self: Sized;
}

impl<T: Ternary> Ternary for Box<T> {
    type Boolean = T::Boolean;
    type Output = Box<T::Output>;

    /// Returns `first` if `condition` is `true`, otherwise returns `second`.
    fn ternary(condition: &Self::Boolean, first: &Self, second: &Self) -> Self::Output {
        Box::new(T::ternary(condition, first, second))
    }
}
