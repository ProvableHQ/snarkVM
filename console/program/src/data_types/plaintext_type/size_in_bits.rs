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

use super::*;

impl<N: Network> PlaintextType<N> {
    /// Returns the number of bits of a plaintext type.
    pub fn size_in_bits<F0, F1>(&self, get_struct: &F0, get_external_struct: &F1) -> Result<usize>
    where
        F0: Fn(&Identifier<N>) -> Result<StructType<N>>,
        F1: Fn(&Locator<N>) -> Result<StructType<N>>,
    {
        self.size_in_bits_internal(get_struct, get_external_struct, 0)
    }

    /// A helper function to determine the number of bits of a plaintext type, while tracking the depth of the data.
    pub(crate) fn size_in_bits_internal<F0, F1>(
        &self,
        get_struct: &F0,
        get_external_struct: &F1,
        depth: usize,
    ) -> Result<usize>
    where
        F0: Fn(&Identifier<N>) -> Result<StructType<N>>,
        F1: Fn(&Locator<N>) -> Result<StructType<N>>,
    {
        // Ensure that the depth is within the maximum limit.
        ensure!(depth <= N::MAX_DATA_DEPTH, "Plaintext depth exceeds maximum limit: {}", N::MAX_DATA_DEPTH);

        match &self {
            PlaintextType::Literal(literal) => {
                // Account for the plaintext variant bits.
                let mut total = PlaintextType::<N>::LITERAL_PREFIX_BITS.len();
                // Account for the literal variant bits.
                total = total.checked_add(8).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                // Account for the size of the literal in bits.
                total = total.checked_add(16).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                // Account for the literal.

                total = total
                    .checked_add(literal.size_in_bits::<N>() as usize)
                    .ok_or(anyhow!("`size_in_bits` overflowed"))?;

                Ok(total)
            }
            PlaintextType::Struct(identifier) => {
                // Look up the struct.
                let struct_ = get_struct(identifier)?;

                // Account for the plaintext variant bits.
                let mut total = PlaintextType::<N>::STRUCT_PREFIX_BITS.len();
                // Account for the number of members in the struct.
                total = total.checked_add(8).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                // Add up the sizes of each member.
                for (identifier, member_type) in struct_.members() {
                    // Account for the size of the identifier.
                    total = total.checked_add(8).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                    // Account for the identifier.
                    total = total
                        .checked_add(identifier.size_in_bits() as usize)
                        .ok_or(anyhow!("`size_in_bits` overflowed"))?;
                    // Account for the size of the member
                    total = total.checked_add(16).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                    // Account for the member itself.
                    let member_size = member_type.size_in_bits_internal(get_struct, get_external_struct, depth + 1)?;
                    total = total.checked_add(member_size).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                }

                Ok(total)
            }
            PlaintextType::ExternalStruct(locator) => {
                // Look up the struct
                let struct_ = get_external_struct(locator)?;

                // Account for the plaintext variant bits.
                let mut total = PlaintextType::<N>::STRUCT_PREFIX_BITS.len();
                // Account for the number of members in the struct.
                total = total.checked_add(8).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                // Add up the sizes of each member.
                for (identifier, member_type) in struct_.members() {
                    // Account for the size of the identifier.
                    total = total.checked_add(8).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                    // Account for the identifier.
                    total = total
                        .checked_add(identifier.size_in_bits() as usize)
                        .ok_or(anyhow!("`size_in_bits` overflowed"))?;
                    // Account for the size of the member
                    total = total.checked_add(16).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                    // Account for the member itself.
                    let member_size = member_type.size_in_bits_internal(get_struct, get_external_struct, depth + 1)?;
                    total = total.checked_add(member_size).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                }

                Ok(total)
            }
            PlaintextType::Array(array_type) => {
                // Account for the plaintext variant bits.
                let mut total = PlaintextType::<N>::ARRAY_PREFIX_BITS.len();
                // Account for the size of the array length.
                total = total.checked_add(32).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                // Get the size of the element type.
                let element_size =
                    array_type.next_element_type().size_in_bits_internal(get_struct, get_external_struct, depth + 1)?;
                // Get the total size of an element.
                let element_total = 16usize.checked_add(element_size).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                // Multiply by the length of the array, ensuring no overflow occurs.
                total = total
                    .checked_add(
                        element_total
                            .checked_mul(**array_type.length() as usize)
                            .ok_or(anyhow!("`size_in_bits` overflowed"))?,
                    )
                    .ok_or(anyhow!("`size_in_bits` overflowed"))?;

                Ok(total)
            }
        }
    }

    /// Returns the number of raw bits of a plaintext type.
    pub fn size_in_bits_raw<F0, F1>(&self, get_struct: &F0, get_external_struct: &F1) -> Result<usize>
    where
        F0: Fn(&Identifier<N>) -> Result<StructType<N>>,
        F1: Fn(&Locator<N>) -> Result<StructType<N>>,
    {
        self.size_in_bits_raw_internal(get_struct, get_external_struct, 0)
    }

    // A helper function to determine the number of raw bits of a plaintext type, while tracking the depth of the data.
    fn size_in_bits_raw_internal<F0, F1>(
        &self,
        get_struct: &F0,
        get_external_struct: &F1,
        depth: usize,
    ) -> Result<usize>
    where
        F0: Fn(&Identifier<N>) -> Result<StructType<N>>,
        F1: Fn(&Locator<N>) -> Result<StructType<N>>,
    {
        // Ensure that the depth is within the maximum limit.
        ensure!(depth <= N::MAX_DATA_DEPTH, "Plaintext depth exceeds maximum limit: {}", N::MAX_DATA_DEPTH);

        match &self {
            PlaintextType::Literal(literal) => Ok(literal.size_in_bits::<N>() as usize),
            PlaintextType::Struct(identifier) => {
                // Look up the struct.
                let struct_ = get_struct(identifier)?;
                // Add up the sizes of each member.
                let mut total = 0usize;
                for member_type in struct_.members().values() {
                    // Get the size of the member.
                    let member_size =
                        member_type.size_in_bits_raw_internal(get_struct, get_external_struct, depth + 1)?;
                    // Add to the total size, ensuring no overflow occurs.
                    total = total.checked_add(member_size).ok_or(anyhow!("`size_in_bits_raw` overflowed"))?;
                }
                Ok(total)
            }
            PlaintextType::ExternalStruct(locator) => {
                // Look up the struct.
                let struct_ = get_external_struct(locator)?;
                // Add up the sizes of each member.
                let mut total = 0usize;
                for member_type in struct_.members().values() {
                    // Get the size of the member.
                    let member_size =
                        member_type.size_in_bits_raw_internal(get_struct, get_external_struct, depth + 1)?;
                    // Add to the total size, ensuring no overflow occurs.
                    total = total.checked_add(member_size).ok_or(anyhow!("`size_in_bits_raw` overflowed"))?;
                }
                Ok(total)
            }
            PlaintextType::Array(array_type) => {
                // Get the size of the element type.
                let element_size = array_type.next_element_type().size_in_bits_raw_internal(
                    get_struct,
                    get_external_struct,
                    depth + 1,
                )?;
                // Multiply by the length of the array, ensuring no overflow occurs.
                let total = element_size
                    .checked_mul(**array_type.length() as usize)
                    .ok_or(anyhow!("`size_in_bits_raw` overflowed"))?;

                Ok(total)
            }
        }
    }
}
