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
pub enum ProcessError {
    /// The given program ID is invalid.
    #[error("Invalid program ID")]
    InvalidProgramId,
    /// The queried program was not found.
    #[error("Program '{0}' does not exist")]
    MissingProgram(String),
    /// The given program ID doesn't match the expected one.
    #[error("Expected program '{0}', found '{1}'")]
    ProgramIdMismatch(String, String),
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

/// Errors that may occur during generic program use.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum ProgramError {
    /// Attempted to use an incompatible mapping.
    #[error("Expected mapping '{0}', but found mapping '{1}'")]
    MappingMismatch(String, String),
    /// Attempted to use a non-existent mapping.
    #[error("Mapping '{0}' is not defined.")]
    MappingMissing(String),
}

/// Errors that may occur during program upgrade.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum ProgramUpgradeError {
    /// Attempted to upgrade credits.aleo.
    #[error("Cannot upgrade 'credits.aleo'")]
    CreditsUpgrade,
    /// Attempted to upgrade a program using incompatible mappings.
    #[error("Cannot upgrade '{0}' because the closure '{1}' does not match")]
    ClosureMismatch(String, String),
    /// Attempted to upgrade a program lacking an expected closure.
    #[error("Cannot upgrade '{0}' because the closure '{1}' is missing")]
    ClosureMissing(String, String),
    /// Attempted to upgrade a program to one with a different constructor.
    #[error("Cannot upgrade '{0}' because the constructor does not match")]
    ConstructorMismatch(String),
    /// Attempted to upgrade a program to one without a constructor.
    #[error("A program cannot be upgraded to a program without a constructor")]
    ConstructorMissingFinal,
    /// Attempted to upgrade a program without a constructor.
    #[error("A program without a constructor cannot be upgraded")]
    ConstructorMissingOriginal,
    /// Attempted to upgrade a program using a different ID.
    #[error("Cannot upgrade '{0}' with different program ID")]
    DifferentProgramId(String),
    /// Attempted to upgrade a program containing a function without an expected finalize block.
    #[error("Cannot upgrade '{0}' because the function '{0}' should have a finalize block")]
    FunctionFinalizeBlockExpected(String, String),
    /// Attempted to upgrade a program containing a function with an unexpected finalize block.
    #[error("Cannot upgrade '{0}' because the function '{0}' should not have a finalize block")]
    FunctionFinalizeBlockUnexpected(String, String),
    /// Attempted to upgrade a program with incompatible finalize inputs.
    #[error("Cannot upgrade '{0}' because the finalize inputs to the function '{1}' do not match")]
    FunctionFinalizeInputMismatch(String, String),
    /// Attempted to upgrade a program using incompatible function inputs.
    #[error("Cannot upgrade '{0}' because the input types to the function '{1}' do not match")]
    FunctionInputMismatch(String, String),
    /// Attempted to upgrade a program lacking an expected function.
    #[error("Cannot upgrade '{0}' because the function '{1}' is missing")]
    FunctionMissing(String, String),
    /// Attempted to upgrade a program using incompatible function outputs.
    #[error("Cannot upgrade '{0}' because the output types to the function '{1}' do not match")]
    FunctionOutputMismatch(String, String),
    /// Attempted to upgrade a program using incompatible mappings.
    #[error("Cannot upgrade '{0}' because the mapping '{1}' does not match")]
    MappingMismatch(String, String),
    /// Attempted to upgrade a program lacking an expected mapping.
    #[error("Cannot upgrade '{0}' because the mapping '{1}' is missing")]
    MappingMissing(String, String),
    /// Attempted to upgrade a program using incompatible records.
    #[error("Cannot upgrade '{0}' because the record '{1}' does not match")]
    RecordMismatch(String, String),
    /// Attempted to upgrade a program lacking an expected record.
    #[error("Cannot upgrade '{0}' because the record '{1}' is missing")]
    RecordMissing(String, String),
    /// Attempted to upgrade a program using incompatible structs.
    #[error("Cannot upgrade '{0}' because the struct '{1}' does not match")]
    StructMismatch(String, String),
    /// Attempted to upgrade a program lacking an expected struct.
    #[error("Cannot upgrade '{0}' because the struct '{1}' is missing")]
    StructMissing(String, String),
    /// Attempted to upgrade a program using a different ID.
    #[error("Cannot upgrade '{0}' because it is missing the original import '{1}'")]
    MissingOriginalImport(String, String),
}

/// Errors that may occur during the initialization of finalize types.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum RegisterTypesInitError {
    /// The register already exists.
    #[error("Register '{0}' already exists")]
    AlreadyExists(String),
    /// The register type was incompatible.
    #[error("Input '{0}' does not match the expected input register type.")]
    IncompatibleInputType(String),
    /// Input registers were added after destination registers.
    #[error("Cannot add input registers after destination registers.")]
    InvalidAddOrder,
    /// The register references no accesses.
    #[error("Register '{0}' references no accesses")]
    MissingAccesses(String),
    /// The register doesn't exist.
    #[error("Register '{0}' does not exist")]
    MissingRegister(String),
    /// The register wasn't a locator when expected.
    #[error("Register '{0}' must be a locator.")]
    NotALocator(String),
    /// The registers weren't increasing monotonically.
    #[error("Register '{0}' is out of order")]
    OutOfOrder(String),
    /// The given struct is undefined.
    #[error("Struct '{0}' in '{1}' is not defined.")]
    StructUndefined(String, String),
}

/// Errors that may occur during the initialization of finalize types.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum FinalizeTypesInitError {
    /// The await register was not a locator.
    #[error("The await register '{0}' must be a locator")]
    AwaitRegisterInvalid(String),
    /// The await register type was not a future.
    #[error("The await register '{0}' must be a future")]
    AwaitRegisterTypeInvalid(String),
    /// A command error was encountered.
    #[error("Command error: {0}")]
    Command(#[from] CommandError),
    /// A constructor contained an `await` command.
    #[error("`await` commands are not allowed in constructors")]
    ConstructorWithAwait,
    /// A constructor contained a `call` command.
    #[error("`call` commands are not allowed in constructors")]
    ConstructorWithCall,
    /// A constructor contained a `cast` command.
    #[error("`cast` (to record) commands are not allowed in constructors")]
    ConstructorWithCast,
    /// An instruction checking error was encountered.
    #[error("Instruction check error: {0}")]
    InstructionCheck(#[from] InstructionCheckError),
    /// A locator references the current program.
    #[error("Locator '{0}' does not reference an external mapping.")]
    LocatorInternal(String),
    /// The given mapping is undefined.
    #[error("Mapping '{0}' in '{1}' is not defined.")]
    MappingUndefined(String, String),
    /// Not all the futures are awaited.
    #[error("Futures in finalize '{0}' are not all awaited.")]
    MissingAwait(String),
    /// Attempted to use an external dependency that hasn't been imported.
    #[error("External program '{0}' is not imported by '{1}'")]
    MissingExternalImport(String, String),
    /// Attempted to use a dependency that hasn't been imported.
    #[error("Program '{0}' is not imported by '{1}'")]
    MissingImport(String, String),
    /// Encountered a program error.
    #[error("Program error: {0}")]
    Program(#[from] ProgramError),
    /// Register types initialization failed.
    #[error("Initialization of register types failed: {0}")]
    RegisterTypesInit(#[from] RegisterTypesInitError),
}

/// Errors related to command checks.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum CommandError {
    /// A branch contains an undefined position.
    #[error("Command '{0}' expects a defined position to jump to. Found undefined position '{1}'")]
    BranchUndefinedPosition(String, String),
    /// A future was used as a default value.
    #[error("A default value cannot be a future")]
    DefaultValueFuture,
    /// The destination wasn't a locator when expected.
    #[error("Destination '{0}' must be a locator.")]
    DestinationNotALocator(String),
    /// The destination type wasn't a plaintext when expected.
    #[error("Destination '{0}' must be a plaintext type.")]
    DestinationNotPlaintext(String),
    /// The command's operands have incompatible types.
    #[error("Command '{0}' expects operands of the same type. Found operands of type '{1}' and '{2}'")]
    IncompatibleTypes(String, String, String),
    /// A future was used in an incompatible command.
    #[error("A future cannot be used in a `{0}` command")]
    IncompatibleWithFuture(String),
    /// The chosen destination type is not allowed.
    #[error("Destination type '{0}' is not allowed.")]
    InvalidDestinationType(String),
    /// The key in the command is incompatible with the related mapping.
    #[error("Key type in `{0}` '{1}' does not match the key type in the mapping '{2}'.")]
    MappingKeyTypeMismatch(String, String, String),
    /// The value in the command is incompatible with the related mapping.
    #[error("Value type in `{0}` '{1}' does not match the value type in the mapping '{2}'.")]
    MappingValueTypeMismatch(String, String, String),
    /// Too many operands were used with the command.
    #[error("The number of operands must be <= {0}")]
    TooManyOperands(usize),
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
    /// The program with the given ID exists, but is not a match.
    #[error("Program '{0}' already exists with different contents")]
    DifferentProgramAlreadyExists(String),
    /// An error related to finalize types initialization.
    #[error("Initialization of finalize types failed: {0}")]
    FinalizeTypesInit(#[from] FinalizeTypesInitError),
    /// A function already exists in the process.
    #[error("Function '{0}' already exists")]
    FunctionAlreadyExists(String),
    /// A program attempted to use a dependency that hasn't been imported.
    #[error("Cannot add program, because its import '{0}' must be added first")]
    MissingImport(String),
    /// A generic Process error was encountered.
    #[error("Process error: {0}")]
    Process(#[from] ProcessError),
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
    ProgramSelfImport,
    /// An error related to program upgrade.
    #[error("Program upgrade failed: {0}")]
    ProgramUpgrade(#[from] ProgramUpgradeError),
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

/// An error occurred during the checking/execution/evaluation/synthesis of an
/// instruction.
#[derive(Debug, Error)]
pub enum InstructionError {
    /// Failed to check an instruction.
    #[error("Failed to evaluate: {0}")]
    Check(#[from] InstructionCheckError),
    /// Failed to evaluate an instruction.
    #[error("Failed to evaluate: {0}")]
    Eval(#[from] InstructionEvalError),
    /// Failed to execute an instruction.
    #[error("Failed to execute: {0}")]
    Exec(#[from] InstructionExecError),
}

/// An error occurred during the evaluation of an instruction.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum InstructionCheckError {
    /// The given instruction is disallowed in the context.
    #[error("Instruction '{0}' is not allowed in this context.")]
    ContextDisallowed(String),
    /// The instruction had multiple destinations.
    #[error("Instruction '{0}' has multiple destinations.")]
    MultipleDestinations(String),
    /// The opcode is invalid.
    #[error("'{0}' is not an opcode.")]
    OpcodeInvalid(String),
    /// The opcode is invalid for the given instruction.
    #[error("Instruction '{0}' is not for opcode '{1}'.")]
    OpcodeMismatch(String, String),
    /// Encountered an error related to register types.
    #[error("Failed to initialize register types: {0}")]
    RegisterTypesInit(#[from] RegisterTypesInitError),
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
