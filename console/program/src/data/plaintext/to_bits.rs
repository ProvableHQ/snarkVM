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

use super::*;

impl<N: Network> ToBits for Plaintext<N> {
    /// Returns this plaintext as a list of **little-endian** bits.
    fn write_bits_le(&self, vec: &mut Vec<bool>) {
        match self {
            Self::Literal(literal, bits_le) => {
                // Compute the bits.
                let bits = bits_le.get_or_init(|| {
                    let mut bits_le = PlaintextType::<N>::LITERAL_PREFIX_BITS.to_vec(); // Variant bits.
                    literal.variant().write_bits_le(&mut bits_le);
                    literal.size_in_bits().write_bits_le(&mut bits_le);
                    literal.write_bits_le(&mut bits_le);
                    bits_le
                });
                // Extend the vector with the bits.
                vec.extend_from_slice(bits)
            }
            Self::Struct(struct_, bits_le) => {
                // Compute the bits.
                let bits = bits_le.get_or_init(|| {
                    let mut bits_le = PlaintextType::<N>::STRUCT_PREFIX_BITS.to_vec(); // Variant bits.

                    // Write the length of the struct.
                    u8::try_from(struct_.len())
                        .or_halt_with::<N>("Plaintext struct length exceeds u8::MAX")
                        .write_bits_le(&mut bits_le);

                    // Write each member of the struct.
                    for (identifier, value) in struct_ {
                        // Write the identifier of the member.
                        identifier.size_in_bits().write_bits_le(&mut bits_le);
                        identifier.write_bits_le(&mut bits_le);

                        // Write the value of the member.
                        let value_bits = value.to_bits_le();
                        u16::try_from(value_bits.len())
                            .or_halt_with::<N>("Plaintext member exceeds u16::MAX bits")
                            .write_bits_le(&mut bits_le);
                        bits_le.extend_from_slice(&value_bits);
                    }
                    bits_le
                });
                // Extend the vector with the bits.
                vec.extend_from_slice(bits)
            }
            Self::Array(array, bits_le) => {
                // Compute the bits.
                let bits = bits_le.get_or_init(|| {
                    let mut bits_le = PlaintextType::<N>::ARRAY_PREFIX_BITS.to_vec(); // Variant bits.

                    // Write the length of the array.
                    u32::try_from(array.len())
                        .or_halt_with::<N>("Plaintext array length exceeds u32::MAX")
                        .write_bits_le(&mut bits_le);

                    // Write each element of the array.
                    for element in array {
                        let element_bits = element.to_bits_le();

                        // Write the size of the element.
                        u16::try_from(element_bits.len())
                            .or_halt_with::<N>("Plaintext element exceeds u16::MAX bits")
                            .write_bits_le(&mut bits_le);

                        // Write the element.
                        bits_le.extend(element_bits);
                    }
                    bits_le
                });
                // Extend the vector with the bits.
                vec.extend_from_slice(bits)
            }
        }
    }

    /// Returns this plaintext as a list of **big-endian** bits.
    fn write_bits_be(&self, vec: &mut Vec<bool>) {
        match self {
            Self::Literal(literal, bits_be) => {
                // Compute the bits.
                let bits = bits_be.get_or_init(|| {
                    let mut bits_be = PlaintextType::<N>::LITERAL_PREFIX_BITS.to_vec(); // Variant bits.
                    literal.variant().write_bits_be(&mut bits_be);
                    literal.size_in_bits().write_bits_be(&mut bits_be);
                    literal.write_bits_be(&mut bits_be);
                    bits_be
                });
                // Extend the vector with the bits.
                vec.extend_from_slice(bits)
            }
            Self::Struct(struct_, bits_be) => {
                // Compute the bits.
                let bits = bits_be.get_or_init(|| {
                    let mut bits_be = PlaintextType::<N>::STRUCT_PREFIX_BITS.to_vec(); // Variant bits.

                    // Write the length of the struct.
                    u8::try_from(struct_.len())
                        .or_halt_with::<N>("Plaintext struct length exceeds u8::MAX")
                        .write_bits_be(&mut bits_be);

                    // Write each member of the struct.
                    for (identifier, value) in struct_ {
                        // Write the identifier of the member.
                        identifier.size_in_bits().write_bits_be(&mut bits_be);
                        identifier.write_bits_be(&mut bits_be);

                        // Write the value of the member.
                        let value_bits = value.to_bits_be();
                        u16::try_from(value_bits.len())
                            .or_halt_with::<N>("Plaintext member exceeds u16::MAX bits")
                            .write_bits_be(&mut bits_be);
                        bits_be.extend_from_slice(&value_bits);
                    }

                    bits_be
                });
                // Extend the vector with the bits.
                vec.extend_from_slice(bits)
            }
            Self::Array(array, bits_be) => {
                // Compute the bits.
                let bits = bits_be.get_or_init(|| {
                    let mut bits_be = PlaintextType::<N>::ARRAY_PREFIX_BITS.to_vec(); // Variant bits.

                    // Write the length of the array.
                    u32::try_from(array.len())
                        .or_halt_with::<N>("Plaintext array length exceeds u32::MAX")
                        .write_bits_be(&mut bits_be);

                    // Write each element of the array.
                    for element in array {
                        let element_bits = element.to_bits_be();

                        // Write the size of the element.
                        u16::try_from(element_bits.len())
                            .or_halt_with::<N>("Plaintext element exceeds u16::MAX bits")
                            .write_bits_be(&mut bits_be);

                        // Write the element.
                        bits_be.extend(element_bits);
                    }
                    bits_be
                });
                // Extend the vector with the bits.
                vec.extend_from_slice(bits)
            }
        }
    }
}

impl<N: Network> Plaintext<N> {
    /// Returns the number of bits in the little-endian bit encoding of this plaintext,
    /// or an error when the encoding does not fit a size prefix.
    ///
    /// This traversal mirrors `write_bits_le` above, which halts on the same conditions,
    /// and must stay in sync with it. Decoders must run this check on untrusted values
    /// before they merkleize them, so that oversized values are rejected instead of
    /// halting the process.
    pub(in crate::data) fn size_in_bits_checked(&self) -> Result<usize> {
        match self {
            // The encoding is the variant tag, a u16 size, and the literal value.
            Self::Literal(literal, ..) => {
                // Account for the plaintext variant bits.
                let mut total = PlaintextType::<N>::LITERAL_PREFIX_BITS.len();
                // Account for the literal variant bits, and the size of the literal in bits.
                total = total.checked_add(8).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                total = total.checked_add(16).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                // Account for the literal.
                total =
                    total.checked_add(literal.size_in_bits() as usize).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                Ok(total)
            }
            // The encoding is the variant tag, a u8 member count, then per member the
            // identifier size, the identifier, a u16 size, and the member.
            Self::Struct(members, ..) => {
                // Ensure the member count fits the u8 count prefix.
                ensure!(members.len() <= u8::MAX as usize, "Plaintext struct length exceeds u8::MAX");
                // Account for the plaintext variant bits and the member count.
                let mut total = PlaintextType::<N>::STRUCT_PREFIX_BITS.len();
                total = total.checked_add(8).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                // Add up the sizes of each member.
                for (name, member) in members {
                    // Account for the identifier size and the identifier.
                    total = total.checked_add(8).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                    total =
                        total.checked_add(name.size_in_bits() as usize).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                    // Account for the member, ensuring its size fits the u16 size prefix.
                    total = total.checked_add(16).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                    let member_size = member.size_in_bits_checked()?;
                    ensure!(member_size <= u16::MAX as usize, "Plaintext member exceeds u16::MAX bits");
                    total = total.checked_add(member_size).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                }
                Ok(total)
            }
            // The encoding is the variant tag, a u32 element count, then per element a u16
            // size and the element.
            Self::Array(elements, ..) => {
                // Ensure the element count fits the u32 count prefix.
                ensure!(elements.len() <= u32::MAX as usize, "Plaintext array length exceeds u32::MAX");
                // Account for the plaintext variant bits and the element count.
                let mut total = PlaintextType::<N>::ARRAY_PREFIX_BITS.len();
                total = total.checked_add(32).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                // Add up the sizes of each element.
                for element in elements {
                    // Account for the element, ensuring its size fits the u16 size prefix.
                    total = total.checked_add(16).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                    let element_size = element.size_in_bits_checked()?;
                    ensure!(element_size <= u16::MAX as usize, "Plaintext element exceeds u16::MAX bits");
                    total = total.checked_add(element_size).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                }
                Ok(total)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;
    use snarkvm_console_types::prelude::*;

    use core::str::FromStr;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_size_in_bits_checked_matches_to_bits_le() {
        let rng = &mut TestRng::default();

        // A literal.
        let literal = Plaintext::<CurrentNetwork>::from(Literal::U64(U64::rand(rng)));
        assert_eq!(literal.size_in_bits_checked().unwrap(), literal.to_bits_le().len());

        // A struct with a literal member, a struct member, and an array member.
        let struct_ = Plaintext::<CurrentNetwork>::Struct(
            indexmap::indexmap! {
                Identifier::<CurrentNetwork>::from_str("a").unwrap() => Plaintext::from(Literal::U64(U64::rand(rng))),
                Identifier::<CurrentNetwork>::from_str("b").unwrap() => Plaintext::Struct(
                    indexmap::indexmap! {
                        Identifier::<CurrentNetwork>::from_str("c").unwrap() => Plaintext::from(Literal::U8(U8::rand(rng))),
                    },
                    Default::default(),
                ),
                Identifier::<CurrentNetwork>::from_str("d").unwrap() => Plaintext::Array(
                    vec![Plaintext::from(Literal::Boolean(Boolean::rand(rng)))],
                    Default::default(),
                ),
            },
            Default::default(),
        );
        assert_eq!(struct_.size_in_bits_checked().unwrap(), struct_.to_bits_le().len());

        // An array of arrays of literals.
        let array = Plaintext::<CurrentNetwork>::Array(
            vec![
                Plaintext::Array(
                    vec![Plaintext::from(Literal::U64(U64::rand(rng))), Plaintext::from(Literal::U64(U64::rand(rng)))],
                    Default::default(),
                ),
                Plaintext::Array(vec![Plaintext::from(Literal::Field(Field::rand(rng)))], Default::default()),
            ],
            Default::default(),
        );
        assert_eq!(array.size_in_bits_checked().unwrap(), array.to_bits_le().len());
    }
}
