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

use crate::{EvalError, ExecError};
use snarkvm_circuit_environment::ConstraintUnsatisfied;
use thiserror::Error;

// NOTE: Many errors in this module temporarily contain `Anyhow` variants.
// Remove these variants as we migrate errors to thiserror.

/// Errors that may occur during process authorization.
#[derive(Debug, Error)]
pub enum ProcessAuthError {
    /// Stack authorization failed.
    #[error("Stack authorization failed: {0}")]
    StackAuth(#[from] StackAuthError),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

/// Errors that may occur during process evaluation.
#[derive(Debug, Error)]
pub enum ProcessEvalError {
    /// Stack evaluation failed.
    #[error("Stack evaluation failed: {0}")]
    StackEval(#[from] StackEvalError),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

/// Errors that may occur during process execution.
#[derive(Debug, Error)]
pub enum ProcessExecError {
    /// Stack execution failed.
    #[error("Stack execution failed: {0}")]
    StackExec(#[from] StackExecError),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

/// Errors that may occur during process deployment.
#[derive(Debug, Error)]
pub enum ProcessDeployError {
    /// Stack execution failed during synthesis.
    #[error("Stack synthesis failed: {0}")]
    StackExec(#[from] StackExecError),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

/// Errors that may occur during call evaluation.
#[derive(Debug, Error)]
pub enum CallEvalError {
    /// An error occurred during substack evaluation.
    #[error("Substack evaluation failed: {0}")]
    StackEval(#[from] StackEvalError),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

/// Errors that may occur during call execution.
#[derive(Debug, Error)]
pub enum CallExecError {
    /// An error occurred during substack execution.
    #[error("Substack execution failed: {0}")]
    StackExec(#[from] StackExecError),
    /// An error occurred during substack evaluation.
    #[error("Substack evaluation failed: {0}")]
    StackEval(#[from] StackEvalError),
    /// A circuit constraint was not satisfied.
    #[error(transparent)]
    Constraint(#[from] ConstraintUnsatisfied),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

/// Errors that may occur during stack authorization.
#[derive(Debug, Error)]
pub enum StackAuthError {
    /// Stack execution failed.
    #[error("Stack execution failed: {0}")]
    Exec(#[from] StackExecError),
    /// Stack evaluation failed.
    #[error("Stack evaluation failed: {0}")]
    Eval(#[from] StackEvalError),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

/// Errors that may occur during stack execution.
#[derive(Debug, Error)]
pub enum StackExecError {
    /// Instruction at the given index failed.
    #[error(transparent)]
    Instruction(#[from] IndexedInstructionError<InstructionError>),
    /// A circuit constraint was not satisfied.
    #[error(transparent)]
    Constraint(#[from] ConstraintUnsatisfied),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

/// Errors that may occur during stack evaluation.
#[derive(Debug, Error)]
pub enum StackEvalError {
    /// Instruction at the given index failed.
    #[error(transparent)]
    Instruction(#[from] IndexedInstructionError<InstructionEvalError>),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

/// An instruction error occurred at a particular index.
#[derive(Debug, Error)]
#[error("Instruction ({instruction}) at index {index} failed: {error}")]
pub struct IndexedInstructionError<E> {
    /// The index of the failing instruction.
    pub index: usize,
    /// The failing instruction formatted.
    pub instruction: String,
    /// The instruction error.
    pub error: E,
}

/// An error occurred during the execution/evaluation/synthesis of an
/// instruction.
#[derive(Debug, Error)]
pub enum InstructionError {
    /// Failed to evaluate an instruction.
    #[error("Failed to evaluate: {0}")]
    Eval(#[from] InstructionEvalError),
    /// Failed to execute an instruction.
    #[error("Failed to execute: {0}")]
    Exec(#[from] InstructionExecError),
}

/// An error occurred during the evaluation of an instruction.
#[derive(Debug, Error)]
pub enum InstructionEvalError {
    /// An instruction evaluation failed.
    #[error(transparent)]
    Eval(#[from] EvalError),
    /// An error occurred during a `Call` instruction.
    #[error("Call failed: {0}")]
    Call(#[from] Box<CallEvalError>),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

/// An error occurred during the execution of an instruction.
#[derive(Debug, Error)]
pub enum InstructionExecError {
    /// An error occurred during a `Call` instruction.
    #[error("Call failed: {0}")]
    Call(#[from] Box<CallExecError>),
    /// An instruction execution error.
    #[error(transparent)]
    Exec(#[from] ExecError),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl<E> IndexedInstructionError<E> {
    /// Short-hand constructor for the `IndexedInstructionError` type.
    pub fn new(index: usize, instruction: String, error: E) -> Self {
        Self { index, instruction, error }
    }
}
