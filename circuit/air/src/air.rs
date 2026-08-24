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

use crate::{AirBuilder, Trace};
use snarkvm_fields::PrimeField;

/// Metadata for an AIR: main-trace width and optional preprocessed columns.
///
/// Preprocessed traces are circuit-specific constants (round keys, selectors)
/// and have the same height as the main trace.
pub trait BaseAir<F: PrimeField> {
    /// Returns the number of main-trace columns.
    fn width(&self) -> usize;

    /// Returns the number of preprocessed columns, or `0` if none are used.
    fn preprocessed_width(&self) -> usize {
        0
    }

    /// Returns the preprocessed trace, if this AIR uses fixed columns.
    fn preprocessed_trace(&self) -> Option<Trace<F>> {
        None
    }
}

/// A Plonky3-style AIR: `eval` emits polynomials in the local and next rows.
///
/// The same polynomials are applied on every row. Row-dependent data belongs
/// in the main trace or the preprocessed trace, not in control flow inside
/// `eval`.
pub trait Air<AB: AirBuilder>: BaseAir<AB::F> {
    /// Evaluate the constraint polynomials, asserting they vanish on the trace.
    fn eval(&self, builder: &mut AB);
}
