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
    network::Network,
    prelude::{FromBytes, Parser, ToBytes},
    program::{Identifier, Register},
};

pub trait CommandTrait<N: Network>: Clone + Parser + FromBytes + ToBytes {
    /// Returns the destination registers of the command.
    fn destinations(&self) -> Vec<Register<N>>;
    /// Returns the branch target, if the command is a branch command.
    fn branch_to(&self) -> Option<&Identifier<N>>;
    /// Returns the position name, if the command is a position command.
    fn position(&self) -> Option<&Identifier<N>>;
    /// Returns `true` if the command is a call instruction.
    fn is_call(&self) -> bool;
    /// Returns `true` if the command is a cast to record instruction.
    fn is_cast_to_record(&self) -> bool;
    /// Returns `true` if the command is a write operation.
    fn is_write(&self) -> bool;
}
