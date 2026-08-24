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

use crate::{Air, AirBuilder, Trace, Window};
use snarkvm_fields::PrimeField;

use anyhow::{Result, bail, ensure};

/// A row at which a constraint polynomial did not vanish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstraintFailure<F: PrimeField> {
    /// Row index in the main trace.
    pub row: usize,
    /// Non-zero evaluation of the constraint.
    pub value: F,
}

/// Evaluates an AIR on a concrete trace, collecting vanishing failures.
pub struct DebugAirBuilder<F: PrimeField> {
    local: Vec<F>,
    next: Vec<F>,
    prep_local: Vec<F>,
    prep_next: Vec<F>,
    is_first: bool,
    is_last: bool,
    is_transition: bool,
    row: usize,
    failures: Vec<ConstraintFailure<F>>,
}

impl<F: PrimeField> DebugAirBuilder<F> {
    fn new(trace: &Trace<F>, preprocessed: Option<&Trace<F>>, row: usize) -> Self {
        let height = trace.height();
        let is_last = row + 1 == height;
        let local = trace.row(row).to_vec();
        let next = if is_last { vec![F::zero(); trace.width()] } else { trace.row(row + 1).to_vec() };

        let (prep_local, prep_next) = match preprocessed {
            Some(prep) => {
                let prep_local = prep.row(row).to_vec();
                let prep_next = if is_last { vec![F::zero(); prep.width()] } else { prep.row(row + 1).to_vec() };
                (prep_local, prep_next)
            }
            None => (Vec::new(), Vec::new()),
        };

        Self {
            local,
            next,
            prep_local,
            prep_next,
            is_first: row == 0,
            is_last,
            is_transition: !is_last,
            row,
            failures: Vec::new(),
        }
    }
}

impl<F: PrimeField> AirBuilder for DebugAirBuilder<F> {
    type Expr = F;
    type F = F;
    type Var = F;

    fn main(&self) -> Window<Self::Var> {
        Window { local: self.local.clone(), next: self.next.clone() }
    }

    fn preprocessed(&self) -> Option<Window<Self::Var>> {
        if self.prep_local.is_empty() {
            None
        } else {
            Some(Window { local: self.prep_local.clone(), next: self.prep_next.clone() })
        }
    }

    fn is_first_row(&self) -> Self::Expr {
        if self.is_first { F::one() } else { F::zero() }
    }

    fn is_last_row(&self) -> Self::Expr {
        if self.is_last { F::one() } else { F::zero() }
    }

    fn is_transition(&self) -> Self::Expr {
        if self.is_transition { F::one() } else { F::zero() }
    }

    fn assert_zero<I: Into<Self::Expr>>(&mut self, x: I) {
        let value = x.into();
        if !value.is_zero() {
            self.failures.push(ConstraintFailure { row: self.row, value });
        }
    }
}

/// Checks that every AIR constraint vanishes on `trace`.
pub fn debug_constraints<F, A>(air: &A, trace: &Trace<F>) -> Result<()>
where
    F: PrimeField,
    A: Air<DebugAirBuilder<F>>,
{
    ensure!(trace.width() == air.width(), "Trace width {} does not match AIR width {}", trace.width(), air.width());
    ensure!(trace.height() > 0, "Trace height must be positive");

    let preprocessed = air.preprocessed_trace();
    if let Some(prep) = preprocessed.as_ref() {
        ensure!(
            prep.height() == trace.height(),
            "Preprocessed height {} does not match main-trace height {}",
            prep.height(),
            trace.height()
        );
    }

    let mut failures = Vec::new();
    for row in 0..trace.height() {
        let mut builder = DebugAirBuilder::new(trace, preprocessed.as_ref(), row);
        air.eval(&mut builder);
        failures.append(&mut builder.failures);
    }

    if failures.is_empty() {
        Ok(())
    } else {
        bail!(
            "AIR constraints failed on {} row(s); first failure at row {} with value {}",
            failures.len(),
            failures[0].row,
            failures[0].value
        )
    }
}
