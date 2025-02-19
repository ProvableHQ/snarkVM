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

mod load;
mod store;

use crate::ConstructorTypes;
use console::{
    network::prelude::*,
    program::{Identifier, Literal, Plaintext, Register, Value},
    types::{U16, U32},
};
use synthesizer_program::{
    FinalizeGlobalState,
    FinalizeRegistersState,
    Operand,
    RegistersLoad,
    RegistersStore,
    StackMatches,
    StackProgram,
};

use indexmap::IndexMap;

#[derive(Clone)]
pub struct ConstructorRegisters<N: Network> {
    /// The global state for the constructor scope.
    state: FinalizeGlobalState,
    /// The transition ID for the constructor scope.
    transition_id: N::TransitionID,
    /// The name of the constructor scope.
    name: Identifier<N>,
    /// The mapping of all registers to their defined types.
    constructor_types: ConstructorTypes<N>,
    /// The mapping of assigned registers to their values.
    registers: IndexMap<u64, Value<N>>,
    /// A nonce for constructor registers.
    nonce: u64,
    /// The tracker for the last register locator.
    last_register: Option<u64>,
}

impl<N: Network> ConstructorRegisters<N> {
    /// Initializes a new set of registers, given the finalize types.
    #[inline]
    pub fn new(
        state: FinalizeGlobalState,
        transition_id: N::TransitionID,
        name: Identifier<N>,
        constructor_types: ConstructorTypes<N>,
        nonce: u64,
    ) -> Self {
        Self { state, transition_id, constructor_types, name, registers: IndexMap::new(), nonce, last_register: None }
    }
}

impl<N: Network> FinalizeRegistersState<N> for ConstructorRegisters<N> {
    /// Returns the global state for the constructor scope.
    #[inline]
    fn state(&self) -> &FinalizeGlobalState {
        &self.state
    }

    /// Returns the transition ID for the constructor scope.
    #[inline]
    fn transition_id(&self) -> &N::TransitionID {
        &self.transition_id
    }

    /// Returns the function name for the constructor scope.
    /// Note that in the case of a constructor, the program name is used.
    #[inline]
    fn function_name(&self) -> &Identifier<N> {
        &self.name
    }

    /// Returns the nonce for the constructor registers.
    #[inline]
    fn nonce(&self) -> u64 {
        self.nonce
    }
}
