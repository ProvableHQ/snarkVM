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

mod call;
mod caller;
mod load;
mod store;

use crate::{CallStack, RegisterTypes, RegistersCall};
use console::{
    network::prelude::*,
    program::{Entry, Literal, Plaintext, Register, Value},
    types::{Address, Field},
};
use synthesizer_program::{
    Operand,
    RegistersLoad,
    RegistersLoadCircuit,
    RegistersSigner,
    RegistersSignerCircuit,
    RegistersStore,
    RegistersStoreCircuit,
    StackMatches,
    StackProgram,
};

use indexmap::IndexMap;

#[derive(Clone)]
pub struct Registers<N: Network, A: circuit::Aleo<Network = N>> {
    /// The current call stack.
    call_stack: CallStack<N>,
    /// The mapping of all registers to their defined types.
    register_types: RegisterTypes<N>,
    /// The mapping of assigned console registers to their values.
    console_registers: IndexMap<u64, Value<N>>,
    /// The mapping of assigned circuit registers to their values.
    circuit_registers: IndexMap<u64, circuit::Value<A>>,
    /// The transition signer.
    signer: Option<Address<N>>,
    /// The transition signer, as a circuit.
    signer_circuit: Option<circuit::Address<A>>,
    /// The root transition view key.
    root_tvk: Option<Field<N>>,
    /// The root transition view key, as a circuit.
    root_tvk_circuit: Option<circuit::Field<A>>,
    /// The transition caller.
    caller: Option<Address<N>>,
    /// The transition caller, as a circuit.
    caller_circuit: Option<circuit::Address<A>>,
    /// The transition view key.
    tvk: Option<Field<N>>,
    /// The transition view key, as a circuit.
    tvk_circuit: Option<circuit::Field<A>>,
}

impl<N: Network, A: circuit::Aleo<Network = N>> Registers<N, A> {
    /// Initializes a new set of registers, given the call stack.
    #[inline]
    pub fn new(call_stack: CallStack<N>, register_types: RegisterTypes<N>) -> Self {
        Self {
            call_stack,
            register_types,
            console_registers: IndexMap::new(),
            circuit_registers: IndexMap::new(),
            signer: None,
            signer_circuit: None,
            root_tvk: None,
            root_tvk_circuit: None,
            caller: None,
            caller_circuit: None,
            tvk: None,
            tvk_circuit: None,
        }
    }

    /// Ensure the console and circuit registers match.
    #[inline]
    pub fn ensure_console_and_circuit_registers_match(&self) -> Result<()> {
        use circuit::Eject;

        for ((console_index, console_register), (circuit_index, circuit_register)) in
            self.console_registers.iter().zip_eq(&self.circuit_registers)
        {
            // Ensure the console and circuit index match (executed in same order).
            if *console_index != *circuit_index {
                bail!("Console and circuit register indices are mismatching ({console_index} != {circuit_index})")
            }
            // Ensure the console and circuit registers match (executed to same value).
            if console_register != &circuit_register.eject_value() {
                bail!("The console and circuit register values do not match at index {console_index}")
            }
        }
        Ok(())
    }
}
