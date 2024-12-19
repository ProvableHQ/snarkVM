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

use std::sync::Arc;

use crate::{FinalizeGlobalState, Function, Operand, Program};
use console::{
    account::Group,
    network::Network,
    prelude::{Result, bail},
    program::{
        Future,
        Identifier,
        Literal,
        Locator,
        Plaintext,
        PlaintextType,
        ProgramID,
        Record,
        RecordType,
        Register,
        RegisterType,
        Value,
        ValueType,
    },
    types::{Address, Field},
};
use rand::{CryptoRng, Rng};

pub trait StackMatches<N: Network> {
    /// Checks that the given value matches the layout of the value type.
    fn matches_value_type(&self, value: &Value<N>, value_type: &ValueType<N>) -> Result<()>;

    /// Checks that the given stack value matches the layout of the register type.
    fn matches_register_type(&self, stack_value: &Value<N>, register_type: &RegisterType<N>) -> Result<()>;

    /// Checks that the given record matches the layout of the external record type.
    fn matches_external_record(&self, record: &Record<N, Plaintext<N>>, locator: &Locator<N>) -> Result<()>;

    /// Checks that the given record matches the layout of the record type.
    fn matches_record(&self, record: &Record<N, Plaintext<N>>, record_name: &Identifier<N>) -> Result<()>;

    /// Checks that the given plaintext matches the layout of the plaintext type.
    fn matches_plaintext(&self, plaintext: &Plaintext<N>, plaintext_type: &PlaintextType<N>) -> Result<()>;

    /// Checks that the given future matches the layout of the future type.
    fn matches_future(&self, future: &Future<N>, locator: &Locator<N>) -> Result<()>;
}

pub trait StackProgram<N: Network> {
    /// Returns the program.
    fn program(&self) -> &Program<N>;

    /// Returns the program ID.
    fn program_id(&self) -> &ProgramID<N>;

    /// Returns the program depth.
    fn program_depth(&self) -> usize;

    /// Returns the program address.
    fn program_address(&self) -> &Address<N>;

    /// Returns `true` if the stack contains the external record.
    fn contains_external_record(&self, locator: &Locator<N>) -> bool;

    /// Returns the external stack for the given program ID.
    fn get_external_stack(&self, program_id: &ProgramID<N>) -> Result<&Arc<Self>>;

    /// Returns the external program for the given program ID.
    fn get_external_program(&self, program_id: &ProgramID<N>) -> Result<&Program<N>>;

    /// Returns `true` if the stack contains the external record.
    fn get_external_record(&self, locator: &Locator<N>) -> Result<&RecordType<N>>;

    /// Returns the expected finalize cost for the given function name.
    fn get_finalize_cost(&self, function_name: &Identifier<N>) -> Result<u64>;

    /// Returns the function with the given function name.
    fn get_function(&self, function_name: &Identifier<N>) -> Result<Function<N>>;

    /// Returns a reference to the function with the given function name.
    fn get_function_ref(&self, function_name: &Identifier<N>) -> Result<&Function<N>>;

    /// Returns the expected number of calls for the given function name.
    fn get_number_of_calls(&self, function_name: &Identifier<N>) -> Result<usize>;

    /// Samples a value for the given value_type.
    fn sample_value<R: Rng + CryptoRng>(
        &self,
        burner_address: &Address<N>,
        value_type: &ValueType<N>,
        rng: &mut R,
    ) -> Result<Value<N>>;

    /// Returns a record for the given record name, with the given burner address and nonce.
    fn sample_record<R: Rng + CryptoRng>(
        &self,
        burner_address: &Address<N>,
        record_name: &Identifier<N>,
        record_nonce: Group<N>,
        rng: &mut R,
    ) -> Result<Record<N, Plaintext<N>>>;
}

pub trait FinalizeRegistersState<N: Network> {
    /// Returns the global state for the finalize scope.
    fn state(&self) -> &FinalizeGlobalState;

    /// Returns the transition ID for the finalize scope.
    fn transition_id(&self) -> &N::TransitionID;

    /// Returns the function name for the finalize scope.
    fn function_name(&self) -> &Identifier<N>;

    /// Returns the nonce for the finalize registers.
    fn nonce(&self) -> u64;
}

pub trait RegistersSigner<N: Network> {
    /// Returns the transition signer.
    fn signer(&self) -> Result<Address<N>>;

    /// Sets the transition signer.
    fn set_signer(&mut self, signer: Address<N>);

    /// Returns the root transition view key.
    fn root_tvk(&self) -> Result<Field<N>>;

    /// Sets the root transition view key.
    fn set_root_tvk(&mut self, root_tvk: Field<N>);

    /// Returns the transition caller.
    fn caller(&self) -> Result<Address<N>>;

    /// Sets the transition caller.
    fn set_caller(&mut self, caller: Address<N>);

    /// Returns the transition view key.
    fn tvk(&self) -> Result<Field<N>>;

    /// Sets the transition view key.
    fn set_tvk(&mut self, tvk: Field<N>);
}

pub trait RegistersSignerCircuit<N: Network, A: circuit::Aleo<Network = N>> {
    /// Returns the transition signer, as a circuit.
    fn signer_circuit(&self) -> Result<circuit::Address<A>>;

    /// Sets the transition signer, as a circuit.
    fn set_signer_circuit(&mut self, signer_circuit: circuit::Address<A>);

    /// Returns the root transition view key, as a circuit.
    fn root_tvk_circuit(&self) -> Result<circuit::Field<A>>;

    /// Sets the root transition view key, as a circuit.
    fn set_root_tvk_circuit(&mut self, root_tvk_circuit: circuit::Field<A>);

    /// Returns the transition caller, as a circuit.
    fn caller_circuit(&self) -> Result<circuit::Address<A>>;

    /// Sets the transition caller, as a circuit.
    fn set_caller_circuit(&mut self, caller_circuit: circuit::Address<A>);

    /// Returns the transition view key, as a circuit.
    fn tvk_circuit(&self) -> Result<circuit::Field<A>>;

    /// Sets the transition view key, as a circuit.
    fn set_tvk_circuit(&mut self, tvk_circuit: circuit::Field<A>);
}

pub trait RegistersLoad<N: Network> {
    /// Loads the value of a given operand.
    ///
    /// # Errors
    /// This method should halt if the register locator is not found.
    /// In the case of register members, this method should halt if the member is not found.
    fn load(&self, stack: &(impl StackMatches<N> + StackProgram<N>), operand: &Operand<N>) -> Result<Value<N>>;

    /// Loads the literal of a given operand.
    ///
    /// # Errors
    /// This method should halt if the given operand is not a literal.
    /// This method should halt if the register locator is not found.
    /// In the case of register members, this method should halt if the member is not found.
    #[inline]
    fn load_literal(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        operand: &Operand<N>,
    ) -> Result<Literal<N>> {
        match self.load(stack, operand)? {
            Value::Plaintext(Plaintext::Literal(literal, ..)) => Ok(literal),
            Value::Plaintext(Plaintext::Struct(..))
            | Value::Plaintext(Plaintext::Array(..))
            | Value::Record(..)
            | Value::Future(..) => {
                bail!("Operand must be a literal")
            }
        }
    }

    /// Loads the plaintext of a given operand.
    ///
    /// # Errors
    /// This method should halt if the given operand is not a plaintext.
    /// This method should halt if the register locator is not found.
    /// In the case of register members, this method should halt if the member is not found.
    #[inline]
    fn load_plaintext(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        operand: &Operand<N>,
    ) -> Result<Plaintext<N>> {
        match self.load(stack, operand)? {
            Value::Plaintext(plaintext) => Ok(plaintext),
            Value::Record(..) | Value::Future(..) => bail!("Operand must be a plaintext"),
        }
    }
}

pub trait RegistersLoadCircuit<N: Network, A: circuit::Aleo<Network = N>> {
    /// Loads the value of a given operand.
    ///
    /// # Errors
    /// This method should halt if the register locator is not found.
    /// In the case of register members, this method should halt if the member is not found.
    fn load_circuit(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        operand: &Operand<N>,
    ) -> Result<circuit::Value<A>>;

    /// Loads the literal of a given operand.
    ///
    /// # Errors
    /// This method should halt if the given operand is not a literal.
    /// This method should halt if the register locator is not found.
    /// In the case of register members, this method should halt if the member is not found.
    #[inline]
    fn load_literal_circuit(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        operand: &Operand<N>,
    ) -> Result<circuit::Literal<A>> {
        match self.load_circuit(stack, operand)? {
            circuit::Value::Plaintext(circuit::Plaintext::Literal(literal, ..)) => Ok(literal),
            circuit::Value::Plaintext(circuit::Plaintext::Struct(..))
            | circuit::Value::Plaintext(circuit::Plaintext::Array(..))
            | circuit::Value::Record(..)
            | circuit::Value::Future(..) => bail!("Operand must be a literal"),
        }
    }

    /// Loads the plaintext of a given operand.
    ///
    /// # Errors
    /// This method should halt if the given operand is not a plaintext.
    /// This method should halt if the register locator is not found.
    /// In the case of register members, this method should halt if the member is not found.
    #[inline]
    fn load_plaintext_circuit(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        operand: &Operand<N>,
    ) -> Result<circuit::Plaintext<A>> {
        match self.load_circuit(stack, operand)? {
            circuit::Value::Plaintext(plaintext) => Ok(plaintext),
            circuit::Value::Record(..) | circuit::Value::Future(..) => bail!("Operand must be a plaintext"),
        }
    }
}

pub trait RegistersStore<N: Network> {
    /// Assigns the given value to the given register, assuming the register is not already assigned.
    ///
    /// # Errors
    /// This method should halt if the given register is a register member.
    /// This method should halt if the given register is an input register.
    /// This method should halt if the register is already used.
    fn store(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        register: &Register<N>,
        stack_value: Value<N>,
    ) -> Result<()>;

    /// Assigns the given literal to the given register, assuming the register is not already assigned.
    ///
    /// # Errors
    /// This method should halt if the given register is a register member.
    /// This method should halt if the given register is an input register.
    /// This method should halt if the register is already used.
    #[inline]
    fn store_literal(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        register: &Register<N>,
        literal: Literal<N>,
    ) -> Result<()> {
        self.store(stack, register, Value::Plaintext(Plaintext::from(literal)))
    }
}

pub trait RegistersStoreCircuit<N: Network, A: circuit::Aleo<Network = N>> {
    /// Assigns the given value to the given register, assuming the register is not already assigned.
    ///
    /// # Errors
    /// This method should halt if the given register is a register member.
    /// This method should halt if the given register is an input register.
    /// This method should halt if the register is already used.
    fn store_circuit(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        register: &Register<N>,
        stack_value: circuit::Value<A>,
    ) -> Result<()>;

    /// Assigns the given literal to the given register, assuming the register is not already assigned.
    ///
    /// # Errors
    /// This method should halt if the given register is a register member.
    /// This method should halt if the given register is an input register.
    /// This method should halt if the register is already used.
    #[inline]
    fn store_literal_circuit(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        register: &Register<N>,
        literal: circuit::Literal<A>,
    ) -> Result<()> {
        self.store_circuit(stack, register, circuit::Value::Plaintext(circuit::Plaintext::from(literal)))
    }
}
