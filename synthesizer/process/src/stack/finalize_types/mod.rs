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

mod initialize;
mod matches;

use crate::RegisterTypes;

use console::{
    network::prelude::*,
    program::{
        Access,
        ArrayType,
        FinalizeType,
        Identifier,
        LiteralType,
        PlaintextType,
        Register,
        RegisterType,
        StructType,
    },
    types::U32,
};
use synthesizer_program::{
    Await,
    Branch,
    CallOperator,
    CastType,
    Command,
    CommandTrait,
    Constructor,
    Contains,
    Finalize,
    Get,
    GetOrUse,
    Instruction,
    InstructionTrait,
    MAX_ADDITIONAL_SEEDS,
    Opcode,
    Operand,
    Program,
    RandChaCha,
    Remove,
    Set,
    StackMatches,
    StackProgram,
};

use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Default, PartialEq, Eq)]
pub struct FinalizeTypes<N: Network> {
    /// The mapping of all input registers to their defined types.
    /// Note that in a finalize context, all registers are finalize types.
    inputs: IndexMap<u64, FinalizeType<N>>,
    /// The mapping of all destination registers to their defined types.
    /// Note that in a finalize context, all registers are finalize types.
    destinations: IndexMap<u64, FinalizeType<N>>,
}

impl<N: Network> FinalizeTypes<N> {
    /// Initializes a new instance of `FinalizeTypes` for the given constructor.
    /// Checks that the given constructor is well-formed for the given stack.
    #[inline]
    pub fn from_constructor(
        stack: &(impl StackMatches<N> + StackProgram<N>),
        constructor: &Constructor<N>,
    ) -> Result<Self> {
        Self::initialize_finalize_types_from_constructor(stack, constructor)
    }

    /// Initializes a new instance of `FinalizeTypes` for the given finalize.
    /// Checks that the given finalize is well-formed for the given stack.
    #[inline]
    pub fn from_finalize(stack: &(impl StackMatches<N> + StackProgram<N>), finalize: &Finalize<N>) -> Result<Self> {
        Self::initialize_finalize_types_from_finalize(stack, finalize)
    }

    /// Returns `true` if the given register exists.
    pub fn contains(&self, register: &Register<N>) -> bool {
        // Retrieve the register locator.
        let locator = &register.locator();
        // The input and destination registers represent the full set of registers.
        // The output registers represent a subset of registers that are returned by the function.
        self.inputs.contains_key(locator) || self.destinations.contains_key(locator)
    }

    /// Returns `true` if the given register corresponds to an input register.
    pub fn is_input(&self, register: &Register<N>) -> bool {
        self.inputs.contains_key(&register.locator())
    }

    /// Returns the type of the given operand.
    pub fn get_type_from_operand(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        operand: &Operand<N>,
    ) -> Result<FinalizeType<N>> {
        Ok(match operand {
            Operand::Literal(literal) => FinalizeType::Plaintext(PlaintextType::from(literal.to_type())),
            Operand::Register(register) => self.get_type(stack, register)?,
            Operand::ProgramID(_) => FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::Address)),
            Operand::Signer => bail!("'self.signer' is not a valid operand in a finalize context."),
            Operand::Caller => bail!("'self.caller' is not a valid operand in a finalize context."),
            Operand::BlockHeight => FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::U32)),
            Operand::NetworkID => FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::U16)),
            Operand::Checksum(_) => FinalizeType::Plaintext(PlaintextType::Array(ArrayType::new(
                PlaintextType::Literal(LiteralType::U8),
                vec![U32::new(32)],
            )?)),
            Operand::Edition(_) => FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::U16)),
            Operand::ProgramOwner(_) => FinalizeType::Plaintext(PlaintextType::Literal(LiteralType::Address)),
        })
    }

    /// Returns the type of the given register.
    pub fn get_type(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        register: &Register<N>,
    ) -> Result<FinalizeType<N>> {
        // Initialize a tracker for the type of the register.
        let finalize_type = if self.is_input(register) {
            // Retrieve the input value type as a register type.
            self.inputs.get(&register.locator()).ok_or_else(|| anyhow!("Register '{register}' does not exist"))?
        } else {
            // Retrieve the destination register type.
            self.destinations.get(&register.locator()).ok_or_else(|| anyhow!("Register '{register}' does not exist"))?
        };

        // Retrieve the path if the register is an access. Otherwise, return the type.
        let (mut finalize_type, path) = match (finalize_type, register) {
            // If the register is a locator, then output the register type.
            (_, Register::Locator(..)) => return Ok(finalize_type.clone()),
            // If the register is an access, then traverse the path to output the register type.
            (finalize_type, Register::Access(_, path)) => {
                // Ensure the path is valid.
                ensure!(!path.is_empty(), "Register '{register}' references no accesses.");
                // Return the finalize type and path.
                (finalize_type.clone(), path)
            }
        };

        // Traverse the path to find the register type.
        for access in path.iter() {
            match (&finalize_type, access) {
                // Ensure the plaintext type is not a literal, as the register references an access.
                (FinalizeType::Plaintext(PlaintextType::Literal(..)), _) => {
                    bail!("'{register}' references a literal.")
                }
                // Access the member on the path to output the register type.
                (FinalizeType::Plaintext(PlaintextType::Struct(struct_name)), Access::Member(identifier)) => {
                    // Retrieve the member type from the struct and check that it exists.
                    match stack.program().get_struct(struct_name)?.members().get(identifier) {
                        // Retrieve the member and update `finalize_type` for the next iteration.
                        Some(member_type) => finalize_type = FinalizeType::Plaintext(member_type.clone()),
                        // Halts if the member does not exist.
                        None => bail!("'{identifier}' does not exist in struct '{struct_name}'"),
                    }
                }
                // Access the member on the path to output the register type and check that it is in bounds.
                (FinalizeType::Plaintext(PlaintextType::Array(array_type)), Access::Index(index)) => {
                    match index < array_type.length() {
                        // Retrieve the element type and update `finalize_type` for the next iteration.
                        true => finalize_type = FinalizeType::Plaintext(array_type.next_element_type().clone()),
                        // Halts if the index is out of bounds.
                        false => bail!("Index out of bounds"),
                    }
                }
                // Access the input to the future to output the register type and check that it is in bounds.
                (FinalizeType::Future(locator), Access::Index(index)) => {
                    // Get the external stack, if needed.
                    let external_stack = match locator.program_id() == stack.program_id() {
                        true => None,
                        // Attention - This method must fail here and early return if the external program is missing.
                        // Otherwise, this method will proceed to look for the requested function in its own program.
                        false => Some(stack.get_external_stack(locator.program_id())?),
                    };
                    // Retrieve the associated function.
                    let function = match &external_stack {
                        Some(external_stack) => external_stack.get_function_ref(locator.resource())?,
                        None => stack.get_function_ref(locator.resource())?,
                    };
                    // Retrieve the finalize inputs.
                    let finalize_inputs = match function.finalize_logic() {
                        Some(finalize_logic) => finalize_logic.inputs(),
                        None => bail!("Function '{locator}' does not have a finalize block"),
                    };
                    // Check that the index is in bounds.
                    match finalize_inputs.get_index(**index as usize) {
                        // Retrieve the input type and update `finalize_type` for the next iteration.
                        Some(input) => finalize_type = input.finalize_type().clone(),
                        // Halts if the index is out of bounds.
                        None => bail!("Index out of bounds"),
                    }
                }
                (FinalizeType::Plaintext(PlaintextType::Struct(..)), Access::Index(..))
                | (FinalizeType::Plaintext(PlaintextType::Array(..)), Access::Member(..))
                | (FinalizeType::Future(..), Access::Member(..)) => {
                    bail!("Invalid access `{access}`")
                }
            }
        }

        // Return the output type.
        Ok(finalize_type)
    }
}
