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

impl<A: Aleo> ToBits for Plaintext<A> {
    type Boolean = Boolean<A>;

    /// Returns this plaintext as a list of **little-endian** bits.
    fn write_bits_le(&self, vec: &mut Vec<Boolean<A>>) {
        match self {
            Self::Literal(literal, bits_le) => {
                // Compute the bits of the literal.
                let bits = bits_le.get_or_init(|| {
                    let mut bits_le = vec![Boolean::constant(false), Boolean::constant(false)]; // Variant bit.
                    literal.variant().write_bits_le(&mut bits_le);
                    literal.size_in_bits().write_bits_le(&mut bits_le);
                    literal.write_bits_le(&mut bits_le);
                    bits_le
                });
                // Extend the vector with the bits of the literal.
                vec.extend_from_slice(bits);
            }
            Self::Struct(members, bits_le) => {
                // Compute the bits of the struct.
                let bits = bits_le.get_or_init(|| {
                    let mut bits_le = vec![Boolean::constant(false), Boolean::constant(true)]; // Variant bit.
                    U8::constant(console::U8::new(members.len() as u8)).write_bits_le(&mut bits_le);
                    for (identifier, value) in members {
                        let value_bits = value.to_bits_le();
                        identifier.size_in_bits().write_bits_le(&mut bits_le);
                        identifier.write_bits_le(&mut bits_le);
                        U16::constant(console::U16::new(value_bits.len() as u16)).write_bits_le(&mut bits_le);
                        bits_le.extend_from_slice(&value_bits);
                    }
                    bits_le
                });
                // Extend the vector with the bits of the struct.
                vec.extend_from_slice(bits);
            }
            Self::Array(elements, bits_le) => {
                // Compute the bits of the array.
                let bits = bits_le.get_or_init(|| {
                    let mut bits_le = vec![Boolean::constant(true), Boolean::constant(false)]; // Variant bit.
                    U32::constant(console::U32::new(elements.len() as u32)).write_bits_le(&mut bits_le);
                    for value in elements {
                        let value_bits = value.to_bits_le();
                        U16::constant(console::U16::new(value_bits.len() as u16)).write_bits_le(&mut bits_le);
                        bits_le.extend(value_bits);
                    }
                    bits_le
                });
                // Extend the vector with the bits of the array.
                vec.extend_from_slice(bits);
            }
        }
    }

    /// Returns this plaintext as a list of **big-endian** bits.
    fn write_bits_be(&self, vec: &mut Vec<Boolean<A>>) {
        match self {
            Self::Literal(literal, bits_be) => {
                // Compute the bits of the literal.
                let bits = bits_be.get_or_init(|| {
                    let mut bits_be = vec![Boolean::constant(false), Boolean::constant(false)]; // Variant bit.
                    literal.variant().write_bits_be(&mut bits_be);
                    literal.size_in_bits().write_bits_be(&mut bits_be);
                    literal.write_bits_be(&mut bits_be);
                    bits_be
                });
                // Extend the vector with the bits of the literal.
                vec.extend_from_slice(bits);
            }
            Self::Struct(members, bits_be) => {
                // Compute the bits of the struct.
                let bits = bits_be.get_or_init(|| {
                    let mut bits_be = vec![Boolean::constant(false), Boolean::constant(true)]; // Variant bit.
                    U8::constant(console::U8::new(members.len() as u8)).write_bits_be(&mut bits_be);
                    for (identifier, value) in members {
                        let value_bits = value.to_bits_be();
                        identifier.size_in_bits().write_bits_be(&mut bits_be);
                        identifier.write_bits_be(&mut bits_be);
                        U16::constant(console::U16::new(value_bits.len() as u16)).write_bits_be(&mut bits_be);
                        bits_be.extend_from_slice(&value_bits);
                    }
                    bits_be
                });
                // Extend the vector with the bits of the struct.
                vec.extend_from_slice(bits)
            }
            Self::Array(elements, bits_be) => {
                // Compute the bits of the array.
                let bits = bits_be.get_or_init(|| {
                    let mut bits_be = vec![Boolean::constant(true), Boolean::constant(false)]; // Variant bit.
                    U32::constant(console::U32::new(elements.len() as u32)).write_bits_be(&mut bits_be);
                    for value in elements {
                        let value_bits = value.to_bits_be();
                        U16::constant(console::U16::new(value_bits.len() as u16)).write_bits_be(&mut bits_be);
                        bits_be.extend(value_bits);
                    }
                    bits_be
                });
                // Extend the vector with the bits of the array.
                vec.extend_from_slice(bits)
            }
        }
    }
}
