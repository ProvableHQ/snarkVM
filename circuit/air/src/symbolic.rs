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

use crate::{Air, AirBuilder, SymbolicExpr, Window};
use snarkvm_fields::{PrimeField, Zero};

/// A copyable handle to a symbolic trace cell.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SymbolicVar {
    /// Main-trace cell.
    Main {
        /// Column index.
        column: usize,
        /// Whether the cell is on the next row.
        is_next: bool,
    },
    /// Preprocessed-trace cell.
    Preprocessed {
        /// Column index.
        column: usize,
        /// Whether the cell is on the next row.
        is_next: bool,
    },
}

impl<F: PrimeField> From<SymbolicVar> for SymbolicExpr<F> {
    fn from(var: SymbolicVar) -> Self {
        match var {
            SymbolicVar::Main { column, is_next: false } => Self::Local(column),
            SymbolicVar::Main { column, is_next: true } => Self::Next(column),
            SymbolicVar::Preprocessed { column, is_next: false } => Self::PreprocessedLocal(column),
            SymbolicVar::Preprocessed { column, is_next: true } => Self::PreprocessedNext(column),
        }
    }
}

/// Records constraint polynomials without a concrete trace.
pub struct SymbolicAirBuilder<F: PrimeField> {
    width: usize,
    prep_width: usize,
    constraints: Vec<SymbolicExpr<F>>,
}

impl<F: PrimeField> SymbolicAirBuilder<F> {
    /// Constructs a builder for an AIR of the given widths.
    pub fn new(width: usize, prep_width: usize) -> Self {
        Self { width, prep_width, constraints: Vec::new() }
    }

    /// Returns the recorded constraint polynomials.
    pub fn constraints(&self) -> &[SymbolicExpr<F>] {
        &self.constraints
    }

    /// Evaluates `air` and returns the recorded constraint polynomials.
    pub fn constraints_of<A: Air<Self>>(air: &A) -> Vec<SymbolicExpr<F>> {
        let mut builder = Self::new(air.width(), air.preprocessed_width());
        air.eval(&mut builder);
        builder.constraints
    }
}

impl<F: PrimeField> AirBuilder for SymbolicAirBuilder<F> {
    type Expr = SymbolicExpr<F>;
    type F = F;
    type Var = SymbolicVar;

    fn main(&self) -> Window<Self::Var> {
        Window {
            local: (0..self.width).map(|column| SymbolicVar::Main { column, is_next: false }).collect(),
            next: (0..self.width).map(|column| SymbolicVar::Main { column, is_next: true }).collect(),
        }
    }

    fn preprocessed(&self) -> Option<Window<Self::Var>> {
        if self.prep_width == 0 {
            None
        } else {
            Some(Window {
                local: (0..self.prep_width)
                    .map(|column| SymbolicVar::Preprocessed { column, is_next: false })
                    .collect(),
                next: (0..self.prep_width).map(|column| SymbolicVar::Preprocessed { column, is_next: true }).collect(),
            })
        }
    }

    fn is_first_row(&self) -> Self::Expr {
        SymbolicExpr::IsFirstRow
    }

    fn is_last_row(&self) -> Self::Expr {
        SymbolicExpr::IsLastRow
    }

    fn is_transition(&self) -> Self::Expr {
        SymbolicExpr::IsTransition
    }

    fn assert_zero<I: Into<Self::Expr>>(&mut self, x: I) {
        let expr = x.into();
        if !expr.is_zero() {
            self.constraints.push(expr);
        }
    }
}
