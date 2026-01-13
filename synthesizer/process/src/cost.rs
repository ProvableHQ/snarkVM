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

use std::collections::HashMap;

use crate::{Authorization, FinalizeTypes, Process, Stack, StackRef, StackTrait};

use console::{
    prelude::*,
    program::{FinalizeType, Identifier, LiteralType, PlaintextType},
};
use snarkvm_algorithms::snark::varuna::VarunaVersion;
use snarkvm_ledger_block::{Deployment, Execution, Transaction};
use snarkvm_synthesizer_program::{CastType, Command, Instruction, Operand};
use snarkvm_synthesizer_snark::proof_size;

pub type MinimumCost = u64;
pub type StorageCost = u64;
pub type SynthesisCost = u64;
pub type ConstructorCost = u64;
pub type NamespaceCost = u64;
pub type FinalizeCost = u64;
pub type DeployCostDetails = (StorageCost, SynthesisCost, ConstructorCost, NamespaceCost);
pub type ExecuteCostDetails = (StorageCost, FinalizeCost);

/// Returns the deployment cost in microcredits for a given deployment.
pub fn deployment_cost<N: Network>(
    process: &Process<N>,
    deployment: &Deployment<N>,
    consensus_version: ConsensusVersion,
) -> Result<(MinimumCost, DeployCostDetails)> {
    if consensus_version >= ConsensusVersion::V10 {
        deployment_cost_v2(process, deployment)
    } else {
        deployment_cost_v1(process, deployment)
    }
}

/// Returns the execution cost in microcredits for a given execution.
pub fn execution_cost<N: Network>(
    process: &Process<N>,
    execution: &Execution<N>,
    consensus_version: ConsensusVersion,
) -> Result<(MinimumCost, ExecuteCostDetails)> {
    let execution_size = execution.size_in_bytes()?;

    execution_cost_given_size(process, execution, execution_size, consensus_version)
}

// Returns the execution cost in microcredits for a given execution whose size is provided as an argument.
fn execution_cost_given_size<N: Network>(
    process: &Process<N>,
    execution: &Execution<N>,
    execution_size: u64,
    consensus_version: ConsensusVersion,
) -> Result<(MinimumCost, ExecuteCostDetails)> {
    if consensus_version >= ConsensusVersion::V10 {
        execution_cost_v3(process, execution, execution_size)
    } else if consensus_version >= ConsensusVersion::V2 {
        execution_cost_v2(process, execution, execution_size)
    } else {
        execution_cost_v1(process, execution, execution_size)
    }
}

/// Returns the execution cost in microcredits for a given `Authorization.
pub fn execution_cost_for_authorization<N: Network>(
    process: &Process<N>,
    authorization: &Authorization<N>,
    consensus_version: ConsensusVersion,
) -> Result<(MinimumCost, ExecuteCostDetails)> {
    ensure!(
        consensus_version >= ConsensusVersion::V4,
        "Execution-cost computation for authorization relies on proof-size estimation, which is only implemented for Varuna version >= V2 (consensus version >= V4)"
    );

    // Reconstruct an Execution from the Authorization. Note that the StateRoot
    // does not affect the fee (it has constant size).
    let reconstructed_execution =
        Execution::from(authorization.transitions().values().cloned(), N::StateRoot::default(), None)?;

    // Compute the size of the proof that will result from proving the
    // Authorization. The first step is to compute the Varuna batch sizes. The
    // Varuna circuits that must be proved as part of an Execution are:
    // - the circuits of each Transition
    // - one inclusion circuit for input records to *all* of those Transitions

    // TODO: Dynamic dispatch, once implemented, will cause a third type of
    // circuit to appear which needs to be accounted for here.

    let mut circuit_frequencies = HashMap::new();

    // In order to compute the frequencies of function circuits, we mimic the
    // operation of Process::verify_execution:
    for transition in authorization.transitions().values() {
        let entry =
            circuit_frequencies.entry((*transition.program_id(), *transition.function_name())).or_insert(0usize);
        *entry += 1;
    }

    let mut batch_sizes: Vec<usize> = circuit_frequencies.values().cloned().collect();

    // We now add the single batch of inclusion circuits for input records, if
    // any:
    let n_input_records = Authorization::number_of_input_records(authorization.transitions().values());
    if n_input_records > 0 {
        batch_sizes.push(n_input_records);
    }

    // Varuna is always run in hiding (i. e. ZK) mode when proving Executions.
    let hiding_mode = true;

    // If future versions of Varuna are introduced, the correct one should be
    // deduced here from the consensus version. Currently only the latest Varuna
    // version V2 is supported.
    let varuna_version = VarunaVersion::V2;

    let expected_proof_size = u64::try_from(proof_size::<N>(&batch_sizes, varuna_version, hiding_mode)?)?;
    let unproved_execution_size = reconstructed_execution.size_in_bytes()?;
    let execution_size = unproved_execution_size.checked_add(expected_proof_size).ok_or(anyhow!(
        "The execution size computation overflowed for an authorization when the proof was taken into account"
    ))?;

    execution_cost_given_size(process, &reconstructed_execution, execution_size, consensus_version)
}

/// Returns the compute cost for a deployment in microcredits.
/// This is used to limit the amount of single-threaded compute in the block generation hot
/// path. This does NOT represent the full costs which a user has to pay.
pub fn deploy_compute_cost_in_microcredits(
    cost_details: DeployCostDetails,
    consensus_version: ConsensusVersion,
) -> Result<u64> {
    let (storage_cost, synthesis_cost, constructor_cost, _) = cost_details;
    let cost_to_check = if consensus_version >= ConsensusVersion::V10 {
        // From V10, only include the constructor compute cost for
        // deployments.
        //
        // The limits of individual function's finalize compute costs are
        // checked in calls to `deployment_cost`.
        constructor_cost
    } else {
        // Include the storage, synthesis, and constructor cost for deployments.
        storage_cost
            .checked_add(synthesis_cost)
            .and_then(|synthesis_cost| synthesis_cost.checked_add(constructor_cost))
            .ok_or(anyhow!("The storage, synthesis, and constructor cost computation overflowed for a deployment"))?
    };
    Ok(cost_to_check)
}

/// Returns the compute cost for an execution in microcredits.
/// This is used to limit the amount of single-threaded compute in the block generation hot
/// path. This does NOT represent the full costs which a user has to pay.
pub fn execute_compute_cost_in_microcredits(
    cost_details: ExecuteCostDetails,
    consensus_version: ConsensusVersion,
) -> Result<u64> {
    let (storage_cost, finalize_cost) = cost_details;
    let cost_to_check = if consensus_version >= ConsensusVersion::V10 {
        // From V10, only include the finalize compute cost for executions.
        finalize_cost
    } else {
        // Include the finalize cost and storage cost for executions.
        storage_cost
            .checked_add(finalize_cost)
            .ok_or(anyhow!("The storage and finalize cost computation overflowed for an execution"))?
    };
    Ok(cost_to_check)
}

/// Returns the *minimum* cost in microcredits to publish the given deployment using the ARC_0005_COMPUTE_DISCOUNT.
pub fn deployment_cost_v2<N: Network>(
    process: &Process<N>,
    deployment: &Deployment<N>,
) -> Result<(MinimumCost, DeployCostDetails)> {
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
    let synthesis_cost = num_combined_variables.saturating_add(num_combined_constraints) * N::SYNTHESIS_FEE_MULTIPLIER
        / N::ARC_0005_COMPUTE_DISCOUNT;

    // Compute a Stack for the deployment.
    let stack = Stack::new(process, deployment.program())?;

    // Compute the constructor cost in microcredits.
    let constructor_cost = constructor_cost_in_microcredits_v2(&stack)?;

    // Check that the functions are valid.
    for function in deployment.program().functions().values() {
        // Get the finalize cost.
        let finalize_cost = cost_in_microcredits_v3(&stack, function.name())?;
        // Check that the finalize cost does not exceed the maximum.
        ensure!(
            finalize_cost <= N::TRANSACTION_SPEND_LIMIT[1].1,
            "Finalize block '{}' has a cost '{finalize_cost}' which exceeds the transaction spend limit '{}'",
            function.name(),
            N::TRANSACTION_SPEND_LIMIT[1].1
        );
    }

    // Compute the namespace cost in microcredits: 10^(10 - num_characters) * 1e6
    let namespace_cost = 10u64
        .checked_pow(10u32.saturating_sub(num_characters))
        .ok_or(anyhow!("The namespace cost computation overflowed for a deployment"))?
        .saturating_mul(1_000_000); // 1 microcredit = 1e-6 credits.

    // Compute the minimum cost in microcredits.
    let minimum_cost = storage_cost
        .checked_add(synthesis_cost)
        .and_then(|x| x.checked_add(constructor_cost))
        .and_then(|x| x.checked_add(namespace_cost))
        .ok_or(anyhow!("The total cost computation overflowed for a deployment"))?;

    Ok((minimum_cost, (storage_cost, synthesis_cost, constructor_cost, namespace_cost)))
}

/// Returns the *minimum* cost in microcredits to publish the given deployment.
pub fn deployment_cost_v1<N: Network>(
    process: &Process<N>,
    deployment: &Deployment<N>,
) -> Result<(MinimumCost, DeployCostDetails)> {
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

    // Compute a Stack for the deployment.
    let stack = Stack::new(process, deployment.program())?;

    // Compute the constructor cost in microcredits.
    let constructor_cost = constructor_cost_in_microcredits_v1(&stack)?;

    // Check that the functions are valid.
    for function in deployment.program().functions().values() {
        // Get the finalize cost.
        let finalize_cost = cost_in_microcredits_v2(&stack, function.name())?;
        // Check that the finalize cost does not exceed the maximum.
        ensure!(
            finalize_cost <= N::TRANSACTION_SPEND_LIMIT[0].1,
            "Finalize block '{}' has a cost '{finalize_cost}' which exceeds the transaction spend limit '{}'",
            function.name(),
            N::TRANSACTION_SPEND_LIMIT[0].1
        );
    }

    // Compute the namespace cost in microcredits: 10^(10 - num_characters) * 1e6
    let namespace_cost = 10u64
        .checked_pow(10u32.saturating_sub(num_characters))
        .ok_or(anyhow!("The namespace cost computation overflowed for a deployment"))?
        .saturating_mul(1_000_000); // 1 microcredit = 1e-6 credits.

    // Compute the minimum cost in microcredits.
    let minimum_cost = storage_cost
        .checked_add(synthesis_cost)
        .and_then(|x| x.checked_add(constructor_cost))
        .and_then(|x| x.checked_add(namespace_cost))
        .ok_or(anyhow!("The total cost computation overflowed for a deployment"))?;

    Ok((minimum_cost, (storage_cost, synthesis_cost, constructor_cost, namespace_cost)))
}

/// Returns the *minimum* cost in microcredits to publish the given execution using the ARC_0005_COMPUTE_DISCOUNT.
fn execution_cost_v3<N: Network>(
    process: &Process<N>,
    execution: &Execution<N>,
    execution_size: u64,
) -> Result<(MinimumCost, ExecuteCostDetails)> {
    // Compute the storage cost in microcredits.
    let storage_cost = execution_storage_cost::<N>(execution_size);

    // Get the root transition.
    let transition = execution.peek()?;

    // Get the finalize cost for the root transition.
    let stack = process.get_stack(transition.program_id())?;
    let finalize_cost = cost_in_microcredits_v3(&stack, transition.function_name())?;

    // Compute the minimum cost in microcredits.
    let minimum_cost = storage_cost
        .checked_add(finalize_cost)
        .ok_or(anyhow!("The total cost computation overflowed for an execution"))?;

    Ok((minimum_cost, (storage_cost, finalize_cost)))
}

/// Returns the *minimum* cost in microcredits to publish the given execution.
fn execution_cost_v2<N: Network>(
    process: &Process<N>,
    execution: &Execution<N>,
    execution_size: u64,
) -> Result<(MinimumCost, ExecuteCostDetails)> {
    // Compute the storage cost in microcredits.
    let storage_cost = execution_storage_cost::<N>(execution_size);

    // Get the root transition.
    let transition = execution.peek()?;

    // Get the finalize cost for the root transition.
    let stack = process.get_stack(transition.program_id())?;
    let finalize_cost = cost_in_microcredits_v2(&stack, transition.function_name())?;

    // Compute the total cost in microcredits.
    let total_cost = storage_cost
        .checked_add(finalize_cost)
        .ok_or(anyhow!("The total cost computation overflowed for an execution"))?;

    Ok((total_cost, (storage_cost, finalize_cost)))
}

/// Returns the *minimum* cost in microcredits to publish the given execution.
fn execution_cost_v1<N: Network>(
    process: &Process<N>,
    execution: &Execution<N>,
    execution_size: u64,
) -> Result<(MinimumCost, ExecuteCostDetails)> {
    // Compute the storage cost in microcredits.
    let storage_cost = execution_storage_cost::<N>(execution_size);

    // Get the root transition.
    let transition = execution.peek()?;

    // Get the finalize cost for the root transition.
    let stack = process.get_stack(transition.program_id())?;
    let finalize_cost = cost_in_microcredits_v1(&stack, transition.function_name())?;

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

// Finalize costs for compute heavy operations, derived as:
// `BASE_COST + (PER_BYTE_COST * SIZE_IN_BYTES)`.

const CAST_BASE_COST: u64 = 500;
const CAST_PER_BYTE_COST: u64 = 30;

const HASH_BASE_COST: u64 = 10_000;
const HASH_PER_BYTE_COST: u64 = 30;

const HASH_BHP_BASE_COST: u64 = 50_000;
const HASH_BHP_PER_BYTE_COST: u64 = 300;

const HASH_PSD_BASE_COST: u64 = 40_000;
const HASH_PSD_PER_BYTE_COST: u64 = 75;

const ECDSA_VERIFY_BASE_COST: u64 = 60_000;
const ECDSA_VERIFY_ETH_BASE_COST: u64 = 75_000;

#[derive(Copy, Clone)]
pub enum ConsensusFeeVersion {
    V1,
    V2,
    V3,
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
        PlaintextType::ExternalStruct(locator) => {
            let external_stack = stack.get_external_stack(locator.program_id())?;
            plaintext_size_in_bytes(&*external_stack, &PlaintextType::Struct(*locator.resource()))
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
    finalize_types: &FinalizeTypes<N>,
    operands: impl IntoIterator<Item = &'a Operand<N>>,
    byte_multiplier: u64,
    base_cost: u64,
) -> Result<u64> {
    // Compute the size of the operands.
    let size_of_operands = operands.into_iter().try_fold(0u64, |acc, operand| {
        // Determine the size of the operand.
        let operand_size = match finalize_types.get_type_from_operand(stack, operand)? {
            FinalizeType::Plaintext(plaintext_type) => plaintext_size_in_bytes(stack, &plaintext_type)?,
            FinalizeType::Future(future) => {
                bail!("Future '{future}' is not a valid operand");
            }
        };
        // Safely add the size to the accumulator.
        acc.checked_add(operand_size).ok_or(anyhow!(
            "Overflowed while computing the size of the operand '{operand}' in '{}'",
            stack.program_id(),
        ))
    })?;
    // Return the cost.
    Ok(base_cost.saturating_add(byte_multiplier.saturating_mul(size_of_operands)))
}

/// Returns the the cost of a command in a finalize scope.
pub fn cost_per_command<N: Network>(
    stack: &Stack<N>,
    finalize_types: &FinalizeTypes<N>,
    command: &Command<N>,
    consensus_fee_version: ConsensusFeeVersion,
) -> Result<u64> {
    let mapping_base_cost = match consensus_fee_version {
        ConsensusFeeVersion::V1 => MAPPING_BASE_COST_V1,
        ConsensusFeeVersion::V2 | ConsensusFeeVersion::V3 => MAPPING_BASE_COST_V2,
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
            cost_in_size(stack, finalize_types, commit.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::CommitBHP512(commit)) => {
            cost_in_size(stack, finalize_types, commit.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::CommitBHP768(commit)) => {
            cost_in_size(stack, finalize_types, commit.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::CommitBHP1024(commit)) => {
            cost_in_size(stack, finalize_types, commit.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::CommitPED64(commit)) => {
            cost_in_size(stack, finalize_types, commit.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::CommitPED128(commit)) => {
            cost_in_size(stack, finalize_types, commit.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::DeserializeBits(deserialize)) => {
            Ok(plaintext_size_in_bytes(stack, &PlaintextType::Array(deserialize.operand_type().clone()))?
                .saturating_mul(CAST_PER_BYTE_COST)
                .saturating_add(CAST_BASE_COST))
        }
        Command::Instruction(Instruction::DeserializeBitsRaw(deserialize)) => {
            Ok(plaintext_size_in_bytes(stack, &PlaintextType::Array(deserialize.operand_type().clone()))?
                .saturating_mul(CAST_PER_BYTE_COST)
                .saturating_add(CAST_BASE_COST))
        }
        Command::Instruction(Instruction::Div(div)) => {
            // Ensure `div` has exactly two operands.
            ensure!(div.operands().len() == 2, "'div' must contain exactly 2 operands");
            // Retrieve the price by the operand type.
            match finalize_types.get_type_from_operand(stack, &div.operands()[0])? {
                FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::Field)) => Ok(1_500),
                FinalizeType::Plaintext(PlaintextType::Literal(_)) => Ok(500),
                FinalizeType::Plaintext(PlaintextType::Array(_)) => bail!("'div' does not support arrays"),
                FinalizeType::Plaintext(PlaintextType::Struct(_) | PlaintextType::ExternalStruct(_)) => {
                    bail!("'div' does not support structs")
                }
                FinalizeType::Future(_) => bail!("'div' does not support futures"),
            }
        }
        Command::Instruction(Instruction::DivWrapped(_)) => Ok(500),
        Command::Instruction(Instruction::Double(_)) => Ok(500),
        Command::Instruction(Instruction::ECDSAVerifyDigest(_)) => Ok(ECDSA_VERIFY_BASE_COST),
        Command::Instruction(Instruction::ECDSAVerifyDigestEth(_)) => Ok(ECDSA_VERIFY_ETH_BASE_COST),
        Command::Instruction(Instruction::ECDSAVerifyKeccak256(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifyKeccak256Raw(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifyKeccak256Eth(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_ETH_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifyKeccak384(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifyKeccak384Raw(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifyKeccak384Eth(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_ETH_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifyKeccak512(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifyKeccak512Raw(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifyKeccak512Eth(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_ETH_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifySha3_256(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifySha3_256Raw(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifySha3_256Eth(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_ETH_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifySha3_384(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifySha3_384Raw(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifySha3_384Eth(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_ETH_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifySha3_512(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifySha3_512Raw(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_BASE_COST)
        }
        Command::Instruction(Instruction::ECDSAVerifySha3_512Eth(ecdsa)) => {
            cost_in_size(stack, finalize_types, ecdsa.operands(), HASH_PER_BYTE_COST, ECDSA_VERIFY_ETH_BASE_COST)
        }
        Command::Instruction(Instruction::GreaterThan(_)) => Ok(500),
        Command::Instruction(Instruction::GreaterThanOrEqual(_)) => Ok(500),
        Command::Instruction(Instruction::HashBHP256(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::HashBHP256Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::HashBHP512(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::HashBHP512Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::HashBHP768(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::HashBHP768Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::HashBHP1024(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::HashBHP1024Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_BHP_PER_BYTE_COST, HASH_BHP_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak256(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak256Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak256Native(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak256NativeRaw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak384(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak384Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak384Native(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak384NativeRaw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak512(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak512Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak512Native(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashKeccak512NativeRaw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashPED64(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashPED64Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashPED128(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashPED128Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashPSD2(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PSD_PER_BYTE_COST, HASH_PSD_BASE_COST)
        }
        Command::Instruction(Instruction::HashPSD2Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PSD_PER_BYTE_COST, HASH_PSD_BASE_COST)
        }
        Command::Instruction(Instruction::HashPSD4(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PSD_PER_BYTE_COST, HASH_PSD_BASE_COST)
        }
        Command::Instruction(Instruction::HashPSD4Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PSD_PER_BYTE_COST, HASH_PSD_BASE_COST)
        }
        Command::Instruction(Instruction::HashPSD8(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PSD_PER_BYTE_COST, HASH_PSD_BASE_COST)
        }
        Command::Instruction(Instruction::HashPSD8Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PSD_PER_BYTE_COST, HASH_PSD_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_256(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_256Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_256Native(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_256NativeRaw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_384(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_384Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_384Native(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_384NativeRaw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_512(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_512Raw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_512Native(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
        }
        Command::Instruction(Instruction::HashSha3_512NativeRaw(hash)) => {
            cost_in_size(stack, finalize_types, hash.operands(), HASH_PER_BYTE_COST, HASH_BASE_COST)
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
            // Retrieve the price by operand type.
            match finalize_types.get_type_from_operand(stack, &mul.operands()[0])? {
                FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::Group)) => Ok(10_000),
                FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::Scalar)) => Ok(10_000),
                FinalizeType::Plaintext(PlaintextType::Literal(_)) => Ok(500),
                FinalizeType::Plaintext(PlaintextType::Array(_)) => bail!("'mul' does not support arrays"),
                FinalizeType::Plaintext(PlaintextType::Struct(_) | PlaintextType::ExternalStruct(_)) => {
                    bail!("'mul' does not support structs")
                }
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
            // Retrieve the price by operand type.
            match finalize_types.get_type_from_operand(stack, &pow.operands()[0])? {
                FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::Field)) => Ok(1_500),
                FinalizeType::Plaintext(PlaintextType::Literal(_)) => Ok(500),
                FinalizeType::Plaintext(PlaintextType::Array(_)) => bail!("'pow' does not support arrays"),
                FinalizeType::Plaintext(PlaintextType::Struct(_) | PlaintextType::ExternalStruct(_)) => {
                    bail!("'pow' does not support structs")
                }
                FinalizeType::Future(_) => bail!("'pow' does not support futures"),
            }
        }
        Command::Instruction(Instruction::PowWrapped(_)) => Ok(500),
        Command::Instruction(Instruction::Rem(_)) => Ok(500),
        Command::Instruction(Instruction::RemWrapped(_)) => Ok(500),
        Command::Instruction(Instruction::SerializeBits(serialize)) => {
            Ok(plaintext_size_in_bytes(stack, &PlaintextType::Array(serialize.destination_type().clone()))?
                .saturating_mul(CAST_PER_BYTE_COST)
                .saturating_add(CAST_BASE_COST))
        }
        Command::Instruction(Instruction::SerializeBitsRaw(serialize)) => {
            Ok(plaintext_size_in_bytes(stack, &PlaintextType::Array(serialize.destination_type().clone()))?
                .saturating_mul(CAST_PER_BYTE_COST)
                .saturating_add(CAST_BASE_COST))
        }
        Command::Instruction(Instruction::SignVerify(sign)) => {
            cost_in_size(stack, finalize_types, sign.operands(), HASH_PSD_PER_BYTE_COST, HASH_PSD_BASE_COST)
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
            cost_in_size(stack, finalize_types, [command.key()], MAPPING_PER_BYTE_COST, mapping_base_cost)
        }
        Command::Get(command) => {
            cost_in_size(stack, finalize_types, [command.key()], MAPPING_PER_BYTE_COST, mapping_base_cost)
        }
        Command::GetOrUse(command) => {
            cost_in_size(stack, finalize_types, [command.key()], MAPPING_PER_BYTE_COST, mapping_base_cost)
        }
        Command::RandChaCha(_) => Ok(25_000),
        Command::Remove(_) => Ok(SET_BASE_COST),
        Command::Set(command) => {
            cost_in_size(stack, finalize_types, [command.key(), command.value()], SET_PER_BYTE_COST, SET_BASE_COST)
        }
        Command::BranchEq(_) | Command::BranchNeq(_) => Ok(500),
        Command::Position(_) => Ok(100),
    }
}

/// Returns the minimum number of microcredits required to run the constructor in the given stack.
/// If a constructor does not exist, no cost is incurred.
pub fn constructor_cost_in_microcredits_v2<N: Network>(stack: &Stack<N>) -> Result<u64> {
    match stack.program().constructor() {
        Some(constructor) => {
            // Get the constructor types.
            let constructor_types = stack.get_constructor_types()?;
            // Get the base cost of the constructor.
            let base_cost = constructor
                .commands()
                .iter()
                .map(|command| cost_per_command(stack, &constructor_types, command, ConsensusFeeVersion::V2))
                .try_fold(0u64, |acc, res| {
                    res.and_then(|x| acc.checked_add(x).ok_or(anyhow!("Constructor cost overflowed")))
                })?;
            // Scale by the multiplier and divide by the ARC-0005 cost reduction factor.
            base_cost
                .checked_mul(N::CONSTRUCTOR_FEE_MULTIPLIER)
                .map(|result| result / N::ARC_0005_COMPUTE_DISCOUNT)
                .ok_or(anyhow!("Constructor cost overflowed"))
        }
        None => Ok(0),
    }
}

/// Returns the minimum number of microcredits required to run the constructor in the given stack.
/// If a constructor does not exist, no cost is incurred.
pub fn constructor_cost_in_microcredits_v1<N: Network>(stack: &Stack<N>) -> Result<u64> {
    match stack.program().constructor() {
        Some(constructor) => {
            // Get the constructor types.
            let constructor_types = stack.get_constructor_types()?;
            // Get the base cost of the constructor.
            let base_cost = constructor
                .commands()
                .iter()
                .map(|command| cost_per_command(stack, &constructor_types, command, ConsensusFeeVersion::V2))
                .try_fold(0u64, |acc, res| {
                    res.and_then(|x| acc.checked_add(x).ok_or(anyhow!("Constructor cost overflowed")))
                })?;
            // Scale by the multiplier and divide by the ARC-0005 cost reduction factor.
            base_cost.checked_mul(N::CONSTRUCTOR_FEE_MULTIPLIER).ok_or(anyhow!("Constructor cost overflowed"))
        }
        None => Ok(0),
    }
}

/// Returns the minimum number of microcredits required to run the finalize using the ARC-0005 cost reduction factor.
pub fn cost_in_microcredits_v3<N: Network>(stack: &Stack<N>, function_name: &Identifier<N>) -> Result<u64> {
    cost_in_microcredits(stack, function_name, ConsensusFeeVersion::V3)
}

/// Returns the minimum number of microcredits required to run the finalize.
pub fn cost_in_microcredits_v2<N: Network>(stack: &Stack<N>, function_name: &Identifier<N>) -> Result<u64> {
    cost_in_microcredits(stack, function_name, ConsensusFeeVersion::V2)
}

/// Returns the minimum number of microcredits required to run the finalize (deprecated).
pub fn cost_in_microcredits_v1<N: Network>(stack: &Stack<N>, function_name: &Identifier<N>) -> Result<u64> {
    cost_in_microcredits(stack, function_name, ConsensusFeeVersion::V1)
}

// A helper function to compute the cost in microcredits for a given function.
fn cost_in_microcredits<N: Network>(
    stack: &Stack<N>,
    function_name: &Identifier<N>,
    consensus_fee_version: ConsensusFeeVersion,
) -> Result<u64> {
    // Initialize the base cost.
    let mut finalize_cost = 0u64;
    // Initialize a queue of finalize blocks to tally.
    let mut finalizes = vec![(StackRef::Internal(stack), *function_name)];
    // Initialize a counter for the number of finalize blocks seen.
    let mut num_finalizes = 1;
    // Get the quotient for the cost reduction factor.
    let quotient = match consensus_fee_version {
        ConsensusFeeVersion::V1 | ConsensusFeeVersion::V2 => 1,
        ConsensusFeeVersion::V3 => N::ARC_0005_COMPUTE_DISCOUNT,
    };
    // Iterate over the finalize blocks.
    while let Some((stack_ref, function_name)) = finalizes.pop() {
        // Ensure that the number of finalize blocks does not exceed the maximum.
        // Note that one transition is reserved for the fee.
        ensure!(
            num_finalizes < Transaction::<N>::MAX_TRANSITIONS,
            "The number of finalize blocks must be less than '{}'",
            Transaction::<N>::MAX_TRANSITIONS
        );
        // Get the finalize logic. If the function does not have a finalize scope then no cost is incurred.
        if let Some(finalize) = stack_ref.get_function_ref(&function_name)?.finalize_logic() {
            // Queue the futures to be tallied.
            for input in finalize.inputs() {
                if let FinalizeType::Future(future) = input.finalize_type() {
                    // Increment the number of finalize blocks seen.
                    num_finalizes += 1;
                    // If the locator matches the program ID of the provided stack, use it directly.
                    // Otherwise, retrieve the external stack.
                    let stack = if future.program_id() == stack.program().id() {
                        StackRef::Internal(stack)
                    } else {
                        StackRef::External(stack_ref.get_external_stack(future.program_id())?)
                    };
                    // Queue the future.
                    finalizes.push((stack, *future.resource()));
                }
            }
            // Get the finalize types.
            let finalize_types = stack_ref.get_finalize_types(finalize.name())?;
            // Iterate over the commands in the finalize block.
            for command in finalize.commands() {
                // Sum the cost of all commands in the current future into the total running cost.
                finalize_cost = finalize_cost
                    .checked_add(cost_per_command(&stack_ref, &finalize_types, command, consensus_fee_version)?)
                    .ok_or(anyhow!("Finalize cost overflowed"))?;
            }
        }
    }
    Ok(finalize_cost / quotient)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::get_execution;
    use circuit::{Aleo, AleoCanaryV0, AleoTestnetV0, AleoV0};

    use console::{
        network::{CanaryV0, MainnetV0, TestnetV0},
        types::Address,
    };
    use snarkvm_synthesizer_program::Program;

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
        let (_, (storage_cost_under_5000, _)) =
            execution_cost_v3(&process, &execution_under_5000, execution_size_under_5000).unwrap();
        let execution_over_5000 = get_execution(&mut process, &program, &over_5000, ["2group"].into_iter());
        let execution_size_over_5000 = execution_over_5000.size_in_bytes().unwrap();
        let (_, (storage_cost_over_5000, _)) =
            execution_cost_v3(&process, &execution_over_5000, execution_size_over_5000).unwrap();

        // Ensure the sizes are below and above the threshold respectively.
        assert!(execution_size_under_5000 < threshold);
        assert!(execution_size_over_5000 > threshold);

        // Ensure storage costs compute correctly.
        assert_eq!(storage_cost_under_5000, execution_storage_cost::<MainnetV0>(execution_size_under_5000));
        assert_eq!(storage_cost_over_5000, execution_storage_cost::<MainnetV0>(execution_size_over_5000));
    }

    #[test]
    fn test_deployment_cost_with_constructors() {
        // A helper to run the test.
        fn run_test<N: Network, A: Aleo<Network = N>>() {
            let process = Process::<N>::load().unwrap();
            let rng = &mut TestRng::default();

            // Define the programs.
            let program_0 = Program::from_str(
                r"
program program_with_constructor.aleo;

constructor:
    assert.eq true true;

mapping foo:
    key as field.public;
    value as field.public;

function dummy:",
            )
            .unwrap();

            let program_1 = Program::from_str(
                r"
program program_with_constructor.aleo;

constructor:
    assert.eq edition 0u16;

mapping foo:
    key as field.public;
    value as field.public;

function dummy:",
            )
            .unwrap();

            let program_2 = Program::from_str(
                r"
program program_with_constructor.aleo;

constructor:
    get foo[0field] into r0;

mapping foo:
    key as field.public;
    value as field.public;

function dummy:",
            )
            .unwrap();

            let program_3 = Program::from_str(
                r"
program program_with_constructor.aleo;

constructor:
    set 0field into foo[0field];

mapping foo:
    key as field.public;
    value as field.public;

function dummy:",
            )
            .unwrap();

            // Verify the deployment costs.
            let mut deployment_0 = process.deploy::<A, _>(&program_0, rng).unwrap();
            deployment_0.set_program_checksum_raw(Some(deployment_0.program().to_checksum()));
            deployment_0.set_program_owner_raw(Some(Address::rand(rng)));
            let expected_storage_cost = 879000;
            let expected_synthesis_cost = 603500;
            let expected_constructor_cost = 50000;
            let expected_namespace_cost = 1000000;
            let expected_total_cost =
                expected_storage_cost + expected_synthesis_cost + expected_constructor_cost + expected_namespace_cost;
            assert_eq!(
                deployment_cost_v1(&process, &deployment_0).unwrap(),
                (
                    expected_total_cost,
                    (
                        expected_storage_cost,
                        expected_synthesis_cost,
                        expected_constructor_cost,
                        expected_namespace_cost
                    )
                )
            );
            let expected_synthesis_cost = expected_synthesis_cost / N::ARC_0005_COMPUTE_DISCOUNT;
            let expected_constructor_cost = expected_constructor_cost / N::ARC_0005_COMPUTE_DISCOUNT;
            let expected_total_cost =
                expected_storage_cost + expected_synthesis_cost + expected_constructor_cost + expected_namespace_cost;
            assert_eq!(
                deployment_cost_v2(&process, &deployment_0).unwrap(),
                (
                    expected_total_cost,
                    (
                        expected_storage_cost,
                        expected_synthesis_cost,
                        expected_constructor_cost,
                        expected_namespace_cost
                    )
                )
            );

            let mut deployment_1 = process.deploy::<A, _>(&program_1, rng).unwrap();
            deployment_1.set_program_checksum_raw(Some(deployment_1.program().to_checksum()));
            deployment_1.set_program_owner_raw(Some(Address::rand(rng)));
            let expected_storage_cost = 878000;
            let expected_synthesis_cost = 603500;
            let expected_constructor_cost = 50000;
            let expected_namespace_cost = 1000000;
            let expected_total_cost =
                expected_storage_cost + expected_synthesis_cost + expected_constructor_cost + expected_namespace_cost;
            assert_eq!(
                deployment_cost_v1(&process, &deployment_1).unwrap(),
                (
                    expected_total_cost,
                    (
                        expected_storage_cost,
                        expected_synthesis_cost,
                        expected_constructor_cost,
                        expected_namespace_cost
                    )
                )
            );
            let expected_synthesis_cost = expected_synthesis_cost / N::ARC_0005_COMPUTE_DISCOUNT;
            let expected_constructor_cost = expected_constructor_cost / N::ARC_0005_COMPUTE_DISCOUNT;
            let expected_total_cost =
                expected_storage_cost + expected_synthesis_cost + expected_constructor_cost + expected_namespace_cost;
            assert_eq!(
                deployment_cost_v2(&process, &deployment_1).unwrap(),
                (
                    expected_total_cost,
                    (
                        expected_storage_cost,
                        expected_synthesis_cost,
                        expected_constructor_cost,
                        expected_namespace_cost
                    )
                )
            );

            let mut deployment_2 = process.deploy::<A, _>(&program_2, rng).unwrap();
            deployment_2.set_program_checksum_raw(Some(deployment_2.program().to_checksum()));
            deployment_2.set_program_owner_raw(Some(Address::rand(rng)));
            let expected_storage_cost = 911000;
            let expected_synthesis_cost = 603500;
            let expected_constructor_cost = 182000;
            let expected_namespace_cost = 1000000;
            let expected_total_cost =
                expected_storage_cost + expected_synthesis_cost + expected_constructor_cost + expected_namespace_cost;
            assert_eq!(
                deployment_cost_v1(&process, &deployment_2).unwrap(),
                (
                    expected_total_cost,
                    (
                        expected_storage_cost,
                        expected_synthesis_cost,
                        expected_constructor_cost,
                        expected_namespace_cost
                    )
                )
            );
            let expected_synthesis_cost = expected_synthesis_cost / N::ARC_0005_COMPUTE_DISCOUNT;
            let expected_constructor_cost = expected_constructor_cost / N::ARC_0005_COMPUTE_DISCOUNT;
            let expected_total_cost =
                expected_storage_cost + expected_synthesis_cost + expected_constructor_cost + expected_namespace_cost;
            assert_eq!(
                deployment_cost_v2(&process, &deployment_2).unwrap(),
                (
                    expected_total_cost,
                    (
                        expected_storage_cost,
                        expected_synthesis_cost,
                        expected_constructor_cost,
                        expected_namespace_cost
                    )
                )
            );

            let mut deployment_3 = process.deploy::<A, _>(&program_3, rng).unwrap();
            deployment_3.set_program_checksum_raw(Some(deployment_3.program().to_checksum()));
            deployment_3.set_program_owner_raw(Some(Address::rand(rng)));
            let expected_storage_cost = 943000;
            let expected_synthesis_cost = 603500;
            let expected_constructor_cost = 1640000;
            let expected_namespace_cost = 1000000;
            let expected_total_cost =
                expected_storage_cost + expected_synthesis_cost + expected_constructor_cost + expected_namespace_cost;
            assert_eq!(
                deployment_cost_v1(&process, &deployment_3).unwrap(),
                (
                    expected_total_cost,
                    (
                        expected_storage_cost,
                        expected_synthesis_cost,
                        expected_constructor_cost,
                        expected_namespace_cost
                    )
                )
            );
            let expected_synthesis_cost = expected_synthesis_cost / N::ARC_0005_COMPUTE_DISCOUNT;
            let expected_constructor_cost = expected_constructor_cost / N::ARC_0005_COMPUTE_DISCOUNT;
            let expected_total_cost =
                expected_storage_cost + expected_synthesis_cost + expected_constructor_cost + expected_namespace_cost;
            assert_eq!(
                deployment_cost_v2(&process, &deployment_3).unwrap(),
                (
                    expected_total_cost,
                    (
                        expected_storage_cost,
                        expected_synthesis_cost,
                        expected_constructor_cost,
                        expected_namespace_cost
                    )
                )
            );
        }

        // Run the tests for all networks.
        run_test::<CanaryV0, AleoCanaryV0>();
        run_test::<MainnetV0, AleoV0>();
        run_test::<TestnetV0, AleoTestnetV0>();
    }
}
