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

use crate::{Process, Stack, StackProgramTypes};

use console::{
    prelude::*,
    program::{FinalizeType, Identifier, LiteralType, PlaintextType},
};
use ledger_block::{Deployment, Execution};
use synthesizer_program::{CastType, Command, Finalize, Instruction, Operand, StackProgram};

/// Returns the *minimum* cost in microcredits to publish the given deployment (total cost, (storage cost, synthesis cost, namespace cost)).
pub fn deployment_cost<N: Network>(deployment: &Deployment<N>) -> Result<(u64, (u64, u64, u64))> {
    // Determine the number of bytes in the deployment.
    let size_in_bytes = deployment.size_in_bytes()?;
    // Retrieve the program ID.
    let program_id = deployment.program_id();
    // Determine the number of characters in the program ID.
    let num_characters = u32::try_from(program_id.name().to_string().len())?;
    // Compute the number of combined variables in the program.
    let num_combined_variables = deployment.num_combined_variables()?;
    // Compute the number of combined constraints in the program.
    let num_combined_constraints = deployment.num_combined_constraints()?;

    // Compute the storage cost in microcredits.
    let storage_cost = size_in_bytes
        .checked_mul(N::DEPLOYMENT_FEE_MULTIPLIER)
        .ok_or(anyhow!("The storage cost computation overflowed for a deployment"))?;

    // Compute the synthesis cost in microcredits.
    let synthesis_cost = num_combined_variables.saturating_add(num_combined_constraints) * N::SYNTHESIS_FEE_MULTIPLIER;

    // Compute the namespace cost in credits: 10^(10 - num_characters).
    let namespace_cost = 10u64
        .checked_pow(10u32.saturating_sub(num_characters))
        .ok_or(anyhow!("The namespace cost computation overflowed for a deployment"))?
        .saturating_mul(1_000_000); // 1 microcredit = 1e-6 credits.

    // Compute the total cost in microcredits.
    let total_cost = storage_cost
        .checked_add(synthesis_cost)
        .and_then(|x| x.checked_add(namespace_cost))
        .ok_or(anyhow!("The total cost computation overflowed for a deployment"))?;

    Ok((total_cost, (storage_cost, synthesis_cost, namespace_cost)))
}

/// Returns the *minimum* cost in microcredits to publish the given execution (total cost, (storage cost, finalize cost)).
pub fn execution_cost_v2<N: Network>(process: &Process<N>, execution: &Execution<N>) -> Result<(u64, (u64, u64))> {
    // Compute the storage cost in microcredits.
    let storage_cost = execution_storage_cost::<N>(execution.size_in_bytes()?);

    // Get the root transition.
    let transition = execution.peek()?;

    // Get the finalize cost for the root transition.
    let finalize_cost = process.get_stack(transition.program_id())?.get_finalize_cost(transition.function_name())?;

    // Compute the total cost in microcredits.
    let total_cost = storage_cost
        .checked_add(finalize_cost)
        .ok_or(anyhow!("The total cost computation overflowed for an execution"))?;

    Ok((total_cost, (storage_cost, finalize_cost)))
}

/// Returns the *minimum* cost in microcredits to publish the given execution (total cost, (storage cost, finalize cost)).
pub fn execution_cost_v1<N: Network>(process: &Process<N>, execution: &Execution<N>) -> Result<(u64, (u64, u64))> {
    // Compute the storage cost in microcredits.
    let storage_cost = execution_storage_cost::<N>(execution.size_in_bytes()?);

    // Get the root transition.
    let transition = execution.peek()?;

    // Get the finalize cost for the root transition.
    let stack = process.get_stack(transition.program_id())?;
    let finalize_cost = cost_in_microcredits_v1(stack, transition.function_name())?;

    // Compute the total cost in microcredits.
    let total_cost = storage_cost
        .checked_add(finalize_cost)
        .ok_or(anyhow!("The total cost computation overflowed for an execution"))?;

    Ok((total_cost, (storage_cost, finalize_cost)))
}

/// Returns the storage cost in microcredits for a program execution.
fn execution_storage_cost<N: Network>(size_in_bytes: u64) -> u64 {
    if size_in_bytes > N::EXECUTION_STORAGE_PENALTY_THRESHOLD {
        size_in_bytes.saturating_mul(size_in_bytes).saturating_div(N::EXECUTION_STORAGE_FEE_SCALING_FACTOR)
    } else {
        size_in_bytes
    }
}

/// Finalize costs for compute heavy operations, derived as:
/// `BASE_COST + (PER_BYTE_COST * SIZE_IN_BYTES)`.

const CAST_BASE_COST: u64 = 500;
const CAST_PER_BYTE_COST: u64 = 30;

const HASH_BASE_COST: u64 = 10_000;
const HASH_PER_BYTE_COST: u64 = 30;

const HASH_BHP_BASE_COST: u64 = 50_000;
const HASH_BHP_PER_BYTE_COST: u64 = 300;

const HASH_PSD_BASE_COST: u64 = 40_000;
const HASH_PSD_PER_BYTE_COST: u64 = 75;

pub enum ConsensusFeeVersion {
    V1,
    V2,
}

const MAPPING_BASE_COST_V1: u64 = 10_000;
const MAPPING_BASE_COST_V2: u64 = 1_500;
const MAPPING_PER_BYTE_COST: u64 = 10;

const SET_BASE_COST: u64 = 10_000;
const SET_PER_BYTE_COST: u64 = 100;

/// A helper function to determine the plaintext type in bytes.
fn plaintext_size_in_bytes<N: Network>(stack: &Stack<N>, plaintext_type: &PlaintextType<N>) -> Result<u64> {
    match plaintext_type {
        PlaintextType::Literal(literal_type) => Ok(literal_type.size_in_bytes::<N>() as u64),
        PlaintextType::Struct(struct_name) => {
            // Retrieve the struct from the stack.
            let struct_ = stack.program().get_struct(struct_name)?;
            // Retrieve the size of the struct name.
            let size_of_name = struct_.name().to_bytes_le()?.len() as u64;
            // Retrieve the size of all the members of the struct.
            let size_of_members = struct_.members().iter().try_fold(0u64, |acc, (_, member_type)| {
                acc.checked_add(plaintext_size_in_bytes(stack, member_type)?).ok_or(anyhow!(
                    "Overflowed while computing the size of the struct '{}/{struct_name}' - {member_type}",
                    stack.program_id()
                ))
            })?;
            // Return the size of the struct.
            Ok(size_of_name.saturating_add(size_of_members))
        }
        PlaintextType::Array(array_type) => {
            // Retrieve the number of elements in the array.
            let num_elements = **array_type.length() as u64;
            // Compute the size of an array element.
            let size_of_element = plaintext_size_in_bytes(stack, array_type.next_element_type())?;
            // Return the size of the array.
            Ok(num_elements.saturating_mul(size_of_element))
        }
    }
}

/// A helper function to compute the following: base_cost + (byte_multiplier * size_of_operands).
fn cost_in_size<'a, N: Network>(
    stack: &Stack<N>,
    finalize: &Finalize<N>,
    operands: impl IntoIterator<Item = &'a Operand<N>>,
    byte_multiplier: u64,
    base_cost: u64,
) -> Result<u64> {
    // Retrieve the finalize types.
    let finalize_types = stack.get_finalize_types(finalize.name())?;
    // Compute the size of the operands.
    let size_of_operands = operands.into_iter().try_fold(0u64, |acc, operand| {
        // Determine the size of the operand.
        let operand_size = match finalize_types.get_type_from_operand(stack, operand)? {
            FinalizeType::Plaintext(plaintext_type) => plaintext_size_in_bytes(stack, &plaintext_type)?,
            FinalizeType::Future(future) => {
                bail!("Future '{future}' is not a valid operand in the finalize scope");
            }
        };
        // Safely add the size to the accumulator.
        acc.checked_add(operand_size).ok_or(anyhow!(
            "Overflowed while computing the size of the operand '{operand}' in '{}/{}' (finalize)",
            stack.program_id(),
            finalize.name()
        ))
    })?;
    // Return the cost.
    Ok(base_cost.saturating_add(byte_multiplier.saturating_mul(size_of_operands)))
}

/// Returns the the cost of a command in a finalize scope.
pub fn cost_per_command<N: Network>(
    stack: &Stack<N>,
    finalize: &Finalize<N>,
    command: &Command<N>,
    consensus_fee_version: ConsensusFeeVersion,
) -> Result<u64> {
    let mapping_base_cost = match consensus_fee_version {
        ConsensusFeeVersion::V1 => MAPPING_BASE_COST_V1,
        ConsensusFeeVersion::V2 => MAPPING_BASE_COST_V2,
    };

    match command {
        Command::Instruction(Instruction::Abs(_)) => Ok(500),
        Command::Instruction(Instruction::AbsWrapped(_)) => Ok(500),
        Command::Instruction(Instruction::Add(_)) => Ok(500),
        Command::Instruction(Instruction::AddWrapped(_)) => Ok(500),
        Command::Instruction(Instruction::And(_)) => Ok(500),
        Command::Instruction(Instruction::AssertEq(_)) => Ok(500),
        Command::Instruction(Instruction::AssertNeq(_)) => Ok(500),
        Command::Instruction(Instruction::Async(_)) => bail!("'async' is not supported in finalize"),
        Command::Instruction(Instruction::Call(_)) => bail!("'call' is not supported in finalize"),
        Command::Instruction(Instruction::Cast(cast)) => match cast.cast_type() {
            CastType::Plaintext(PlaintextType::Literal(_)) => Ok(500),
            CastType::Plaintext(plaintext_type) => Ok(plaintext_size_in_bytes(stack, plaintext_type)?
                .saturating_mul(CAST_PER_BYTE_COST)
                .saturating_add(CAST_BASE_COST)),
            CastType::GroupXCoordinate
            | CastType::GroupYCoordinate
            | CastType::Record(_)
            | CastType::ExternalRecord(_) => Ok(500),
        },
        Command::Instruction(Instruction::CastLossy(cast_lossy)) => match cast_lossy.cast_type() {
            CastType::Plaintext(PlaintextType::Literal(_)) => Ok(500),
            CastType::Plaintext(plaintext_type) => Ok(plaintext_size_in_bytes(stack, plaintext_type)?
                .saturating_mul(CAST_PER_BYTE_COST)
                .saturating_add(CAST_BASE_COST)),
            CastType::GroupXCoordinate
            | CastType::GroupYCoordinate
            | CastType::Record(_)
            | CastType::ExternalRecord(_) => Ok(500),
        },
        Command::Instruction(Instruction::CommitBHP256(commit)) => {
            cost_in_size(stack, finalize, commit.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::CommitBHP512(commit)) => {
            cost_in_size(stack, finalize, commit.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::CommitBHP768(commit)) => {
            cost_in_size(stack, finalize, commit.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::CommitBHP1024(commit)) => {
            cost_in_size(stack, finalize, commit.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::CommitPED64(commit)) => {
            cost_in_size(stack, finalize, commit.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::CommitPED128(commit)) => {
            cost_in_size(stack, finalize, commit.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::Div(div)) => {
            // Ensure `div` has exactly two operands.
            ensure!(div.operands().len() == 2, "'div' must contain exactly 2 operands");
            // Retrieve the finalize types.
            let finalize_types = stack.get_finalize_types(finalize.name())?;
            // Retrieve the price by the operand type.
            match finalize_types.get_type_from_operand(stack, &div.operands()[0])? {
                FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::Field)) => Ok(1_500),
                FinalizeType::Plaintext(PlaintextType::Literal(_)) => Ok(500),
                FinalizeType::Plaintext(PlaintextType::Array(_)) => bail!("'div' does not support arrays"),
                FinalizeType::Plaintext(PlaintextType::Struct(_)) => bail!("'div' does not support structs"),
                FinalizeType::Future(_) => bail!("'div' does not support futures"),
            }
        }
        Command::Instruction(Instruction::DivWrapped(_)) => Ok(500),
        Command::Instruction(Instruction::Double(_)) => Ok(500),
        Command::Instruction(Instruction::GreaterThan(_)) => Ok(500),
        Command::Instruction(Instruction::GreaterThanOrEqual(_)) => Ok(500),
        Command::Instruction(Instruction::HashBHP256(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::HashBHP512(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::HashBHP768(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::HashBHP1024(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak256(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak384(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak512(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashPED64(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashPED128(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashPSD2(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_PSD_PER_BYTE_COST, HASH_PSD_BASE_COST)
        }
        Command::Instruction(Instruction::HashPSD4(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_PSD_PER_BYTE_COST, HASH_PSD_BASE_COST)
        }
        Command::Instruction(Instruction::HashPSD8(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_PSD_PER_BYTE_COST, HASH_PSD_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_256(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_384(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_512(hash)) => {
            cost_in_size(stack, finalize, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashManyPSD2(_)) => {
            bail!("`hash_many.psd2` is not supported in finalize")
        }
        Command::Instruction(Instruction::HashManyPSD4(_)) => {
            bail!("`hash_many.psd4` is not supported in finalize")
        }
        Command::Instruction(Instruction::HashManyPSD8(_)) => {
            bail!("`hash_many.psd8` is not supported in finalize")
        }
        Command::Instruction(Instruction::Inv(_)) => Ok(2_500),
        Command::Instruction(Instruction::IsEq(_)) => Ok(500),
        Command::Instruction(Instruction::IsNeq(_)) => Ok(500),
        Command::Instruction(Instruction::LessThan(_)) => Ok(500),
        Command::Instruction(Instruction::LessThanOrEqual(_)) => Ok(500),
        Command::Instruction(Instruction::Modulo(_)) => Ok(500),
        Command::Instruction(Instruction::Mul(mul)) => {
            // Ensure `mul` has exactly two operands.
            ensure!(mul.operands().len() == 2, "'mul' must contain exactly 2 operands");
            // Retrieve the finalize types.
            let finalize_types = stack.get_finalize_types(finalize.name())?;
            // Retrieve the price by operand type.
            match finalize_types.get_type_from_operand(stack, &mul.operands()[0])? {
                FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::Group)) => Ok(10_000),
                FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::Scalar)) => Ok(10_000),
                FinalizeType::Plaintext(PlaintextType::Literal(_)) => Ok(500),
                FinalizeType::Plaintext(PlaintextType::Array(_)) => bail!("'mul' does not support arrays"),
                FinalizeType::Plaintext(PlaintextType::Struct(_)) => bail!("'mul' does not support structs"),
                FinalizeType::Future(_) => bail!("'mul' does not support futures"),
            }
        }
        Command::Instruction(Instruction::MulWrapped(_)) => Ok(500),
        Command::Instruction(Instruction::Nand(_)) => Ok(500),
        Command::Instruction(Instruction::Neg(_)) => Ok(500),
        Command::Instruction(Instruction::Nor(_)) => Ok(500),
        Command::Instruction(Instruction::Not(_)) => Ok(500),
        Command::Instruction(Instruction::Or(_)) => Ok(500),
        Command::Instruction(Instruction::Pow(pow)) => {
            // Ensure `pow` has at least one operand.
            ensure!(!pow.operands().is_empty(), "'pow' must contain at least 1 operand");
            // Retrieve the finalize types.
            let finalize_types = stack.get_finalize_types(finalize.name())?;
            // Retrieve the price by operand type.
            match finalize_types.get_type_from_operand(stack, &pow.operands()[0])? {
                FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::Field)) => Ok(1_500),
                FinalizeType::Plaintext(PlaintextType::Literal(_)) => Ok(500),
                FinalizeType::Plaintext(PlaintextType::Array(_)) => bail!("'pow' does not support arrays"),
                FinalizeType::Plaintext(PlaintextType::Struct(_)) => bail!("'pow' does not support structs"),
                FinalizeType::Future(_) => bail!("'pow' does not support futures"),
            }
        }
        Command::Instruction(Instruction::PowWrapped(_)) => Ok(500),
        Command::Instruction(Instruction::Rem(_)) => Ok(500),
        Command::Instruction(Instruction::RemWrapped(_)) => Ok(500),
        Command::Instruction(Instruction::SignVerify(sign)) => {
            cost_in_size(stack, finalize, sign.operands(), HASH_PSD_PER_BYTE_COST, HASH_PSD_BASE_COST)
        }
        Command::Instruction(Instruction::Shl(_)) => Ok(500),
        Command::Instruction(Instruction::ShlWrapped(_)) => Ok(500),
        Command::Instruction(Instruction::Shr(_)) => Ok(500),
        Command::Instruction(Instruction::ShrWrapped(_)) => Ok(500),
        Command::Instruction(Instruction::Square(_)) => Ok(500),
        Command::Instruction(Instruction::SquareRoot(_)) => Ok(2_500),
        Command::Instruction(Instruction::Sub(_)) => Ok(500),
        Command::Instruction(Instruction::SubWrapped(_)) => Ok(500),
        Command::Instruction(Instruction::Ternary(_)) => Ok(500),
        Command::Instruction(Instruction::Xor(_)) => Ok(500),
        Command::Await(_) => Ok(500),
        Command::Contains(command) => {
            cost_in_size(stack, finalize, [command.key()], MAPPING_PER_BYTE_COST, mapping_base_cost)
        }
        Command::Get(command) => {
            cost_in_size(stack, finalize, [command.key()], MAPPING_PER_BYTE_COST, mapping_base_cost)
        }
        Command::GetOrUse(command) => {
            cost_in_size(stack, finalize, [command.key()], MAPPING_PER_BYTE_COST, mapping_base_cost)
        }
        Command::RandChaCha(_) => Ok(25_000),
        Command::Remove(_) => Ok(SET_BASE_COST),
        Command::Set(command) => {
            cost_in_size(stack, finalize, [command.key(), command.value()], SET_PER_BYTE_COST, SET_BASE_COST)
        }
        Command::BranchEq(_) | Command::BranchNeq(_) => Ok(500),
        Command::Position(_) => Ok(100),
    }
}

/// Returns the minimum number of microcredits required to run the finalize.
pub fn cost_in_microcredits_v2<N: Network>(stack: &Stack<N>, function_name: &Identifier<N>) -> Result<u64> {
    // Retrieve the finalize logic.
    let Some(finalize) = stack.get_function_ref(function_name)?.finalize_logic() else {
        // Return a finalize cost of 0, if the function does not have a finalize scope.
        return Ok(0);
    };
    // Get the cost of finalizing all futures.
    let mut future_cost = 0u64;
    for input in finalize.inputs() {
        if let FinalizeType::Future(future) = input.finalize_type() {
            // Get the external stack for the future.
            let stack = stack.get_external_stack(future.program_id())?;
            // Accumulate the finalize cost of the future.
            future_cost = future_cost
                .checked_add(stack.get_finalize_cost(future.resource())?)
                .ok_or(anyhow!("Finalize cost overflowed"))?;
        }
    }
    // Aggregate the cost of all commands in the program.
    finalize
        .commands()
        .iter()
        .map(|command| cost_per_command(stack, finalize, command, ConsensusFeeVersion::V2))
        .try_fold(future_cost, |acc, res| {
            res.and_then(|x| acc.checked_add(x).ok_or(anyhow!("Finalize cost overflowed")))
        })
}

/// Returns the minimum number of microcredits required to run the finalize (depcrated).
pub fn cost_in_microcredits_v1<N: Network>(stack: &Stack<N>, function_name: &Identifier<N>) -> Result<u64> {
    // Retrieve the finalize logic.
    let Some(finalize) = stack.get_function_ref(function_name)?.finalize_logic() else {
        // Return a finalize cost of 0, if the function does not have a finalize scope.
        return Ok(0);
    };
    // Get the cost of finalizing all futures.
    let mut future_cost = 0u64;
    for input in finalize.inputs() {
        if let FinalizeType::Future(future) = input.finalize_type() {
            // Get the external stack for the future.
            let stack = stack.get_external_stack(future.program_id())?;
            // Accumulate the finalize cost of the future.
            future_cost = future_cost
                .checked_add(cost_in_microcredits_v1(stack, future.resource())?)
                .ok_or(anyhow!("Finalize cost overflowed"))?;
        }
    }
    // Aggregate the cost of all commands in the program.
    finalize
        .commands()
        .iter()
        .map(|command| cost_per_command(stack, finalize, command, ConsensusFeeVersion::V1))
        .try_fold(future_cost, |acc, res| {
            res.and_then(|x| acc.checked_add(x).ok_or(anyhow!("Finalize cost overflowed")))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::get_execution;

    use console::network::{CanaryV0, MainnetV0, TestnetV0};
    use synthesizer_program::Program;

    // Test program with two functions just below and above the size threshold.
    const SIZE_BOUNDARY_PROGRAM: &str = r#"
program size_boundary.aleo;

function under_five_thousand:
    input r0 as group.public;
    cast r0 r0 r0 r0 r0 r0 r0 r0 r0 into r1 as [group; 9u32];
    cast r1 r1 r1 r1 r1 r1 r1 r1 r1 r1 into r2 as [[group; 9u32]; 10u32];
    cast r0 r0 r0 r0 r0 r0 r0 into r3 as [group; 7u32];
    output r2 as [[group; 9u32]; 10u32].public;
    output r3 as [group; 7u32].public;

function over_five_thousand:
    input r0 as group.public;
    cast r0 r0 r0 r0 r0 r0 r0 r0 r0 into r1 as [group; 9u32];
    cast r1 r1 r1 r1 r1 r1 r1 r1 r1 r1 into r2 as [[group; 9u32]; 10u32];
    cast r0 r0 r0 r0 r0 r0 r0 into r3 as [group; 7u32];
    output r2 as [[group; 9u32]; 10u32].public;
    output r3 as [group; 7u32].public;
    output 5u64 as u64.public;
    "#;
    // Cost for a program +1 byte above the threshold.
    const STORAGE_COST_ABOVE_THRESHOLD: u64 = 5002;
    // Storage cost for an execution transaction at the maximum transaction size.
    const STORAGE_COST_MAX: u64 = 3_276_800;

    fn test_storage_cost_bounds<N: Network>() {
        // Calculate the bounds directly above and below the size threshold.
        let threshold = N::EXECUTION_STORAGE_PENALTY_THRESHOLD;
        let threshold_lower_offset = threshold.saturating_sub(1);
        let threshold_upper_offset = threshold.saturating_add(1);

        // Test the storage cost bounds.
        assert_eq!(execution_storage_cost::<N>(0), 0);
        assert_eq!(execution_storage_cost::<N>(1), 1);
        assert_eq!(execution_storage_cost::<N>(threshold_lower_offset), threshold_lower_offset);
        assert_eq!(execution_storage_cost::<N>(threshold), threshold);
        assert_eq!(execution_storage_cost::<N>(threshold_upper_offset), STORAGE_COST_ABOVE_THRESHOLD);
        assert_eq!(execution_storage_cost::<N>(N::MAX_TRANSACTION_SIZE as u64), STORAGE_COST_MAX);
    }

    #[test]
    fn test_storage_cost_bounds_for_all_networks() {
        test_storage_cost_bounds::<CanaryV0>();
        test_storage_cost_bounds::<MainnetV0>();
        test_storage_cost_bounds::<TestnetV0>();
    }

    #[test]
    fn test_storage_costs_compute_correctly() {
        // Test the storage cost of an execution.
        let threshold = MainnetV0::EXECUTION_STORAGE_PENALTY_THRESHOLD;

        // Test the cost of an execution.
        let mut process = Process::load().unwrap();

        // Get the program.
        let program = Program::from_str(SIZE_BOUNDARY_PROGRAM).unwrap();

        // Get the program identifiers.
        let under_5000 = Identifier::from_str("under_five_thousand").unwrap();
        let over_5000 = Identifier::from_str("over_five_thousand").unwrap();

        // Get execution and cost data.
        let execution_under_5000 = get_execution(&mut process, &program, &under_5000, ["2group"].into_iter());
        let execution_size_under_5000 = execution_under_5000.size_in_bytes().unwrap();
        let (_, (storage_cost_under_5000, _)) = execution_cost_v2(&process, &execution_under_5000).unwrap();
        let execution_over_5000 = get_execution(&mut process, &program, &over_5000, ["2group"].into_iter());
        let execution_size_over_5000 = execution_over_5000.size_in_bytes().unwrap();
        let (_, (storage_cost_over_5000, _)) = execution_cost_v2(&process, &execution_over_5000).unwrap();

        // Ensure the sizes are below and above the threshold respectively.
        assert!(execution_size_under_5000 < threshold);
        assert!(execution_size_over_5000 > threshold);

        // Ensure storage costs compute correctly.
        assert_eq!(storage_cost_under_5000, execution_storage_cost::<MainnetV0>(execution_size_under_5000));
        assert_eq!(storage_cost_over_5000, execution_storage_cost::<MainnetV0>(execution_size_over_5000));
    }
}
