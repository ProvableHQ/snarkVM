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

use snarkvm_fields::{One, PrimeField, Zero};

use core::{
    fmt,
    ops::{Add, Mul, Neg, Sub},
};

/// A symbolic polynomial over local/next main and preprocessed cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolicExpr<F: PrimeField> {
    /// A field constant.
    Constant(F),
    /// Main-trace cell on the current row.
    Local(usize),
    /// Main-trace cell on the next row.
    Next(usize),
    /// Preprocessed cell on the current row.
    PreprocessedLocal(usize),
    /// Preprocessed cell on the next row.
    PreprocessedNext(usize),
    /// Selector that is `1` on the first row.
    IsFirstRow,
    /// Selector that is `1` on the last row.
    IsLastRow,
    /// Selector that is `1` on every row except the last.
    IsTransition,
    /// Sum of two expressions.
    Add(Box<Self>, Box<Self>),
    /// Difference of two expressions.
    Sub(Box<Self>, Box<Self>),
    /// Product of two expressions.
    Mul(Box<Self>, Box<Self>),
    /// Additive inverse.
    Neg(Box<Self>),
}

impl<F: PrimeField> SymbolicExpr<F> {
    /// Returns a constant expression.
    pub fn constant(value: F) -> Self {
        Self::Constant(value)
    }
}

impl<F: PrimeField> From<F> for SymbolicExpr<F> {
    fn from(value: F) -> Self {
        Self::Constant(value)
    }
}

impl<F: PrimeField> One for SymbolicExpr<F> {
    fn one() -> Self {
        Self::Constant(F::one())
    }

    fn is_one(&self) -> bool {
        matches!(self, Self::Constant(value) if value.is_one())
    }
}

impl<F: PrimeField> Zero for SymbolicExpr<F> {
    fn zero() -> Self {
        Self::Constant(F::zero())
    }

    fn is_zero(&self) -> bool {
        matches!(self, Self::Constant(value) if value.is_zero())
    }
}

impl<F: PrimeField> Add for SymbolicExpr<F> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Constant(a), Self::Constant(b)) => Self::Constant(a + b),
            (Self::Constant(a), b) if a.is_zero() => b,
            (a, Self::Constant(b)) if b.is_zero() => a,
            (a, b) => Self::Add(Box::new(a), Box::new(b)),
        }
    }
}

impl<F: PrimeField> Sub for SymbolicExpr<F> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Constant(a), Self::Constant(b)) => Self::Constant(a - b),
            (a, Self::Constant(b)) if b.is_zero() => a,
            (a, b) => Self::Sub(Box::new(a), Box::new(b)),
        }
    }
}

impl<F: PrimeField> Mul for SymbolicExpr<F> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Constant(a), Self::Constant(b)) => Self::Constant(a * b),
            (Self::Constant(a), _) if a.is_zero() => Self::zero(),
            (_, Self::Constant(b)) if b.is_zero() => Self::zero(),
            (Self::Constant(a), b) if a.is_one() => b,
            (a, Self::Constant(b)) if b.is_one() => a,
            (a, b) => Self::Mul(Box::new(a), Box::new(b)),
        }
    }
}

impl<F: PrimeField> Neg for SymbolicExpr<F> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Self::Constant(a) => Self::Constant(-a),
            Self::Neg(inner) => *inner,
            other => Self::Neg(Box::new(other)),
        }
    }
}

impl<F: PrimeField> fmt::Display for SymbolicExpr<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Constant(value) => write!(f, "{value}"),
            Self::Local(column) => write!(f, "local[{column}]"),
            Self::Next(column) => write!(f, "next[{column}]"),
            Self::PreprocessedLocal(column) => write!(f, "prep[{column}]"),
            Self::PreprocessedNext(column) => write!(f, "prep_next[{column}]"),
            Self::IsFirstRow => write!(f, "is_first_row"),
            Self::IsLastRow => write!(f, "is_last_row"),
            Self::IsTransition => write!(f, "is_transition"),
            Self::Add(lhs, rhs) => write!(f, "({lhs} + {rhs})"),
            Self::Sub(lhs, rhs) => write!(f, "({lhs} - {rhs})"),
            Self::Mul(lhs, rhs) => write!(f, "({lhs} * {rhs})"),
            Self::Neg(inner) => write!(f, "(-{inner})"),
        }
    }
}

/// Raises `base` to `exponent` by square-and-multiply.
pub fn exp_u64<E>(base: E, exponent: u64) -> E
where
    E: Clone + One + Mul<Output = E>,
{
    if exponent == 0 {
        return E::one();
    }

    let mut acc = E::one();
    let mut base = base;
    let mut exp = exponent;
    while exp > 0 {
        if exp & 1 == 1 {
            acc = acc * base.clone();
        }
        exp >>= 1;
        if exp > 0 {
            base = base.clone() * base;
        }
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_curves::bls12_377::Fr;
    use snarkvm_fields::{Field, One, Zero};

    #[test]
    fn test_exp_u64_matches_field_pow() {
        let base = Fr::from(3u64);
        assert_eq!(exp_u64(base, 0), Fr::one());
        assert_eq!(exp_u64(base, 1), base);
        assert_eq!(exp_u64(base, 5), base.pow([5]));
        assert_eq!(exp_u64(base, 17), base.pow([17]));
    }

    #[test]
    fn test_symbolic_expr_constant_folding() {
        let a = SymbolicExpr::<Fr>::from(Fr::from(2u64));
        let b = SymbolicExpr::<Fr>::from(Fr::from(3u64));
        assert_eq!(a.clone() + b.clone(), SymbolicExpr::from(Fr::from(5u64)));
        assert_eq!(a.clone() * b, SymbolicExpr::from(Fr::from(6u64)));
        assert_eq!(a.clone() * SymbolicExpr::zero(), SymbolicExpr::zero());
        assert_eq!(-(-a.clone()), a);
    }
}
