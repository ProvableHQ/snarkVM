// Copyright (c) 2019-2025 Provable Inc.
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

//! Errors for instruction operations.

use circuit::environment::ConstraintUnsatisfied;
use console::network::prelude::Error as AnyhowError;
use thiserror::Error;

// NOTE: Many errors in this module temporarily contain `Anyhow` variants.
// TODO: Remove these variants as we migrate errors to thiserror.

/// An error occurred during instruction evaluation.
#[derive(Debug, Error)]
pub enum EvalError {
    /// An assertion instruction failed.
    #[error(transparent)]
    Assert(#[from] AssertError),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] AnyhowError),
}

/// An error occurred during instruction finalization.
#[derive(Debug, Error)]
pub enum FinalizeError {
    /// An evaluation error occurred during finalization.
    #[error(transparent)]
    Eval(#[from] EvalError),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] AnyhowError),
}

/// An error occurred during instruction execution.
#[derive(Debug, Error)]
pub enum ExecError {
    /// A circuit constraint was unsatisfied during execution.
    #[error(transparent)]
    Constraint(#[from] ConstraintUnsatisfied),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] AnyhowError),
}

/// An error occurred during an assert instruction.
#[derive(Debug, Error)]
pub enum AssertError {
    /// The assert.eq instruction failed because the operands are not equal.
    #[error("'assert.eq' failed: '{lhs}' is not equal to '{rhs}' (should be equal)")]
    Eq {
        /// The left-hand side operand.
        lhs: String,
        /// The right-hand side operand.
        rhs: String,
    },
    /// The assert.neq instruction failed because the operands are equal.
    #[error("'assert.neq' failed: '{lhs}' is equal to '{rhs}' (should not be equal)")]
    Neq {
        /// The left-hand side operand.
        lhs: String,
        /// The right-hand side operand.
        rhs: String,
    },
    /// An invalid assert variant was specified.
    #[error("Invalid 'assert' variant: {variant}")]
    Invalid {
        /// The invalid variant.
        variant: u8,
    },
}
