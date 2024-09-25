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

use crate::synthesis::helpers::{LiteralType, PuzzleRegister};

use anyhow::{anyhow, bail, Result};
use indexmap::{indexmap, indexset, IndexMap, IndexSet};

/// A register table is used to track the registers for each literal type.
///
/// For example, the following is an example of what a register table (in use) might look like:
/// ```ignore
///  field => { r0, r1, r7, r10 }
///     u8 => { r2, r3, r6, r8 }
///    u16 => { r4, r5, r9 }
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegisterTable {
    /// The input block.
    input_block: String,
    /// The input register types (ordered).
    input_register_types: Vec<LiteralType>,
    /// The mapping of literal type to its input registers.
    input_registers: IndexMap<LiteralType, IndexSet<PuzzleRegister>>,
    /// The mapping of literal type to its registers.
    register_table: IndexMap<LiteralType, IndexSet<PuzzleRegister>>,
    /// The tracker for the next register locator.
    next_register_locator: u64,
}

impl RegisterTable {
    /// Initializes a new register table.
    pub fn new() -> Self {
        // Set the input block.
        let input_block = r"
    input r0 as boolean.public;
    input r1 as boolean.public;
    input r2 as i8.public;
    input r3 as i8.public;
    input r4 as i16.public;
    input r5 as i16.public;
    input r6 as i32.public;
    input r7 as i32.public;
    input r8 as i64.public;
    input r9 as i64.public;
    input r10 as i128.public;
    input r11 as i128.public;
    input r12 as field.public;
    input r13 as field.public;

    is.eq r1 r0 into r14;
    is.eq r3 r2 into r15;
    is.eq r5 r4 into r16;
    is.eq r7 r6 into r17;
    is.eq r9 r8 into r18;
    is.eq r11 r10 into r19;

    hash.psd2 r12 into r20 as u8;
    hash.psd2 r13 into r21 as u8;

    hash.psd2 r12 into r22 as u16;
    hash.psd2 r13 into r23 as u16;

    hash.psd2 r12 into r24 as u32;
    hash.psd2 r13 into r25 as u32;

    hash.psd2 r12 into r26 as u64;
    hash.psd2 r13 into r27 as u64;

    hash.psd2 r12 into r28 as u128;
    hash.psd2 r13 into r29 as u128;

    mul.w r3 r2 into r30;
    mul.w r5 r4 into r31;
    mul.w r7 r6 into r32;
    mul.w r9 r8 into r33;
    mul.w r11 r10 into r34;

    ternary r15 r30 r2 into r35;
    ternary r16 r31 r4 into r36;
    ternary r17 r32 r6 into r37;
    ternary r18 r33 r8 into r38;
    ternary r19 r34 r10 into r39;
"
        .to_string();

        // Set the input register types.
        let input_register_types = vec![
            LiteralType::Boolean,
            LiteralType::Boolean,
            LiteralType::I8,
            LiteralType::I8,
            LiteralType::I16,
            LiteralType::I16,
            LiteralType::I32,
            LiteralType::I32,
            LiteralType::I64,
            LiteralType::I64,
            LiteralType::I128,
            LiteralType::I128,
            LiteralType::Field,
            LiteralType::Field,
        ];

        // Construct the input registers.
        let input_registers = indexmap! {
            LiteralType::Boolean => indexset![PuzzleRegister::new_locator(0), PuzzleRegister::new_locator(1)],
            LiteralType::I8 => indexset![PuzzleRegister::new_locator(2), PuzzleRegister::new_locator(3)],
            LiteralType::I16 => indexset![PuzzleRegister::new_locator(4), PuzzleRegister::new_locator(5)],
            LiteralType::I32 => indexset![PuzzleRegister::new_locator(6), PuzzleRegister::new_locator(7)],
            LiteralType::I64 => indexset![PuzzleRegister::new_locator(8), PuzzleRegister::new_locator(9)],
            LiteralType::I128 => indexset![PuzzleRegister::new_locator(10), PuzzleRegister::new_locator(11)],
            LiteralType::Field => indexset![PuzzleRegister::new_locator(12), PuzzleRegister::new_locator(13)],
        };

        // Construct the register table.
        let register_table = indexmap! {
            LiteralType::Boolean => indexset![PuzzleRegister::new_locator(0), PuzzleRegister::new_locator(1)],
            LiteralType::I8 => indexset![PuzzleRegister::new_locator(3), PuzzleRegister::new_locator(35)],
            LiteralType::I16 => indexset![PuzzleRegister::new_locator(5), PuzzleRegister::new_locator(36)],
            LiteralType::I32 => indexset![PuzzleRegister::new_locator(7), PuzzleRegister::new_locator(37)],
            LiteralType::I64 => indexset![PuzzleRegister::new_locator(9), PuzzleRegister::new_locator(38)],
            LiteralType::I128 => indexset![PuzzleRegister::new_locator(11), PuzzleRegister::new_locator(39)],
            LiteralType::Field => indexset![PuzzleRegister::new_locator(12), PuzzleRegister::new_locator(13)],
            LiteralType::U8 => indexset![PuzzleRegister::new_locator(20), PuzzleRegister::new_locator(21)],
            LiteralType::U16 => indexset![PuzzleRegister::new_locator(22), PuzzleRegister::new_locator(23)],
            LiteralType::U32 => indexset![PuzzleRegister::new_locator(24), PuzzleRegister::new_locator(25)],
            LiteralType::U64 => indexset![PuzzleRegister::new_locator(26), PuzzleRegister::new_locator(27)],
            LiteralType::U128 => indexset![PuzzleRegister::new_locator(28), PuzzleRegister::new_locator(29)],
        };

        // Set the next register locator.
        let next_register_locator = 40;

        // Return the register table.
        Self { input_block, input_register_types, input_registers, register_table, next_register_locator }
    }
}

impl RegisterTable {
    /// Returns the input block.
    #[inline]
    pub fn input_block(&self) -> &str {
        &self.input_block
    }

    /// Returns the input register types.
    #[inline]
    pub fn input_register_types(&self) -> &[LiteralType] {
        &self.input_register_types
    }

    /// Returns and increments the next register locator.
    #[inline]
    pub fn get_next_locator(&mut self) -> Result<u64> {
        // Get the current locator.
        let locator = self.next_register_locator;
        // Update the next register locator.
        self.next_register_locator = self
            .next_register_locator
            .checked_add(1)
            .ok_or_else(|| anyhow!("Failed to update the next register locator"))?;
        // Return the result.
        Ok(locator)
    }

    /// Inserts the given literal type and register locator into the register table.
    #[inline]
    pub fn insert(&mut self, literal_type: LiteralType, register: PuzzleRegister) -> Result<()> {
        // Retrieve the registers for the given literal type.
        let Some(registers) = self.register_table.get_mut(&literal_type) else {
            bail!("Failed to retrieve registers for literal type {literal_type:?} in the register table");
        };

        // Insert the register into the registers.
        registers.insert(register);

        Ok(())
    }

    /// Returns the k-th last register for the given literal type in the register table.
    #[inline]
    pub fn get_k_th_last_register(&self, literal_type: &LiteralType, k: usize) -> Result<PuzzleRegister> {
        // Retrieve the registers for the given literal type.
        let Some(registers) = self.register_table.get(literal_type) else {
            bail!("Failed to retrieve registers for literal type {literal_type:?} in the register table");
        };

        // Get the k-th last register.
        let index = registers.len().saturating_sub(k).saturating_sub(1);
        match registers.get_index(index) {
            Some(register) => Ok(*register),
            None => bail!("Invalid offset {k} for registers for literal type {literal_type:?} in the register table"),
        }
    }
}

impl RegisterTable {
    /// Returns `true` if the register table is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.register_table.is_empty()
    }

    /// Returns the number of literal types in the register table.
    #[inline]
    pub fn len(&self) -> usize {
        self.register_table.len()
    }

    /// Returns `true` if the input registers contain the given literal type.
    #[inline]
    pub fn contains_input(&self, literal_type: &LiteralType) -> bool {
        self.input_registers.contains_key(literal_type)
    }

    /// Returns `true` if the register table contains the given literal type.
    #[inline]
    pub fn contains_key(&self, literal_type: &LiteralType) -> bool {
        self.register_table.contains_key(literal_type)
    }

    /// Returns a reference to the registers for the given literal type in the register table.
    #[inline]
    pub fn get(&self, literal_type: &LiteralType) -> Option<&IndexSet<PuzzleRegister>> {
        self.register_table.get(literal_type)
    }

    /// Returns a reference to the input register at an index for the given literal type.
    #[inline]
    pub fn get_input_at_index(&self, literal_type: &LiteralType, index: usize) -> Option<&PuzzleRegister> {
        self.input_registers.get(literal_type).and_then(|registers| registers.get_index(index))
    }

    /// Returns an iterator over the register table.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&LiteralType, &IndexSet<PuzzleRegister>)> {
        self.register_table.iter()
    }

    /// Returns the literal types in the register table.
    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = &LiteralType> {
        self.register_table.keys()
    }

    /// Returns the registers in the register table.
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &IndexSet<PuzzleRegister>> {
        self.register_table.values()
    }
}

impl IntoIterator for RegisterTable {
    type IntoIter = indexmap::map::IntoIter<LiteralType, IndexSet<PuzzleRegister>>;
    type Item = (LiteralType, IndexSet<PuzzleRegister>);

    /// Returns the literal types and registers in the register table.
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.register_table.into_iter()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use console::{network::MainnetV0, program::Identifier};
    use snarkvm_synthesizer_program::Program;
    use std::str::FromStr;

    type CurrentNetwork = MainnetV0;

    pub const NUM_PREAMBLE_INSTRUCTIONS: usize = 26;
    pub const NUM_INPUTS: usize = 14;

    #[test]
    fn test_register_table() {
        // The multiplier is hardcoded.
        let multiplier = 2;

        // Initialize a new register table.
        let register_table = RegisterTable::new();

        // Check if the register table is empty.
        assert!(!register_table.is_empty());
        // Check the number of literal types in the register table.
        assert_eq!(register_table.len(), 12);
        // Check each literal type in the register table.
        assert!(register_table.contains_key(&LiteralType::Boolean));
        assert!(register_table.contains_key(&LiteralType::Field));
        assert!(register_table.contains_key(&LiteralType::I8));
        assert!(register_table.contains_key(&LiteralType::I16));
        assert!(register_table.contains_key(&LiteralType::I32));
        assert!(register_table.contains_key(&LiteralType::I64));
        assert!(register_table.contains_key(&LiteralType::I128));
        assert!(register_table.contains_key(&LiteralType::U8));
        assert!(register_table.contains_key(&LiteralType::U16));
        assert!(register_table.contains_key(&LiteralType::U32));
        assert!(register_table.contains_key(&LiteralType::U64));
        assert!(register_table.contains_key(&LiteralType::U128));
        // Check if the register table contains the registers.
        for (_, registers) in register_table.iter() {
            assert_eq!(registers.len(), multiplier);
        }
        // Check the next register locator.
        assert_eq!(register_table.next_register_locator, 40, "Update me if the design has changed");
    }

    #[test]
    fn test_get_k_th_last_register() {
        // Initialize a new register table.
        let register_table = RegisterTable::new();

        for (literal_type, expected_registers) in register_table.iter() {
            // Check the 'get' function.
            assert_eq!(register_table.get(literal_type), Some(expected_registers));
            // Check the 'get_k_th_last_register' function.
            let mut registers = IndexSet::new();
            for k in (0..expected_registers.len()).rev() {
                let register = register_table.get_k_th_last_register(literal_type, k).unwrap();
                registers.insert(register);
            }
            // Check that `registers` matches the `expected_registers`.
            assert_eq!(&registers, expected_registers);
        }
    }

    #[test]
    fn test_input_block_is_well_formed() {
        // Initialize a new register table.
        let register_table = RegisterTable::new();

        // Initialize a program using the input block.
        let program = Program::<CurrentNetwork>::from_str(&format!(
            "program puzzle.aleo;function synthesize:{}",
            register_table.input_block()
        ))
        .unwrap();

        // Get the function.
        let function = program.get_function_ref(&Identifier::from_str("synthesize").unwrap()).unwrap();

        // Check that the number of inputs and instructions is well-formed.
        assert_eq!(function.inputs().len(), NUM_INPUTS, "Update me if the design has changed");
        assert_eq!(function.instructions().len(), NUM_PREAMBLE_INSTRUCTIONS, "Update me if the design has changed");
    }
}
