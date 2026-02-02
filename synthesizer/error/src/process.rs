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

use serde::{Deserialize, Serialize};
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
    /// An error occurred during stack creation.
    #[error("Stack creation failed: {0}")]
    StackInit(#[from] StackInitError),
    /// A temporary variant for type-erased anyhow errors.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

/// Errors that may occur during process finalization.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum ProcessFinalizeError {
    /// An error occurred during stack creation.
    #[error("Stack creation failed: {0}")]
    StackInit(#[from] StackInitError),
}

/// Errors that may occur during generic process use.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum ProcessGenericError {
    /// The queried program was not found.
    #[error("Program '{0}' does not exist")]
    MissingProgram(String),
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

/// Errors that may occur during stack creation.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum StackInitError {
    /// A closure already exists in the process.
    #[error("Closure '{0}' already exists")]
    ClosureAlreadyExists(String),
    /// Attempted to create a stack for credits.aleo.
    #[error("Cannot re-initialize 'credits.aleo'")]
    CreditsReinitialization,
    /// Attempted to upgrade credits.aleo.
    #[error("Cannot upgrade 'credits.aleo'")]
    CreditsUpgrade,
    /// The program with the given ID exists, but is not a match.
    #[error("Program '{0}' already exists with different contents.")]
    DifferentProgramAlreadyExists(String),
    /// A function already exists in the process.
    #[error("Function '{0}' already exists")]
    FunctionAlreadyExists(String),
    /// A program attempted to use a dependency that hasn't been imported.
    #[error("Cannot add program, because its import '{0}' must be added first")]
    MissingImport(String),
    /// A generic Process error was encountered.
    #[error("Generic process error: {0}")]
    ProcessGeneric(#[from] ProcessGenericError),
    /// The value of the program's edition became too large.
    #[error("Overflow while incrementing the program edition")]
    ProgramEditionOverflow,
    /// The program's ID couldn't be converted to an Address.
    #[error("Program ID can't be converted to an Address")]
    ProgramIdConversion,
    /// The program was not well-formed.
    #[error("Program is not well-formed")]
    ProgramMalformed,
    /// The program had no functions.
    #[error("No functions present in the deployment for program '{0}'")]
    ProgramMissingFunctions(String),
    /// A program attempted to import itself.
    #[error("Program cannot import itself")]
    SelfImport,
    /// A constructor contained an `await` command.
    #[error("`await` commands are not allowed in constructors.")]
    TypesConstructorWithAwait,
    /// A constructor contained a `call` command.
    #[error("`call` commands are not allowed in constructors.")]
    TypesConstructorWithCall,
    /// A constructor contained a `cast` command.
    #[error("`cast` (to record) commands are not allowed in constructors.")]
    TypesConstructorWithCast,
    /// Attempted to upgrade a program using incompatible mappings.
    #[error("Cannot upgrade '{0}' because the closure '{1}' does not match")]
    UpgradeClosureMismatch(String, String),
    /// Attempted to upgrade a program lacking an expected closure.
    #[error("Cannot upgrade '{0}' because the closure '{1}' is missing")]
    UpgradeClosureMissing(String, String),
    /// Attempted to upgrade a program to one with a different constructor.
    #[error("Cannot upgrade '{0}' because the constructor does not match")]
    UpgradeConstructorMismatch(String),
    /// Attempted to upgrade a program to one without a constructor.
    #[error("A program cannot be upgraded to a program without a constructor")]
    UpgradeConstructorMissingFinal,
    /// Attempted to upgrade a program without a constructor.
    #[error("A program without a constructor cannot be upgraded")]
    UpgradeConstructorMissingOriginal,
    /// Attempted to upgrade a program using a different ID.
    #[error("Cannot upgrade '{0}' with different program ID")]
    UpgradeDifferentProgramId(String),
    /// Attempted to upgrade a program containing a function without an expected finalize block.
    #[error("Cannot upgrade '{0}' because the function '{0}' should have a finalize block")]
    UpgradeFunctionFinalizeBlockExpected(String, String),
    /// Attempted to upgrade a program containing a function with an unexpected finalize block.
    #[error("Cannot upgrade '{0}' because the function '{0}' should not have a finalize block")]
    UpgradeFunctionFinalizeBlockUnexpected(String, String),
    /// Attempted to upgrade a program with incompatible finalize inputs.
    #[error("Cannot upgrade '{0}' because the finalize inputs to the function '{1}' do not match")]
    UpgradeFunctionFinalizeInputMismatch(String, String),
    /// Attempted to upgrade a program using incompatible function inputs.
    #[error("Cannot upgrade '{0}' because the input types to the function '{1}' do not match")]
    UpgradeFunctionInputMismatch(String, String),
    /// Attempted to upgrade a program lacking an expected function.
    #[error("Cannot upgrade '{0}' because the function '{1}' is missing")]
    UpgradeFunctionMissing(String, String),
    /// Attempted to upgrade a program using incompatible function outputs.
    #[error("Cannot upgrade '{0}' because the output types to the function '{1}' do not match")]
    UpgradeFunctionOutputMismatch(String, String),
    /// Attempted to upgrade a program using incompatible mappings.
    #[error("Cannot upgrade '{0}' because the mapping '{1}' does not match")]
    UpgradeMappingMismatch(String, String),
    /// Attempted to upgrade a program lacking an expected mapping.
    #[error("Cannot upgrade '{0}' because the mapping '{1}' is missing")]
    UpgradeMappingMissing(String, String),
    /// Attempted to upgrade a program using incompatible records.
    #[error("Cannot upgrade '{0}' because the record '{1}' does not match")]
    UpgradeRecordMismatch(String, String),
    /// Attempted to upgrade a program lacking an expected record.
    #[error("Cannot upgrade '{0}' because the record '{1}' is missing")]
    UpgradeRecordMissing(String, String),
    /// Attempted to upgrade a program using incompatible structs.
    #[error("Cannot upgrade '{0}' because the struct '{1}' does not match")]
    UpgradeStructMismatch(String, String),
    /// Attempted to upgrade a program lacking an expected struct.
    #[error("Cannot upgrade '{0}' because the struct '{1}' is missing")]
    UpgradeStructMissing(String, String),
    /// Attempted to upgrade a program using a different ID.
    #[error("Cannot upgrade '{0}' because it is missing the original import '{1}'")]
    UpgradeMissingOriginalImport(String, String),
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
