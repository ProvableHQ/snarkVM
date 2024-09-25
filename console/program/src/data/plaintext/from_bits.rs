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

impl<N: Network> FromBits for Plaintext<N> {
    /// Initializes a new plaintext from a list of little-endian bits *without* trailing zeros.
    fn from_bits_le(bits_le: &[bool]) -> Result<Self> {
        let bits = bits_le;

        // The starting index used to create subsequent subslices of the `bits` slice.
        let mut index = 0;

        // Helper function to get the next n bits as a slice.
        let mut next_bits = |n: usize| -> Result<&[bool]> {
            // Safely procure a subslice with the length `n` starting at `index`.
            let subslice = bits.get(index..index + n);
            // Check if the range is within bounds.
            if let Some(next_bits) = subslice {
                // Move the starting index.
                index += n;
                // Return the subslice.
                Ok(next_bits)
            } else {
                bail!("Insufficient bits");
            }
        };

        let variant = next_bits(2)?;
        let variant = [variant[0], variant[1]];

        // Literal
        if variant == [false, false] {
            let literal_variant = u8::from_bits_le(next_bits(8)?)?;
            let literal_size = u16::from_bits_le(next_bits(16)?)?;
            let literal = Literal::from_bits_le(literal_variant, next_bits(literal_size as usize)?)?;

            // Cache the plaintext bits, and return the literal.
            Ok(Self::Literal(literal, OnceCell::with_value(bits_le.to_vec())))
        }
        // Struct
        else if variant == [false, true] {
            let num_members = u8::from_bits_le(next_bits(8)?)?;
            if num_members as usize > N::MAX_STRUCT_ENTRIES {
                bail!("Struct exceeds maximum of entries.");
            }

            let mut members = IndexMap::with_capacity(num_members as usize);
            for _ in 0..num_members {
                let identifier_size = u8::from_bits_le(next_bits(8)?)?;
                let identifier = Identifier::from_bits_le(next_bits(identifier_size as usize)?)?;

                let member_size = u16::from_bits_le(next_bits(16)?)?;
                let value = Plaintext::from_bits_le(next_bits(member_size as usize)?)?;

                if members.insert(identifier, value).is_some() {
                    bail!("Duplicate identifier in struct.");
                }
            }

            // Cache the plaintext bits, and return the struct.
            Ok(Self::Struct(members, OnceCell::with_value(bits_le.to_vec())))
        }
        // Array
        else if variant == [true, false] {
            let num_elements = u32::from_bits_le(next_bits(32)?)?;
            if num_elements as usize > N::MAX_ARRAY_ELEMENTS {
                bail!("Array exceeds maximum of elements.");
            }

            let mut elements = Vec::with_capacity(num_elements as usize);
            for _ in 0..num_elements {
                let element_size = u16::from_bits_le(next_bits(16)?)?;
                let element = Plaintext::from_bits_le(next_bits(element_size as usize)?)?;

                elements.push(element);
            }

            // Cache the plaintext bits, and return the array.
            Ok(Self::Array(elements, OnceCell::with_value(bits_le.to_vec())))
        }
        // Unknown variant.
        else {
            bail!("Unknown plaintext variant - {variant:?}");
        }
    }

    /// Initializes a new plaintext from a list of big-endian bits *without* trailing zeros.
    fn from_bits_be(bits_be: &[bool]) -> Result<Self> {
        let bits = bits_be;

        // The starting index used to create subsequent subslices of the `bits` slice.
        let mut index = 0;

        // Helper function to get the next n bits as a slice.
        let mut next_bits = |n: usize| -> Result<&[bool]> {
            // Safely procure a subslice with the length `n` starting at `index`.
            let subslice = bits.get(index..index + n);
            // Check if the range is within bounds.
            if let Some(next_bits) = subslice {
                // Move the starting index.
                index += n;
                // Return the subslice.
                Ok(next_bits)
            } else {
                bail!("Insufficient bits");
            }
        };

        let variant = next_bits(2)?;
        let variant = [variant[0], variant[1]];

        // Literal
        if variant == [false, false] {
            let literal_variant = u8::from_bits_be(next_bits(8)?)?;
            let literal_size = u16::from_bits_be(next_bits(16)?)?;
            let literal = Literal::from_bits_be(literal_variant, next_bits(literal_size as usize)?)?;

            // Cache the plaintext bits, and return the literal.
            Ok(Self::Literal(literal, OnceCell::with_value(bits_be.to_vec())))
        }
        // Struct
        else if variant == [false, true] {
            let num_members = u8::from_bits_be(next_bits(8)?)?;
            if num_members as usize > N::MAX_STRUCT_ENTRIES {
                bail!("Struct exceeds maximum of entries.");
            }

            let mut members = IndexMap::with_capacity(num_members as usize);
            for _ in 0..num_members {
                let identifier_size = u8::from_bits_be(next_bits(8)?)?;
                let identifier = Identifier::from_bits_be(next_bits(identifier_size as usize)?)?;

                let member_size = u16::from_bits_be(next_bits(16)?)?;
                let value = Plaintext::from_bits_be(next_bits(member_size as usize)?)?;

                if members.insert(identifier, value).is_some() {
                    bail!("Duplicate identifier in struct.");
                }
            }

            // Cache the plaintext bits, and return the struct.
            Ok(Self::Struct(members, OnceCell::with_value(bits_be.to_vec())))
        }
        // Array
        else if variant == [true, false] {
            let num_elements = u32::from_bits_be(next_bits(32)?)?;
            if num_elements as usize > N::MAX_ARRAY_ELEMENTS {
                bail!("Array exceeds maximum of elements.");
            }

            let mut elements = Vec::with_capacity(num_elements as usize);
            for _ in 0..num_elements {
                let element_size = u16::from_bits_be(next_bits(16)?)?;
                let element = Plaintext::from_bits_be(next_bits(element_size as usize)?)?;

                elements.push(element);
            }

            // Cache the plaintext bits, and return the array.
            Ok(Self::Array(elements, OnceCell::with_value(bits_be.to_vec())))
        }
        // Unknown variant.
        else {
            bail!("Unknown plaintext variant - {variant:?}");
        }
    }
}
