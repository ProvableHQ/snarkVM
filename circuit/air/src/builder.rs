// Copyright (c) 2019-2026 Provable Inc.
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

use crate::Window;
use snarkvm_fields::{One, PrimeField, Zero};

use core::{
    fmt::Debug,
    ops::{Add, Mul, Neg, Sub},
};

/// A Plonky3-style constraint builder over a local/next row window.
///
/// Implementors evaluate constraints either on a concrete trace
/// ([`crate::DebugAirBuilder`]) or symbolically ([`crate::SymbolicAirBuilder`]).
pub trait AirBuilder: Sized {
    /// Base field of the AIR.
    type F: PrimeField;
    /// Constraint polynomial type.
    type Expr: Clone
        + Debug
        + One
        + Zero
        + From<Self::F>
        + From<Self::Var>
        + Add<Output = Self::Expr>
        + Sub<Output = Self::Expr>
        + Mul<Output = Self::Expr>
        + Neg<Output = Self::Expr>;
    /// A copyable reference to a trace cell.
    type Var: Copy + Clone + Debug + Send + Into<Self::Expr>;

    /// Returns the main-trace local/next window.
    fn main(&self) -> Window<Self::Var>;

    /// Returns the preprocessed local/next window, if present.
    fn preprocessed(&self) -> Option<Window<Self::Var>> {
        None
    }

    /// Returns `1` on the first row and `0` otherwise.
    fn is_first_row(&self) -> Self::Expr;

    /// Returns `1` on the last row and `0` otherwise.
    fn is_last_row(&self) -> Self::Expr;

    /// Returns `1` on every row except the last (where `next` is undefined).
    fn is_transition(&self) -> Self::Expr;

    /// Asserts that `x` vanishes.
    fn assert_zero<I: Into<Self::Expr>>(&mut self, x: I);

    /// Asserts that `x` equals `y`.
    fn assert_eq<L, R>(&mut self, x: L, y: R)
    where
        L: Into<Self::Expr>,
        R: Into<Self::Expr>,
    {
        self.assert_zero(x.into() - y.into());
    }

    /// Asserts that `x` is boolean, i.e. `x * (x - 1) = 0`.
    fn assert_bool<I: Into<Self::Expr>>(&mut self, x: I) {
        let x = x.into();
        self.assert_zero(x.clone() * (x - Self::Expr::one()));
    }

    /// Returns a builder that multiplies every assertion by `condition`.
    #[must_use]
    fn when<I: Into<Self::Expr>>(&mut self, condition: I) -> FilteredAirBuilder<'_, Self> {
        FilteredAirBuilder { inner: self, condition: condition.into() }
    }

    /// Constrains only the first row.
    #[must_use]
    fn when_first_row(&mut self) -> FilteredAirBuilder<'_, Self> {
        let condition = self.is_first_row();
        self.when(condition)
    }

    /// Constrains only the last row.
    #[must_use]
    fn when_last_row(&mut self) -> FilteredAirBuilder<'_, Self> {
        let condition = self.is_last_row();
        self.when(condition)
    }

    /// Constrains every row except the last.
    #[must_use]
    fn when_transition(&mut self) -> FilteredAirBuilder<'_, Self> {
        let condition = self.is_transition();
        self.when(condition)
    }
}

/// An [`AirBuilder`] that multiplies each assertion by a selector polynomial.
#[derive(Debug)]
pub struct FilteredAirBuilder<'a, AB: AirBuilder> {
    inner: &'a mut AB,
    condition: AB::Expr,
}

impl<'a, AB: AirBuilder> FilteredAirBuilder<'a, AB> {
    /// Asserts that `condition * x` vanishes.
    pub fn assert_zero<I: Into<AB::Expr>>(&mut self, x: I) {
        self.inner.assert_zero(self.condition.clone() * x.into());
    }

    /// Asserts that `condition * (x - y)` vanishes.
    pub fn assert_eq<L, R>(&mut self, x: L, y: R)
    where
        L: Into<AB::Expr>,
        R: Into<AB::Expr>,
    {
        self.assert_zero(x.into() - y.into());
    }

    /// Asserts that `x` is boolean on rows where the selector is set.
    pub fn assert_bool<I: Into<AB::Expr>>(&mut self, x: I) {
        let x = x.into();
        self.assert_zero(x.clone() * (x - AB::Expr::one()));
    }

    /// Further restricts assertions by the conjunction of selectors.
    #[must_use]
    pub fn when<I: Into<AB::Expr>>(self, condition: I) -> FilteredAirBuilder<'a, AB> {
        FilteredAirBuilder { inner: self.inner, condition: self.condition * condition.into() }
    }
}
