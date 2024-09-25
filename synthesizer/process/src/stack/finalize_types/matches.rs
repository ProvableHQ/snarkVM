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

use super::*;

impl<N: Network> FinalizeTypes<N> {
    /// Checks that the given operands matches the layout of the struct. The ordering of the operands matters.
    pub fn matches_struct(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        operands: &[Operand<N>],
        struct_: &StructType<N>,
    ) -> Result<()> {
        // Retrieve the struct name.
        let struct_name = struct_.name();
        // Ensure the struct name is valid.
        ensure!(!Program::is_reserved_keyword(struct_name), "Struct name '{struct_name}' is reserved");

        // Ensure the operands length is at least the minimum required.
        if operands.len() < N::MIN_STRUCT_ENTRIES {
            bail!("'{struct_name}' must have at least {} operand(s)", N::MIN_STRUCT_ENTRIES)
        }
        // Ensure the number of struct members does not exceed the maximum.
        if operands.len() > N::MAX_STRUCT_ENTRIES {
            bail!("'{struct_name}' cannot exceed {} entries", N::MAX_STRUCT_ENTRIES)
        }

        // Ensure the number of struct members match.
        let num_members = operands.len();
        let expected_num_members = struct_.members().len();
        if expected_num_members != num_members {
            bail!("'{struct_name}' expected {expected_num_members} members, found {num_members} members")
        }

        // Ensure the operand types match the struct.
        for (operand, (member_name, member_type)) in operands.iter().zip_eq(struct_.members()) {
            match operand {
                // Ensure the literal type matches the member type.
                Operand::Literal(literal) => {
                    ensure!(
                        &PlaintextType::Literal(literal.to_type()) == member_type,
                        "Struct member '{struct_name}.{member_name}' expects a {member_type}, but found '{operand}' in the operand.",
                    )
                }
                // Ensure the type of the register matches the member type.
                Operand::Register(register) => {
                    // Retrieve the type.
                    let plaintext_type = match self.get_type(stack, register)? {
                        // If the register is a plaintext type, return it.
                        FinalizeType::Plaintext(plaintext_type) => plaintext_type,
                        // If the register is a future, throw an error.
                        FinalizeType::Future(..) => bail!("Struct member cannot be a future"),
                    };
                    // Ensure the register type matches the member type.
                    ensure!(
                        &plaintext_type == member_type,
                        "Struct member '{struct_name}.{member_name}' expects {member_type}, but found '{plaintext_type}' in the operand '{operand}'.",
                    )
                }
                // Ensure the program ID type (address) matches the member type.
                Operand::ProgramID(..) => {
                    // Retrieve the program ID type.
                    let program_ref_type = PlaintextType::Literal(LiteralType::Address);
                    // Ensure the program ID type matches the member type.
                    ensure!(
                        &program_ref_type == member_type,
                        "Struct member '{struct_name}.{member_name}' expects {member_type}, but found '{program_ref_type}' in the operand '{operand}'.",
                    )
                }
                // If the operand is a signer, throw an error.
                Operand::Signer => bail!(
                    "Struct member '{struct_name}.{member_name}' cannot be cast from a signer in a finalize scope."
                ),
                // If the operand is a caller, throw an error.
                Operand::Caller => bail!(
                    "Struct member '{struct_name}.{member_name}' cannot be cast from a caller in a finalize scope."
                ),
                // Ensure the block height type (u32) matches the member type.
                Operand::BlockHeight => {
                    // Retrieve the block height type.
                    let block_height_type = PlaintextType::Literal(LiteralType::U32);
                    // Ensure the block height type matches the member type.
                    ensure!(
                        &block_height_type == member_type,
                        "Struct member '{struct_name}.{member_name}' expects {member_type}, but found '{block_height_type}' in the operand '{operand}'.",
                    )
                }
                // Ensure the network ID type (u16) matches the member type.
                Operand::NetworkID => {
                    // Retrieve the network ID type.
                    let network_id_type = PlaintextType::Literal(LiteralType::U16);
                    // Ensure the network ID type matches the member type.
                    ensure!(
                        &network_id_type == member_type,
                        "Struct member '{struct_name}.{member_name}' expects {member_type}, but found '{network_id_type}' in the operand '{operand}'.",
                    )
                }
            }
        }
        Ok(())
    }

    /// Checks that the given operands matches the layout of the array.
    pub fn matches_array(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        operands: &[Operand<N>],
        array_type: &ArrayType<N>,
    ) -> Result<()> {
        // Ensure the operands length is at least the minimum required.
        if operands.len() < N::MIN_ARRAY_ELEMENTS {
            bail!("'{array_type}' must have at least {} operand(s)", N::MIN_ARRAY_ELEMENTS)
        }
        // Ensure the number of elements not exceed the maximum.
        if operands.len() > N::MAX_ARRAY_ELEMENTS {
            bail!("'{array_type}' cannot exceed {} elements", N::MAX_ARRAY_ELEMENTS)
        }

        // Ensure the number of operands matches the length of the array.
        let num_elements = operands.len();
        let expected_num_elements = **array_type.length() as usize;
        if expected_num_elements != num_elements {
            bail!("'{array_type}' expected {expected_num_elements} elements, found {num_elements} elements")
        }

        // Ensure the operand types match the element type.
        for operand in operands.iter() {
            match operand {
                // Ensure the literal type matches the member type.
                Operand::Literal(literal) => {
                    ensure!(
                        &PlaintextType::Literal(literal.to_type()) == array_type.next_element_type(),
                        "Array element expects {}, but found '{operand}' in the operand.",
                        array_type.next_element_type()
                    )
                }
                // Ensure the type of the register matches the member type.
                Operand::Register(register) => {
                    // Retrieve the type.
                    let plaintext_type = match self.get_type(stack, register)? {
                        // If the register is a plaintext type, return it.
                        FinalizeType::Plaintext(plaintext_type) => plaintext_type,
                        // If the register is a future, throw an error.
                        FinalizeType::Future(..) => bail!("Array element cannot be a future"),
                    };
                    // Ensure the register type matches the member type.
                    ensure!(
                        &plaintext_type == array_type.next_element_type(),
                        "Array element expects {}, but found '{plaintext_type}' in the operand '{operand}'.",
                        array_type.next_element_type()
                    )
                }
                // Ensure the program ID type (address) matches the member type.
                Operand::ProgramID(..) => {
                    // Retrieve the program ID type.
                    let program_ref_type = PlaintextType::Literal(LiteralType::Address);
                    // Ensure the program ID type matches the member type.
                    ensure!(
                        &program_ref_type == array_type.next_element_type(),
                        "Array element expects {}, but found '{program_ref_type}' in the operand '{operand}'.",
                        array_type.next_element_type()
                    )
                }
                // If the operand is a signer, throw an error.
                Operand::Signer => bail!("Array element cannot be cast from a signer in a finalize scope."),
                // If the operand is a caller, throw an error.
                Operand::Caller => bail!("Array element cannot be cast from a caller in a finalize scope."),
                // Ensure the block height type (u32) matches the member type.
                Operand::BlockHeight => {
                    // Retrieve the block height type.
                    let block_height_type = PlaintextType::Literal(LiteralType::U32);
                    // Ensure the block height type matches the member type.
                    ensure!(
                        &block_height_type == array_type.next_element_type(),
                        "Array element expects {}, but found '{block_height_type}' in the operand '{operand}'.",
                        array_type.next_element_type()
                    )
                }
                // Ensure the network ID type (u16) matches the member type.
                Operand::NetworkID => {
                    // Retrieve the network ID type.
                    let network_id_type = PlaintextType::Literal(LiteralType::U16);
                    // Ensure the network ID type matches the member type.
                    ensure!(
                        &network_id_type == array_type.next_element_type(),
                        "Array element expects {}, but found '{network_id_type}' in the operand '{operand}'.",
                        array_type.next_element_type()
                    )
                }
            }
        }
        Ok(())
    }
}
