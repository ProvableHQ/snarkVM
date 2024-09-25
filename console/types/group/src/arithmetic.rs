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

impl<E: Environment> Neg for Group<E> {
    type Output = Group<E>;

    /// Returns the `negation` of `self`.
    #[inline]
    fn neg(self) -> Self::Output {
        Group::from_projective(-self.group)
    }
}

impl<E: Environment> Add<Group<E>> for Group<E> {
    type Output = Group<E>;

    /// Returns the `sum` of `self` and `other`.
    #[inline]
    fn add(self, other: Group<E>) -> Self::Output {
        Group::from_projective(self.group + other.group)
    }
}

impl<E: Environment> Add<&Group<E>> for Group<E> {
    type Output = Group<E>;

    /// Returns the `sum` of `self` and `other`.
    #[inline]
    fn add(self, other: &Group<E>) -> Self::Output {
        Group::from_projective(self.group + other.group)
    }
}

impl<E: Environment> AddAssign<Group<E>> for Group<E> {
    /// Adds `other` to `self`.
    #[inline]
    fn add_assign(&mut self, other: Group<E>) {
        self.group += other.group;
    }
}

impl<E: Environment> AddAssign<&Group<E>> for Group<E> {
    /// Adds `other` to `self`.
    #[inline]
    fn add_assign(&mut self, other: &Group<E>) {
        self.group += other.group;
    }
}

impl<E: Environment> Sub<Group<E>> for Group<E> {
    type Output = Group<E>;

    /// Returns the `difference` of `self` and `other`.
    #[inline]
    fn sub(self, other: Group<E>) -> Self::Output {
        Group::from_projective(self.group - other.group)
    }
}

impl<E: Environment> Sub<&Group<E>> for Group<E> {
    type Output = Group<E>;

    /// Returns the `difference` of `self` and `other`.
    #[inline]
    fn sub(self, other: &Group<E>) -> Self::Output {
        Group::from_projective(self.group - other.group)
    }
}

impl<E: Environment> SubAssign<Group<E>> for Group<E> {
    /// Subtracts `other` from `self`.
    #[inline]
    fn sub_assign(&mut self, other: Group<E>) {
        self.group -= other.group;
    }
}

impl<E: Environment> SubAssign<&Group<E>> for Group<E> {
    /// Subtracts `other` from `self`.
    #[inline]
    fn sub_assign(&mut self, other: &Group<E>) {
        self.group -= other.group;
    }
}

impl<E: Environment> Mul<Scalar<E>> for Group<E> {
    type Output = Group<E>;

    /// Returns the `product` of `self` and `other`.
    #[inline]
    fn mul(self, other: Scalar<E>) -> Self::Output {
        Group::from_projective(self.group * *other)
    }
}

impl<E: Environment> Mul<&Scalar<E>> for Group<E> {
    type Output = Group<E>;

    /// Returns the `product` of `self` and `other`.
    #[inline]
    fn mul(self, other: &Scalar<E>) -> Self::Output {
        Group::from_projective(self.group * **other)
    }
}

impl<E: Environment> Mul<Group<E>> for Scalar<E> {
    type Output = Group<E>;

    /// Returns the `product` of `self` and `other`.
    #[inline]
    fn mul(self, other: Group<E>) -> Self::Output {
        Group::from_projective(other.group * *self)
    }
}

impl<E: Environment> Mul<&Group<E>> for Scalar<E> {
    type Output = Group<E>;

    /// Returns the `product` of `self` and `other`.
    #[inline]
    fn mul(self, other: &Group<E>) -> Self::Output {
        Group::from_projective(other.group * *self)
    }
}

impl<E: Environment> MulAssign<Scalar<E>> for Group<E> {
    /// Multiplies `self` by `other`.
    #[inline]
    fn mul_assign(&mut self, other: Scalar<E>) {
        self.group *= *other;
    }
}

impl<E: Environment> MulAssign<&Scalar<E>> for Group<E> {
    /// Multiplies `self` by `other`.
    #[inline]
    fn mul_assign(&mut self, other: &Scalar<E>) {
        self.group *= **other;
    }
}

impl<E: Environment> Double for Group<E> {
    type Output = Group<E>;

    /// Returns the `double` of `self`.
    #[inline]
    fn double(&self) -> Self::Output {
        Group::from_projective(self.group.double())
    }
}

impl<E: Environment> Sum<Group<E>> for Group<E> {
    /// Returns the `sum` of `self` and `other`.
    #[inline]
    fn sum<I: Iterator<Item = Group<E>>>(iter: I) -> Self {
        iter.fold(Group::zero(), |a, b| a + b)
    }
}

impl<'a, E: Environment> Sum<&'a Group<E>> for Group<E> {
    /// Returns the `sum` of `self` and `other`.
    #[inline]
    fn sum<I: Iterator<Item = &'a Group<E>>>(iter: I) -> Self {
        iter.fold(Group::zero(), |a, b| a + b)
    }
}
