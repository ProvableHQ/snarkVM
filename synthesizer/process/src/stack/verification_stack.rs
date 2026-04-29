// Copyright (c) 2019-2026 Provable Inc.
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

use crate::Stack;
use console::{
    account::Group,
    network::Network,
    prelude::{Result, Uniform, anyhow, bail, ensure},
    program::{
        Future,
        Identifier,
        Locator,
        Plaintext,
        PlaintextType,
        ProgramID,
        Record,
        RegisterType,
        Value,
        ValueType,
    },
    types::{Address, Field, U8, U16},
};
use indexmap::IndexMap;
use rand::{CryptoRng, Rng};
use snarkvm_synthesizer_program::{Function, Program, StackTrait};
use snarkvm_synthesizer_snark::{ProvingKey, VerifyingKey};
use std::sync::Arc;

/// A read-only `StackTrait` wrapper that resolves external programs against a
/// fixed `IndexMap` snapshot of stacks rather than the live `Process`-level map.
///
/// `VerificationStack` is intended for verification paths (e.g. `verify_execution`)
/// that need a stable view of the imported program set, even if the underlying
/// `Process` is mutated concurrently.
///
/// # Lifetimes
///
/// The wrapper borrows the `snapshot` for `'a`, so the snapshot must outlive
/// every wrapper produced by `get_external_stack` / `get_stack_global`. The
/// inner `Stack<N>` is held behind an `Arc` so it can be re-pointed cheaply
/// when external resolution swaps in a different program's stack.
pub struct VerificationStack<'a, N: Network> {
    /// The underlying stack whose program is being verified against.
    inner: Arc<Stack<N>>,
    /// A snapshot of the process's stack map captured at the start of verification.
    snapshot: &'a IndexMap<ProgramID<N>, Arc<Stack<N>>>,
}

impl<'a, N: Network> VerificationStack<'a, N> {
    /// Constructs a new `VerificationStack` wrapper from the given stack and snapshot.
    #[inline]
    pub fn new(inner: Arc<Stack<N>>, snapshot: &'a IndexMap<ProgramID<N>, Arc<Stack<N>>>) -> Self {
        Self { inner, snapshot }
    }

    /// Returns a reference to the underlying `Stack`.
    #[inline]
    pub fn inner(&self) -> &Arc<Stack<N>> {
        &self.inner
    }

    /// Returns a reference to the snapshot used for external resolution.
    #[inline]
    pub fn snapshot(&self) -> &IndexMap<ProgramID<N>, Arc<Stack<N>>> {
        self.snapshot
    }

    /// Wraps `stack` with the same `snapshot` as `self`.
    #[inline]
    fn wrap(&self, stack: Arc<Stack<N>>) -> Arc<Self> {
        Arc::new(Self { inner: stack, snapshot: self.snapshot })
    }
}

impl<'a, N: Network> StackTrait<N> for VerificationStack<'a, N> {
    fn contains_proving_key(&self, function_or_record_name: &Identifier<N>) -> bool {
        self.inner.contains_proving_key(function_or_record_name)
    }

    fn get_proving_key(&self, function_or_record_name: &Identifier<N>) -> Result<ProvingKey<N>> {
        self.inner.get_proving_key(function_or_record_name)
    }

    fn insert_proving_key(&self, function_or_record_name: &Identifier<N>, proving_key: ProvingKey<N>) -> Result<()> {
        self.inner.insert_proving_key(function_or_record_name, proving_key)
    }

    fn remove_proving_key(&self, function_or_record_name: &Identifier<N>) {
        self.inner.remove_proving_key(function_or_record_name)
    }

    fn contains_verifying_key(&self, function_or_record_name: &Identifier<N>) -> bool {
        self.inner.contains_verifying_key(function_or_record_name)
    }

    fn get_verifying_key(&self, function_or_record_name: &Identifier<N>) -> Result<VerifyingKey<N>> {
        self.inner.get_verifying_key(function_or_record_name)
    }

    fn insert_verifying_key(
        &self,
        function_or_record_name: &Identifier<N>,
        verifying_key: VerifyingKey<N>,
    ) -> Result<()> {
        self.inner.insert_verifying_key(function_or_record_name, verifying_key)
    }

    fn remove_verifying_key(&self, function_or_record_name: &Identifier<N>) {
        self.inner.remove_verifying_key(function_or_record_name)
    }

    fn matches_value_type(&self, value: &Value<N>, value_type: &ValueType<N>) -> Result<()> {
        match (value, value_type) {
            (Value::Plaintext(plaintext), ValueType::Constant(plaintext_type))
            | (Value::Plaintext(plaintext), ValueType::Public(plaintext_type))
            | (Value::Plaintext(plaintext), ValueType::Private(plaintext_type)) => {
                self.matches_plaintext(plaintext, plaintext_type)
            }
            (Value::Record(record), ValueType::Record(record_name)) => self.matches_record(record, record_name),
            (Value::Record(record), ValueType::ExternalRecord(locator)) => {
                self.matches_external_record(record, locator)
            }
            (Value::Future(future), ValueType::Future(locator)) => self.matches_future(future, locator),
            (Value::DynamicRecord(_), ValueType::DynamicRecord) => Ok(()),
            (Value::DynamicFuture(_), ValueType::DynamicFuture) => Ok(()),
            (value, _) => bail!("A value '{value}' does not match its declared value type '{value_type}'"),
        }
    }

    fn matches_register_type(&self, stack_value: &Value<N>, register_type: &RegisterType<N>) -> Result<()> {
        match (stack_value, register_type) {
            (Value::Plaintext(plaintext), RegisterType::Plaintext(plaintext_type)) => {
                self.matches_plaintext(plaintext, plaintext_type)
            }
            (Value::Record(record), RegisterType::Record(record_name)) => self.matches_record(record, record_name),
            (Value::Record(record), RegisterType::ExternalRecord(locator)) => {
                self.matches_external_record(record, locator)
            }
            (Value::Future(future), RegisterType::Future(locator)) => self.matches_future(future, locator),
            (Value::DynamicRecord(_), RegisterType::DynamicRecord) => Ok(()),
            (Value::DynamicFuture(_), RegisterType::DynamicFuture) => Ok(()),
            (value, _) => bail!("A value '{value}' does not match its declared register type '{register_type}'"),
        }
    }

    fn matches_external_record(&self, record: &Record<N, Plaintext<N>>, locator: &Locator<N>) -> Result<()> {
        let record_name = locator.resource();
        ensure!(!Program::is_reserved_keyword(record_name), "Record name '{record_name}' is reserved");
        let external_stack = self.get_external_stack(locator.program_id())?;
        external_stack.matches_record(record, record_name)
    }

    fn matches_record(&self, record: &Record<N, Plaintext<N>>, record_name: &Identifier<N>) -> Result<()> {
        // Delegating to the inner stack is sound because `matches_record` only consults
        // the program's local record types and recursively dispatches to `matches_plaintext`,
        // which does not cross program boundaries on its own.
        self.inner.matches_record(record, record_name)
    }

    fn matches_plaintext(&self, plaintext: &Plaintext<N>, plaintext_type: &PlaintextType<N>) -> Result<()> {
        // For non-`ExternalStruct` types, defer to the inner stack. For external structs,
        // re-route resolution through this wrapper so the snapshot is consulted.
        match plaintext_type {
            PlaintextType::ExternalStruct(locator) => {
                let external_stack = self.get_external_stack(locator.program_id())?;
                let new_type = PlaintextType::Struct(*locator.resource());
                external_stack.matches_plaintext(plaintext, &new_type)
            }
            _ => self.inner.matches_plaintext(plaintext, plaintext_type),
        }
    }

    fn matches_future(&self, future: &Future<N>, locator: &Locator<N>) -> Result<()> {
        // Validate the program ID and function name match the locator, then dispatch on the
        // resolved stack to ensure imports come from the snapshot.
        ensure!(future.program_id() == locator.program_id(), "Future program ID does not match");
        ensure!(future.function_name() == locator.resource(), "Future name does not match");

        let resolved_stack: Arc<Self> = if locator.program_id() == self.program_id() {
            // Same program: delegate to the inner stack via the wrapper.
            // We re-wrap to keep all matching going through `VerificationStack`'s plaintext path.
            self.wrap(self.inner.clone())
        } else {
            self.get_external_stack(locator.program_id())?
        };

        let function = resolved_stack.get_function_ref(locator.resource())?;
        let inputs = match function.finalize_logic() {
            Some(finalize_logic) => finalize_logic.inputs(),
            None => bail!("Function '{locator}' does not have a finalize block"),
        };
        ensure!(future.arguments().len() == inputs.len(), "Future arguments do not match");

        for (argument, input) in future.arguments().iter().zip(inputs.iter()) {
            use console::program::{Argument, FinalizeType};
            match (argument, input.finalize_type()) {
                (Argument::Plaintext(plaintext), FinalizeType::Plaintext(plaintext_type)) => {
                    resolved_stack.matches_plaintext(plaintext, plaintext_type)?
                }
                (Argument::Future(inner_future), FinalizeType::Future(inner_locator)) => {
                    resolved_stack.matches_future(inner_future, inner_locator)?
                }
                (Argument::DynamicFuture(_), FinalizeType::DynamicFuture) => {}
                (_, input_type) => bail!("Argument type does not match input type: expected '{input_type}'"),
            }
        }
        Ok(())
    }

    fn program(&self) -> &Program<N> {
        self.inner.program()
    }

    fn program_id(&self) -> &ProgramID<N> {
        self.inner.program_id()
    }

    fn program_address(&self) -> &Address<N> {
        self.inner.program_address()
    }

    fn program_checksum(&self) -> &[U8<N>; 32] {
        self.inner.program_checksum()
    }

    fn program_checksum_as_field(&self) -> Result<Field<N>> {
        self.inner.program_checksum_as_field()
    }

    fn program_edition(&self) -> U16<N> {
        self.inner.program_edition()
    }

    fn program_owner(&self) -> &Option<Address<N>> {
        self.inner.program_owner()
    }

    fn set_program_owner(&mut self, _program_owner: Option<Address<N>>) {
        // The wrapper is read-only and never used for setting the program owner.
        // `set_program_owner` requires `&mut self`, but `VerificationStack` only
        // exposes immutable views over `Arc<Stack<N>>`, so this is unreachable
        // for the verification call sites.
        unreachable!("VerificationStack is a read-only wrapper; set_program_owner is not supported")
    }

    /// Returns the external stack for the given program ID, consulting the snapshot
    /// rather than the live process-level stack map.
    fn get_external_stack(&self, program_id: &ProgramID<N>) -> Result<Arc<Self>> {
        // Mirror `Stack::get_external_stack`: forbid resolving to the current program,
        // and require the program to be imported by the current program.
        ensure!(
            program_id != self.program_id(),
            "Attempted to get the main program '{program_id}' as an external program."
        );
        ensure!(self.program().contains_import(program_id), "External program '{program_id}' is not imported.");
        let stack = self
            .snapshot
            .get(program_id)
            .cloned()
            .ok_or_else(|| anyhow!("External stack for '{program_id}' does not exist"))?;
        Ok(self.wrap(stack))
    }

    /// Returns the stack for the given program ID, without checking that the program
    /// is imported by the current program. Consults the snapshot rather than the
    /// live process-level stack map.
    fn get_stack_global(&self, program_id: &ProgramID<N>) -> Result<Arc<Self>> {
        let stack = self
            .snapshot
            .get(program_id)
            .cloned()
            .ok_or_else(|| anyhow!("External stack for '{program_id}' does not exist"))?;
        Ok(self.wrap(stack))
    }

    fn get_function(&self, function_name: &Identifier<N>) -> Result<Function<N>> {
        self.inner.get_function(function_name)
    }

    fn get_function_ref(&self, function_name: &Identifier<N>) -> Result<&Function<N>> {
        self.inner.get_function_ref(function_name)
    }

    // `get_minimum_number_of_calls` and `contains_dynamic_call` use the default trait
    // bodies, which traverse the call graph through `StackTrait::get_external_stack`.
    // Those calls automatically resolve via `VerificationStack`'s `get_external_stack`
    // implementation above, so the snapshot is honored for the entire walk.

    fn sample_value<R: Rng + CryptoRng>(
        &self,
        burner_address: &Address<N>,
        register_type: &RegisterType<N>,
        rng: &mut R,
    ) -> Result<Value<N>> {
        // Re-implement `sample_value` so that `ExternalRecord` resolution uses the snapshot.
        match register_type {
            RegisterType::Plaintext(plaintext_type) => {
                Ok(Value::Plaintext(self.inner.sample_plaintext(plaintext_type, rng)?))
            }
            RegisterType::Record(record_name) => {
                Ok(Value::Record(self.sample_record(burner_address, record_name, Group::rand(rng), rng)?))
            }
            RegisterType::ExternalRecord(locator) => {
                let stack = self.get_external_stack(locator.program_id())?;
                Ok(Value::Record(stack.sample_record(burner_address, locator.resource(), Group::rand(rng), rng)?))
            }
            RegisterType::Future(locator) => Ok(Value::Future(self.inner.sample_future(locator, rng)?)),
            RegisterType::DynamicRecord => Ok(Value::DynamicRecord(self.inner.sample_dynamic_record(rng)?)),
            RegisterType::DynamicFuture => Ok(Value::DynamicFuture(self.inner.sample_dynamic_future(rng)?)),
        }
    }

    fn sample_record<R: Rng + CryptoRng>(
        &self,
        burner_address: &Address<N>,
        record_name: &Identifier<N>,
        nonce: Group<N>,
        rng: &mut R,
    ) -> Result<Record<N, Plaintext<N>>> {
        // Sampling consults only the inner program's record type definition, which is
        // delegated. The subsequent layout check is rerun through the wrapper so any
        // external structs encountered during validation also flow through the snapshot.
        let record = self.inner.sample_record(burner_address, record_name, nonce, rng)?;
        self.matches_record(&record, record_name)?;
        Ok(record)
    }

    fn sample_record_using_tvk<R: Rng + CryptoRng>(
        &self,
        burner_address: &Address<N>,
        record_name: &Identifier<N>,
        tvk: Field<N>,
        index: Field<N>,
        rng: &mut R,
    ) -> Result<Record<N, Plaintext<N>>> {
        let randomizer = N::hash_to_scalar_psd2(&[tvk, index])?;
        let record_nonce = N::g_scalar_multiply(&randomizer);
        self.sample_record(burner_address, record_name, record_nonce, rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Process;
    use console::network::MainnetV0;
    use snarkvm_synthesizer_program::Program;
    use std::str::FromStr;

    type CurrentNetwork = MainnetV0;

    /// Build a snapshot `IndexMap` from a `Process` by cloning the program-id -> stack map.
    fn snapshot_from_process(
        process: &Process<CurrentNetwork>,
    ) -> IndexMap<ProgramID<CurrentNetwork>, Arc<Stack<CurrentNetwork>>> {
        let mut snapshot = IndexMap::new();
        for program_id in process.program_ids() {
            // Each `get_stack` returns a clone of the `Arc<Stack<N>>`, which shares state.
            let stack = process.get_stack(program_id).unwrap();
            snapshot.insert(program_id, stack);
        }
        snapshot
    }

    // The wrapper exposes the same program-level metadata as the inner stack.
    #[test]
    fn test_verification_stack_program_metadata_matches_inner() {
        let process = Process::<CurrentNetwork>::load().unwrap();
        let stack = process.get_stack("credits.aleo").unwrap();
        let snapshot = snapshot_from_process(&process);

        let wrapper = VerificationStack::new(stack.clone(), &snapshot);

        assert_eq!(wrapper.program_id(), stack.program_id());
        assert_eq!(wrapper.program_address(), stack.program_address());
        assert_eq!(wrapper.program_checksum(), stack.program_checksum());
        assert_eq!(wrapper.program_edition(), stack.program_edition());
        assert_eq!(wrapper.program_owner(), stack.program_owner());
        assert_eq!(wrapper.program(), stack.program());
    }

    // External resolution consults the snapshot, not the live process map.
    #[test]
    fn test_verification_stack_get_external_stack_uses_snapshot() {
        // Build a process with a helper and a caller that imports it.
        let helper_program = Program::<CurrentNetwork>::from_str(
            r"
program helper.aleo;

function dynamic_helper:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as field.public;
    call.dynamic r0 r1 r2;",
        )
        .unwrap();
        let caller_program = Program::<CurrentNetwork>::from_str(
            r"
import helper.aleo;

program caller.aleo;

function caller_func:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as field.public;
    call helper.aleo/dynamic_helper r0 r1 r2;",
        )
        .unwrap();
        let mut process = Process::<CurrentNetwork>::load().unwrap();
        process.add_program(&helper_program).unwrap();
        process.add_program(&caller_program).unwrap();

        // Capture the snapshot before any further mutation.
        let snapshot = snapshot_from_process(&process);
        let caller_stack = process.get_stack("caller.aleo").unwrap();
        let wrapper = VerificationStack::new(caller_stack, &snapshot);

        // External resolution against `helper.aleo` should succeed using the snapshot.
        let helper_id = ProgramID::<CurrentNetwork>::from_str("helper.aleo").unwrap();
        let external = wrapper.get_external_stack(&helper_id).unwrap();
        assert_eq!(external.program_id(), &helper_id);

        // Resolving the current program ID as external is not allowed.
        let caller_id = ProgramID::<CurrentNetwork>::from_str("caller.aleo").unwrap();
        assert!(wrapper.get_external_stack(&caller_id).is_err());

        // Resolving a non-imported program is not allowed.
        let other_id = ProgramID::<CurrentNetwork>::from_str("credits.aleo").unwrap();
        assert!(wrapper.get_external_stack(&other_id).is_err());

        // `get_stack_global` allows any program in the snapshot.
        assert!(wrapper.get_stack_global(&helper_id).is_ok());
        assert!(wrapper.get_stack_global(&other_id).is_ok());
    }

    // The default trait bodies for `get_minimum_number_of_calls` and
    // `contains_dynamic_call` should produce the same result on the wrapper as on the
    // underlying `Stack`.
    #[test]
    fn test_verification_stack_call_graph_walks_match_inner() {
        let helper_program = Program::<CurrentNetwork>::from_str(
            r"
program helper.aleo;

function dynamic_helper:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as field.public;
    call.dynamic r0 r1 r2;",
        )
        .unwrap();
        let caller_program = Program::<CurrentNetwork>::from_str(
            r"
import helper.aleo;

program caller.aleo;

function caller_func:
    input r0 as field.public;
    input r1 as field.public;
    input r2 as field.public;
    call helper.aleo/dynamic_helper r0 r1 r2;",
        )
        .unwrap();
        let mut process = Process::<CurrentNetwork>::load().unwrap();
        process.add_program(&helper_program).unwrap();
        process.add_program(&caller_program).unwrap();

        let snapshot = snapshot_from_process(&process);
        let caller_stack = process.get_stack("caller.aleo").unwrap();
        let function_name = Identifier::<CurrentNetwork>::from_str("caller_func").unwrap();

        let inner_min = caller_stack.get_minimum_number_of_calls(&function_name).unwrap();
        let inner_has_dynamic = caller_stack.contains_dynamic_call(&function_name).unwrap();

        let wrapper = VerificationStack::new(caller_stack, &snapshot);
        assert_eq!(wrapper.get_minimum_number_of_calls(&function_name).unwrap(), inner_min);
        assert_eq!(wrapper.contains_dynamic_call(&function_name).unwrap(), inner_has_dynamic);
    }
}
