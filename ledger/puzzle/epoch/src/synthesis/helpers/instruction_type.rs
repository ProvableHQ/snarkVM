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

use std::fmt::Display;

/// The number of puzzle instruction variants.
pub const NUM_PUZZLE_INSTRUCTION_VARIANTS: u8 = 64;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PuzzleInstructionType {
    Abs,
    AbsWrapped,
    Add,
    AddWrapped,
    And,
    AssertEq,
    AssertNeq,
    BranchEq,
    BranchNeq,
    Cast,
    CastLossy,
    CommitBhp256,
    CommitBhp512,
    CommitBhp768,
    CommitBhp1024,
    CommitPed64,
    CommitPed128,
    Div,
    DivWrapped,
    Double,
    Gt,
    Gte,
    HashBhp256,
    HashBhp512,
    HashBhp768,
    HashBhp1024,
    HashKeccak256,
    HashKeccak384,
    HashKeccak512,
    HashPed64,
    HashPed128,
    HashPsd2,
    HashPsd4,
    HashPsd8,
    HashSha3256,
    HashSha3384,
    HashSha3512,
    Inv,
    IsEq,
    IsNeq,
    Lt,
    Lte,
    Mod,
    Mul,
    MulWrapped,
    Nand,
    Neg,
    Nor,
    Not,
    Or,
    Pow,
    PowWrapped,
    Rem,
    RemWrapped,
    Shl,
    ShlWrapped,
    Shr,
    ShrWrapped,
    Sqrt,
    Square,
    Sub,
    SubWrapped,
    Ternary,
    Xor,
}

impl PuzzleInstructionType {
    /// All puzzle instruction variants.
    ///
    /// Note: This is used as a compile-time safety check that `NUM_PUZZLE_INSTRUCTION_VARIANTS` is correct.
    pub const ALL: [Self; NUM_PUZZLE_INSTRUCTION_VARIANTS as usize] = [
        Self::Abs,
        Self::AbsWrapped,
        Self::Add,
        Self::AddWrapped,
        Self::And,
        Self::AssertEq,
        Self::AssertNeq,
        Self::BranchEq,
        Self::BranchNeq,
        Self::Cast,
        Self::CastLossy,
        Self::CommitBhp256,
        Self::CommitBhp512,
        Self::CommitBhp768,
        Self::CommitBhp1024,
        Self::CommitPed64,
        Self::CommitPed128,
        Self::Div,
        Self::DivWrapped,
        Self::Double,
        Self::Gt,
        Self::Gte,
        Self::HashBhp256,
        Self::HashBhp512,
        Self::HashBhp768,
        Self::HashBhp1024,
        Self::HashKeccak256,
        Self::HashKeccak384,
        Self::HashKeccak512,
        Self::HashPed64,
        Self::HashPed128,
        Self::HashPsd2,
        Self::HashPsd4,
        Self::HashPsd8,
        Self::HashSha3256,
        Self::HashSha3384,
        Self::HashSha3512,
        Self::Inv,
        Self::IsEq,
        Self::IsNeq,
        Self::Lt,
        Self::Lte,
        Self::Mod,
        Self::Mul,
        Self::MulWrapped,
        Self::Nand,
        Self::Neg,
        Self::Nor,
        Self::Not,
        Self::Or,
        Self::Pow,
        Self::PowWrapped,
        Self::Rem,
        Self::RemWrapped,
        Self::Shl,
        Self::ShlWrapped,
        Self::Shr,
        Self::ShrWrapped,
        Self::Sqrt,
        Self::Square,
        Self::Sub,
        Self::SubWrapped,
        Self::Ternary,
        Self::Xor,
    ];

    /// Initializes a new puzzle instruction type.
    pub fn new(variant: u8) -> Self {
        match variant {
            0 => Self::Abs,
            1 => Self::AbsWrapped,
            2 => Self::Add,
            3 => Self::AddWrapped,
            4 => Self::And,
            5 => Self::AssertEq,
            6 => Self::AssertNeq,
            7 => Self::BranchEq,
            8 => Self::BranchNeq,
            9 => Self::Cast,
            10 => Self::CastLossy,
            11 => Self::CommitBhp256,
            12 => Self::CommitBhp512,
            13 => Self::CommitBhp768,
            14 => Self::CommitBhp1024,
            15 => Self::CommitPed64,
            16 => Self::CommitPed128,
            17 => Self::Div,
            18 => Self::DivWrapped,
            19 => Self::Double,
            20 => Self::Gt,
            21 => Self::Gte,
            22 => Self::HashBhp256,
            23 => Self::HashBhp512,
            24 => Self::HashBhp768,
            25 => Self::HashBhp1024,
            26 => Self::HashKeccak256,
            27 => Self::HashKeccak384,
            28 => Self::HashKeccak512,
            29 => Self::HashPed64,
            30 => Self::HashPed128,
            31 => Self::HashPsd2,
            32 => Self::HashPsd4,
            33 => Self::HashPsd8,
            34 => Self::HashSha3256,
            35 => Self::HashSha3384,
            36 => Self::HashSha3512,
            37 => Self::Inv,
            38 => Self::IsEq,
            39 => Self::IsNeq,
            40 => Self::Lt,
            41 => Self::Lte,
            42 => Self::Mod,
            43 => Self::Mul,
            44 => Self::MulWrapped,
            45 => Self::Nand,
            46 => Self::Neg,
            47 => Self::Nor,
            48 => Self::Not,
            49 => Self::Or,
            50 => Self::Pow,
            51 => Self::PowWrapped,
            52 => Self::Rem,
            53 => Self::RemWrapped,
            54 => Self::Shl,
            55 => Self::ShlWrapped,
            56 => Self::Shr,
            57 => Self::ShrWrapped,
            58 => Self::Sqrt,
            59 => Self::Square,
            60 => Self::Sub,
            61 => Self::SubWrapped,
            62 => Self::Ternary,
            63 => Self::Xor,
            64.. => unreachable!("Invalid puzzle instruction variant"),
        }
    }

    /// Returns the puzzle instruction variant.
    pub fn variant(&self) -> u8 {
        *self as u8
    }

    /// Returns the puzzle instruction opcode.
    pub fn opcode(&self) -> &'static str {
        match self {
            Self::Abs => "abs",
            Self::AbsWrapped => "abs.w",
            Self::Add => "add",
            Self::AddWrapped => "add.w",
            Self::And => "and",
            Self::AssertEq => "assert.eq",
            Self::AssertNeq => "assert.neq",
            Self::BranchEq => "branch.eq",
            Self::BranchNeq => "branch.neq",
            Self::Cast => "cast",
            Self::CastLossy => "cast.lossy",
            Self::CommitBhp256 => "commit.bhp256",
            Self::CommitBhp512 => "commit.bhp512",
            Self::CommitBhp768 => "commit.bhp768",
            Self::CommitBhp1024 => "commit.bhp1024",
            Self::CommitPed64 => "commit.ped64",
            Self::CommitPed128 => "commit.ped128",
            Self::Div => "div",
            Self::DivWrapped => "div.w",
            Self::Double => "double",
            Self::Gt => "gt",
            Self::Gte => "gte",
            Self::HashBhp256 => "hash.bhp256",
            Self::HashBhp512 => "hash.bhp512",
            Self::HashBhp768 => "hash.bhp768",
            Self::HashBhp1024 => "hash.bhp1024",
            Self::HashKeccak256 => "hash.keccak256",
            Self::HashKeccak384 => "hash.keccak384",
            Self::HashKeccak512 => "hash.keccak512",
            Self::HashPed64 => "hash.ped64",
            Self::HashPed128 => "hash.ped128",
            Self::HashPsd2 => "hash.psd2",
            Self::HashPsd4 => "hash.psd4",
            Self::HashPsd8 => "hash.psd8",
            Self::HashSha3256 => "hash.sha3_256",
            Self::HashSha3384 => "hash.sha3_384",
            Self::HashSha3512 => "hash.sha3_512",
            Self::Inv => "inv",
            Self::IsEq => "is.eq",
            Self::IsNeq => "is.neq",
            Self::Lt => "lt",
            Self::Lte => "lte",
            Self::Mod => "mod",
            Self::Mul => "mul",
            Self::MulWrapped => "mul.w",
            Self::Nand => "nand",
            Self::Neg => "neg",
            Self::Nor => "nor",
            Self::Not => "not",
            Self::Or => "or",
            Self::Pow => "pow",
            Self::PowWrapped => "pow.w",
            Self::Rem => "rem",
            Self::RemWrapped => "rem.w",
            Self::Shl => "shl",
            Self::ShlWrapped => "shl.w",
            Self::Shr => "shr",
            Self::ShrWrapped => "shr.w",
            Self::Sqrt => "sqrt",
            Self::Square => "square",
            Self::Sub => "sub",
            Self::SubWrapped => "sub.w",
            Self::Ternary => "ternary",
            Self::Xor => "xor",
        }
    }

    /// Returns the number of operands required by the puzzle instruction type.
    pub fn num_operands(&self) -> usize {
        match self {
            Self::Abs => 1,
            Self::AbsWrapped => 1,
            Self::Add => 2,
            Self::AddWrapped => 2,
            Self::And => 2,
            Self::AssertEq => 2,
            Self::AssertNeq => 2,
            Self::BranchEq => 2,
            Self::BranchNeq => 2,
            Self::Cast => 1,
            Self::CastLossy => 1,
            Self::CommitBhp256 => 2,
            Self::CommitBhp512 => 2,
            Self::CommitBhp768 => 2,
            Self::CommitBhp1024 => 2,
            Self::CommitPed64 => 2,
            Self::CommitPed128 => 2,
            Self::Div => 2,
            Self::DivWrapped => 2,
            Self::Double => 1,
            Self::Gt => 2,
            Self::Gte => 2,
            Self::HashBhp256 => 1,
            Self::HashBhp512 => 1,
            Self::HashBhp768 => 1,
            Self::HashBhp1024 => 1,
            Self::HashKeccak256 => 1,
            Self::HashKeccak384 => 1,
            Self::HashKeccak512 => 1,
            Self::HashPed64 => 1,
            Self::HashPed128 => 1,
            Self::HashPsd2 => 1,
            Self::HashPsd4 => 1,
            Self::HashPsd8 => 1,
            Self::HashSha3256 => 1,
            Self::HashSha3384 => 1,
            Self::HashSha3512 => 1,
            Self::Inv => 1,
            Self::IsEq => 2,
            Self::IsNeq => 2,
            Self::Lt => 2,
            Self::Lte => 2,
            Self::Mod => 2,
            Self::Mul => 2,
            Self::MulWrapped => 2,
            Self::Nand => 2,
            Self::Neg => 1,
            Self::Nor => 2,
            Self::Not => 1,
            Self::Or => 2,
            Self::Pow => 2,
            Self::PowWrapped => 2,
            Self::Rem => 2,
            Self::RemWrapped => 2,
            Self::Shl => 2,
            Self::ShlWrapped => 2,
            Self::Shr => 2,
            Self::ShrWrapped => 2,
            Self::Sqrt => 1,
            Self::Square => 1,
            Self::Sub => 2,
            Self::SubWrapped => 2,
            Self::Ternary => 3,
            Self::Xor => 2,
        }
    }
}

impl Display for PuzzleInstructionType {
    /// Returns the display string for the instruction type.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.opcode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_puzzle_instruction_type() {
        for instruction_variant in 0..NUM_PUZZLE_INSTRUCTION_VARIANTS {
            let instruction_type = PuzzleInstructionType::new(instruction_variant);
            assert_eq!(instruction_type.variant(), instruction_variant);
        }
    }

    #[test]
    fn test_puzzle_instruction_type_all() {
        assert_eq!(PuzzleInstructionType::ALL.len(), NUM_PUZZLE_INSTRUCTION_VARIANTS as usize);
        for (instruction_variant, instruction_type) in PuzzleInstructionType::ALL.iter().enumerate() {
            assert_eq!(instruction_type.variant(), u8::try_from(instruction_variant).unwrap());
        }
    }

    #[test]
    fn test_puzzle_instruction_type_opcode() {
        for instruction_type in PuzzleInstructionType::ALL.iter() {
            let opcode = instruction_type.opcode();
            assert!(!opcode.is_empty());
        }
    }

    #[test]
    fn test_puzzle_instruction_type_num_operands() {
        for instruction_type in PuzzleInstructionType::ALL.iter() {
            let num_operands = instruction_type.num_operands();
            assert!(num_operands > 0);
            assert!(num_operands <= 3);
        }
    }
}
