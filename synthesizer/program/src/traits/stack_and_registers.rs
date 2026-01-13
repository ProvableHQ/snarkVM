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
        Register,
        RegisterType,
        StructType,
        Value,
        ValueType,
    },
    types::{Address, Field, U8, U16},
};
use rand::{CryptoRng, Rng};
use snarkvm_synthesizer_snark::{ProvingKey, VerifyingKey};

/// This trait is intended to be implemented only by `snarkvm_synthesizer_process::Stack`.
///
/// We make it a trait only to avoid circular dependencies.
pub trait StackTrait<N: Network> {
    /// Returns `true` if the proving key for the given function name exists.
    fn contains_proving_key(&self, function_name: &Identifier<N>) -> bool;

    /// Returns the proving key for the given function name.
    fn get_proving_key(&self, function_name: &Identifier<N>) -> Result<ProvingKey<N>>;

    /// Inserts the proving key for the given function name.
    fn insert_proving_key(&self, function_name: &Identifier<N>, proving_key: ProvingKey<N>) -> Result<()>;

    /// Removes the proving key for the given function name.
    fn remove_proving_key(&self, function_name: &Identifier<N>);

    /// Returns `true` if the verifying key for the given function name exists.
    fn contains_verifying_key(&self, function_name: &Identifier<N>) -> bool;

    /// Returns the verifying key for the given function name.
    fn get_verifying_key(&self, function_name: &Identifier<N>) -> Result<VerifyingKey<N>>;

    /// Inserts the verifying key for the given function name.
    fn insert_verifying_key(&self, function_name: &Identifier<N>, verifying_key: VerifyingKey<N>) -> Result<()>;

    /// Removes the verifying key for the given function name.
    fn remove_verifying_key(&self, function_name: &Identifier<N>);

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

    /// Returns the program.
    fn program(&self) -> &Program<N>;

    /// Returns the program ID.
    fn program_id(&self) -> &ProgramID<N>;

    /// Returns the program address.
    fn program_address(&self) -> &Address<N>;

    /// Returns the program checksum.
    fn program_checksum(&self) -> &[U8<N>; 32];

    /// Returns the program checksum as a field element.
    fn program_checksum_as_field(&self) -> Result<Field<N>>;

    /// Returns the program edition.
    fn program_edition(&self) -> U16<N>;

    /// Returns the program owner.
    /// The program owner should only be set for programs that are deployed after `ConsensusVersion::V9` is active.
    fn program_owner(&self) -> &Option<Address<N>>;

    /// Sets the program owner.
    fn set_program_owner(&mut self, program_owner: Option<Address<N>>);

    /// Returns the external stack for the given program ID.
    fn get_external_stack(&self, program_id: &ProgramID<N>) -> Result<Arc<Self>>;

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
        value_type: &RegisterType<N>,
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

    /// Returns a record for the given record name, deriving the nonce from tvk and index.
    fn sample_record_using_tvk<R: Rng + CryptoRng>(
        &self,
        burner_address: &Address<N>,
        record_name: &Identifier<N>,
        tvk: Field<N>,
        index: Field<N>,
        rng: &mut R,
    ) -> Result<Record<N, Plaintext<N>>>;
}

/// Are the two types either the same, or both structurally equivalent `PlaintextType`s?
pub fn register_types_equivalent<N: Network>(
    stack0: &impl StackTrait<N>,
    type0: &RegisterType<N>,
    stack1: &impl StackTrait<N>,
    type1: &RegisterType<N>,
) -> Result<bool> {
    use RegisterType::*;
    if let (Plaintext(plaintext0), Plaintext(plaintext1)) = (type0, type1) {
        types_equivalent(stack0, plaintext0, stack1, plaintext1)
    } else {
        Ok(type0 == type1)
    }
}

/// Determines whether two `PlaintextType` values are equivalent.
///
/// Equivalence of literals means they're the same type.
///
/// Equivalence of structs means they have the same local names (regardless of whether
/// they're local or external), and their members have the same names and equivalent
/// types in the same order, recursively.
///
/// Equivalence of arrays means they have the same length and their element types are
/// equivalent.
///
/// This definition of equivalence was chosen to balance these concerns:
///
/// 1. All programs from before the existence of external structs will continue to work;
///    thus it's necessary for a struct created from another program to be considered equivalent
///    to a local one with the same name and structure, as in practice that was the behavior.
/// 2. We don't want to allow a fork. Thus we do need to check names, not just structural
///    equivalence - otherwise we could get a program deployable to a node which is using
///    this check, but not deployable to a node running an earlier SnarkVM.
///
/// The stacks are passed because struct types need to access their stack to get their
/// structure.
pub fn types_equivalent<N: Network>(
    stack0: &impl StackTrait<N>,
    type0: &PlaintextType<N>,
    stack1: &impl StackTrait<N>,
    type1: &PlaintextType<N>,
) -> Result<bool> {
    use PlaintextType::*;

    let struct_compare = |stack0, st0: &StructType<N>, stack1, st1: &StructType<N>| -> Result<bool> {
        if st0.members().len() != st1.members().len() {
            return Ok(false);
        }

        for ((name0, type0), (name1, type1)) in st0.members().iter().zip(st1.members()) {
            if name0 != name1 || !types_equivalent(stack0, type0, stack1, type1)? {
                return Ok(false);
            }
        }

        Ok(true)
    };

    match (type0, type1) {
        (Array(array0), Array(array1)) => Ok(array0.length() == array1.length()
            && types_equivalent(stack0, array0.next_element_type(), stack1, array1.next_element_type())?),
        (Literal(lit0), Literal(lit1)) => Ok(lit0 == lit1),
        (Struct(id0), Struct(id1)) => {
            if id0 != id1 {
                return Ok(false);
            }
            let struct_type0 = stack0.program().get_struct(id0)?;
            let struct_type1 = stack1.program().get_struct(id1)?;
            struct_compare(stack0, struct_type0, stack1, struct_type1)
        }
        (ExternalStruct(loc0), ExternalStruct(loc1)) => {
            if loc0.resource() != loc1.resource() {
                return Ok(false);
            }
            let external_stack0 = stack0.get_external_stack(loc0.program_id())?;
            let struct_type0 = external_stack0.program().get_struct(loc0.resource())?;
            let external_stack1 = stack1.get_external_stack(loc1.program_id())?;
            let struct_type1 = external_stack1.program().get_struct(loc1.resource())?;
            struct_compare(&*external_stack0, struct_type0, &*external_stack1, struct_type1)
        }
        (ExternalStruct(loc), Struct(id)) => {
            if loc.resource() != id {
                return Ok(false);
            }
            let external_stack = stack0.get_external_stack(loc.program_id())?;
            let struct_type0 = external_stack.program().get_struct(loc.resource())?;
            let struct_type1 = stack1.program().get_struct(id)?;
            struct_compare(&*external_stack, struct_type0, stack1, struct_type1)
        }
        (Struct(id), ExternalStruct(loc)) => {
            if id != loc.resource() {
                return Ok(false);
            }
            let struct_type0 = stack0.program().get_struct(id)?;
            let external_stack = stack1.get_external_stack(loc.program_id())?;
            let struct_type1 = external_stack.program().get_struct(loc.resource())?;
            struct_compare(stack0, struct_type0, &*external_stack, struct_type1)
        }
        _ => Ok(false),
    }
}

pub trait FinalizeRegistersState<N: Network>: RegistersTrait<N> {
    /// Returns the global state for the finalize scope.
    fn state(&self) -> &FinalizeGlobalState;

    /// Returns the transition ID for the finalize scope.
    fn transition_id(&self) -> &N::TransitionID;

    /// Returns the function name for the finalize scope.
    fn function_name(&self) -> &Identifier<N>;

    /// Returns the nonce for the finalize registers.
    fn nonce(&self) -> u64;
}

pub trait RegistersSigner<N: Network>: RegistersTrait<N> {
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

pub trait RegistersTrait<N: Network> {
    /// Loads the value of a given operand.
    ///
    /// # Errors
    /// This method should halt if the register locator is not found.
    /// In the case of register members, this method should halt if the member is not found.
    fn load(&self, stack: &impl StackTrait<N>, operand: &Operand<N>) -> Result<Value<N>>;

    /// Loads the literal of a given operand.
    ///
    /// # Errors
    /// This method should halt if the given operand is not a literal.
    /// This method should halt if the register locator is not found.
    /// In the case of register members, this method should halt if the member is not found.
    fn load_literal(&self, stack: &impl StackTrait<N>, operand: &Operand<N>) -> Result<Literal<N>> {
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
    fn load_plaintext(&self, stack: &impl StackTrait<N>, operand: &Operand<N>) -> Result<Plaintext<N>> {
        match self.load(stack, operand)? {
            Value::Plaintext(plaintext) => Ok(plaintext),
            Value::Record(..) | Value::Future(..) => bail!("Operand must be a plaintext"),
        }
    }

    /// Assigns the given value to the given register, assuming the register is not already assigned.
    ///
    /// # Errors
    /// This method should halt if the given register is a register member.
    /// This method should halt if the given register is an input register.
    /// This method should halt if the register is already used.
    fn store(&mut self, stack: &impl StackTrait<N>, register: &Register<N>, stack_value: Value<N>) -> Result<()>;

    /// Assigns the given literal to the given register, assuming the register is not already assigned.
    ///
    /// # Errors
    /// This method should halt if the given register is a register member.
    /// This method should halt if the given register is an input register.
    /// This method should halt if the register is already used.
    fn store_literal(&mut self, stack: &impl StackTrait<N>, register: &Register<N>, literal: Literal<N>) -> Result<()> {
        self.store(stack, register, Value::Plaintext(Plaintext::from(literal)))
    }
}

/// This trait is intended to be implemented only by `snarkvm_synthesizer_process::Registers`.
///
/// We make it a trait only to avoid circular dependencies.
pub trait RegistersCircuit<N: Network, A: circuit::Aleo<Network = N>> {
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

    /// Loads the value of a given operand.
    ///
    /// # Errors
    /// This method should halt if the register locator is not found.
    /// In the case of register members, this method should halt if the member is not found.
    fn load_circuit(&self, stack: &impl StackTrait<N>, operand: &Operand<N>) -> Result<circuit::Value<A>>;

    /// Loads the literal of a given operand.
    ///
    /// # Errors
    /// This method should halt if the given operand is not a literal.
    /// This method should halt if the register locator is not found.
    /// In the case of register members, this method should halt if the member is not found.
    fn load_literal_circuit(&self, stack: &impl StackTrait<N>, operand: &Operand<N>) -> Result<circuit::Literal<A>> {
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
    fn load_plaintext_circuit(
        &self,
        stack: &impl StackTrait<N>,
        operand: &Operand<N>,
    ) -> Result<circuit::Plaintext<A>> {
        match self.load_circuit(stack, operand)? {
            circuit::Value::Plaintext(plaintext) => Ok(plaintext),
            circuit::Value::Record(..) | circuit::Value::Future(..) => bail!("Operand must be a plaintext"),
        }
    }

    /// Assigns the given value to the given register, assuming the register is not already assigned.
    ///
    /// # Errors
    /// This method should halt if the given register is a register member.
    /// This method should halt if the given register is an input register.
    /// This method should halt if the register is already used.
    fn store_circuit(
        &mut self,
        stack: &impl StackTrait<N>,
        register: &Register<N>,
        stack_value: circuit::Value<A>,
    ) -> Result<()>;

    /// Assigns the given literal to the given register, assuming the register is not already assigned.
    ///
    /// # Errors
    /// This method should halt if the given register is a register member.
    /// This method should halt if the given register is an input register.
    /// This method should halt if the register is already used.
    fn store_literal_circuit(
        &mut self,
        stack: &impl StackTrait<N>,
        register: &Register<N>,
        literal: circuit::Literal<A>,
    ) -> Result<()> {
        self.store_circuit(stack, register, circuit::Value::Plaintext(circuit::Plaintext::from(literal)))
    }
}
