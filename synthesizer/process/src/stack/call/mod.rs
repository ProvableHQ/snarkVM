// Copyright (c) 2019-2025 Provable Inc.
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

mod dynamic_call;
mod static_call;

use crate::{CallStack, Registers, RegistersCall, StackEvaluate, StackExecute, stack::Address};

use console::{
    account::Field,
    network::prelude::*,
    program::{Register, Request, Value, ValueType},
};
use synthesizer_program::{
    Call,
    CallOperator,
    DynamicCall,
    Operand,
    RegistersLoad,
    RegistersLoadCircuit,
    RegistersSigner,
    RegistersSignerCircuit,
    RegistersStore,
    RegistersStoreCircuit,
    StackKeys,
    StackMatches,
    StackProgram,
};

use aleo_std::prelude::{finish, lap, timer};

pub trait CallTrait<N: Network> {
    /// Evaluates the instruction.
    fn evaluate<A: circuit::Aleo<Network = N>>(
        &self,
        stack: &(impl StackEvaluate<N> + StackMatches<N> + StackProgram<N>),
        registers: &mut Registers<N, A>,
    ) -> Result<()>;

    /// Executes the instruction.
    fn execute<A: circuit::Aleo<Network = N>, R: CryptoRng + Rng>(
        &self,
        stack: &(impl StackEvaluate<N> + StackExecute<N> + StackMatches<N> + StackKeys<N> + StackProgram<N>),
        registers: &mut (
                 impl RegistersCall<N>
                 + RegistersSigner<N>
                 + RegistersSignerCircuit<N, A>
                 + RegistersLoadCircuit<N, A>
                 + RegistersStoreCircuit<N, A>
             ),
        rng: &mut R,
    ) -> Result<()>;
}
