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

use crate::FinalizeTypes;
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
pub struct FinalizeRegisters<N: Network> {
    /// The global state for the finalize scope.
    state: FinalizeGlobalState,
    /// The transition ID for the finalize scope.
    transition_id: N::TransitionID,
    /// The function name for the finalize scope.
    function_name: Identifier<N>,
    /// The mapping of all registers to their defined types.
    finalize_types: FinalizeTypes<N>,
    /// The mapping of assigned registers to their values.
    registers: IndexMap<u64, Value<N>>,
    /// The tracker for the last register locator.
    last_register: Option<u64>,
}

impl<N: Network> FinalizeRegisters<N> {
    /// Initializes a new set of registers, given the finalize types.
    #[inline]
    pub fn new(
        state: FinalizeGlobalState,
        transition_id: N::TransitionID,
        function_name: Identifier<N>,
        finalize_types: FinalizeTypes<N>,
    ) -> Self {
        Self { state, transition_id, finalize_types, function_name, registers: IndexMap::new(), last_register: None }
    }
}

impl<N: Network> FinalizeRegistersState<N> for FinalizeRegisters<N> {
    /// Returns the global state for the finalize scope.
    #[inline]
    fn state(&self) -> &FinalizeGlobalState {
        &self.state
    }

    /// Returns the transition ID for the finalize scope.
    #[inline]
    fn transition_id(&self) -> &N::TransitionID {
        &self.transition_id
    }

    /// Returns the function name for the finalize scope.
    #[inline]
    fn function_name(&self) -> &Identifier<N> {
        &self.function_name
    }
}
