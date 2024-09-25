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

use console::{
    prelude::Network,
    program::{Literal, LiteralType},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PuzzleOperand<N: Network> {
    Ephemeral(LiteralType, u16),
    Input(LiteralType, u16),
    Literal(Literal<N>),
    Register(LiteralType),
    RegisterOffset(LiteralType, u16),
}

impl<N: Network> PuzzleOperand<N> {
    /// Returns whether or not the puzzle operand is an ephemeral register.
    #[inline]
    pub fn is_ephemeral(&self) -> bool {
        matches!(self, Self::Ephemeral(_, _))
    }

    /// Returns whether or not the puzzle operand is an input register.
    #[inline]
    pub fn is_input(&self) -> bool {
        matches!(self, Self::Input(_, _))
    }

    /// Returns whether or not the puzzle operand is a literal.
    #[inline]
    pub fn is_literal(&self) -> bool {
        matches!(self, Self::Literal(_))
    }

    /// Returns whether or not the puzzle operand is a register.
    #[inline]
    pub fn is_register(&self) -> bool {
        matches!(self, Self::Register(_))
    }

    /// Returns whether or not the puzzle operand is a register offset.
    #[inline]
    pub fn is_register_offset(&self) -> bool {
        matches!(self, Self::RegisterOffset(_, _))
    }
}
