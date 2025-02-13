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

mod initialize;
mod matches;

use crate::RegisterTypes;

use console::{
    network::prelude::*,
    program::{Access, ArrayType, Identifier, LiteralType, PlaintextType, Register, RegisterType, StructType},
};
use synthesizer_program::{
    Await,
    Branch,
    CallOperator,
    CastType,
    Command,
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
use std::collections::HashSet;

// TODO (@d0cd) This can be unified with `FinalizeTypes`.

#[derive(Clone, PartialEq, Eq)]
pub struct ConstructorTypes<N: Network> {
    /// The mapping of all destination registers to their defined types.
    /// Note that in a constructor context, all registers are plaintext types.
    destinations: IndexMap<u64, PlaintextType<N>>,
}

impl<N: Network> ConstructorTypes<N> {
    /// Initializes a new instance of `ConstructorTypes` for the given constructor.
    /// Checks that the given constructor is well-formed for the given stack.
    #[inline]
    pub fn from_constructor(
        stack: &(impl StackMatches<N> + StackProgram<N>),
        finalize: &Constructor<N>,
    ) -> Result<Self> {
        Self::initialize_constructor_types(stack, finalize)
    }

    /// Returns `true` if the given register exists.
    pub fn contains(&self, register: &Register<N>) -> bool {
        // Retrieve the register locator.
        let locator = &register.locator();
        // The destination registers represent the full set of registers.
        self.destinations.contains_key(locator)
    }

    /// Returns the type of the given operand.
    pub fn get_type_from_operand(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        operand: &Operand<N>,
    ) -> Result<PlaintextType<N>> {
        Ok(match operand {
            Operand::Literal(literal) => PlaintextType::from(literal.to_type()),
            Operand::Register(register) => self.get_type(stack, register)?,
            Operand::ProgramID(_) => PlaintextType::Literal(LiteralType::Address),
            Operand::Signer => bail!("'self.signer' is not a valid operand in a finalize context."),
            Operand::Caller => bail!("'self.caller' is not a valid operand in a finalize context."),
            Operand::BlockHeight => PlaintextType::Literal(LiteralType::U32),
            Operand::NetworkID => PlaintextType::Literal(LiteralType::U16),
        })
    }

    /// Returns the type of the given register.
    pub fn get_type(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        register: &Register<N>,
    ) -> Result<PlaintextType<N>> {
        // Initialize a tracker for the type of the register.
        let plaintext_type = self
            .destinations
            .get(&register.locator())
            .ok_or_else(|| anyhow!("Register '{register}' does not exist"))?;

        // Retrieve the path if the register is an access. Otherwise, return the type.
        let (mut plaintext_type, path) = match (plaintext_type, register) {
            // If the register is a locator, then output the register type.
            (_, Register::Locator(..)) => return Ok(plaintext_type.clone()),
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
            match (&plaintext_type, access) {
                // Ensure the plaintext type is not a literal, as the register references an access.
                (PlaintextType::Literal(..), _) => {
                    bail!("'{register}' references a literal.")
                }
                // Access the member on the path to output the register type.
                (PlaintextType::Struct(struct_name), Access::Member(identifier)) => {
                    // Retrieve the member type from the struct and check that it exists.
                    match stack.program().get_struct(struct_name)?.members().get(identifier) {
                        // Retrieve the member and update `finalize_type` for the next iteration.
                        Some(member_type) => plaintext_type = member_type.clone(),
                        // Halts if the member does not exist.
                        None => bail!("'{identifier}' does not exist in struct '{struct_name}'"),
                    }
                }
                // Access the member on the path to output the register type and check that it is in bounds.
                (PlaintextType::Array(array_type), Access::Index(index)) => {
                    match index < array_type.length() {
                        // Retrieve the element type and update `finalize_type` for the next iteration.
                        true => plaintext_type = array_type.next_element_type().clone(),
                        // Halts if the index is out of bounds.
                        false => bail!("Index out of bounds"),
                    }
                }
                (PlaintextType::Struct(..), Access::Index(..)) | (PlaintextType::Array(..), Access::Member(..)) => {
                    bail!("Invalid access `{access}`")
                }
            }
        }
        // Return the output type.
        Ok(plaintext_type)
    }
}

impl<N: Network> Default for ConstructorTypes<N> {
    #[inline]
    fn default() -> Self {
        Self { destinations: IndexMap::new() }
    }
}
