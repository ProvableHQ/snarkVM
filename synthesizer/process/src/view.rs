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

use crate::{FinalizeRegisters, Stack};
use console::{
    network::prelude::*,
    program::{Identifier, Value},
};
use snarkvm_ledger_store::{FinalizeStorage, FinalizeStore};
use snarkvm_synthesizer_program::{
    FinalizeGlobalState,
    FinalizeRegistersState,
    FinalizeStoreTrait,
    RegistersTrait,
    StackTrait,
};

/// Evaluates a view function against the latest confirmed finalize-store state.
///
/// Evaluates whatever `stack` it is given; the caller must supply the stack for the program's
/// current edition. Prefer `VM::evaluate_view`, which resolves the stack.
pub fn evaluate_view_with_stack<N: Network, P: FinalizeStorage<N>>(
    state: FinalizeGlobalState,
    store: &FinalizeStore<N, P>,
    stack: &Stack<N>,
    view_name: &Identifier<N>,
    inputs: Vec<Value<N>>,
) -> Result<Vec<Value<N>>> {
    let confirmed = ConfirmedFinalizeStore { store };
    evaluate_view_inner(state, &confirmed, stack, view_name, inputs)
}

/// Read-only `FinalizeStoreTrait` adapter that routes mapping reads to the finalize store's
/// *confirmed* state, so a view never observes the pending writes of an in-flight atomic batch.
/// Writes bail — they are unreachable on the view path (views reject `set` / `remove` at
/// construction), but bailing here preserves that invariant if the adapter is ever passed to
/// other code.
struct ConfirmedFinalizeStore<'a, N: Network, P: FinalizeStorage<N>> {
    store: &'a FinalizeStore<N, P>,
}

impl<N: Network, P: FinalizeStorage<N>> FinalizeStoreTrait<N> for ConfirmedFinalizeStore<'_, N, P> {
    fn contains_mapping_confirmed(
        &self,
        program_id: &console::program::ProgramID<N>,
        mapping_name: &Identifier<N>,
    ) -> Result<bool> {
        self.store.contains_mapping_confirmed(program_id, mapping_name)
    }

    fn contains_mapping_speculative(
        &self,
        program_id: &console::program::ProgramID<N>,
        mapping_name: &Identifier<N>,
    ) -> Result<bool> {
        // Views only read confirmed state; the "speculative" query is answered from it too.
        self.store.contains_mapping_confirmed(program_id, mapping_name)
    }

    fn contains_key_speculative(
        &self,
        program_id: console::program::ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &console::program::Plaintext<N>,
    ) -> Result<bool> {
        self.store.contains_key_confirmed(program_id, mapping_name, key)
    }

    fn get_value_speculative(
        &self,
        program_id: console::program::ProgramID<N>,
        mapping_name: Identifier<N>,
        key: &console::program::Plaintext<N>,
    ) -> Result<Option<Value<N>>> {
        self.store.get_value_confirmed(program_id, mapping_name, key)
    }

    fn insert_key_value(
        &self,
        _program_id: console::program::ProgramID<N>,
        _mapping_name: Identifier<N>,
        _key: console::program::Plaintext<N>,
        _value: Value<N>,
    ) -> Result<snarkvm_synthesizer_program::FinalizeOperation<N>> {
        bail!("Forbidden operation: view path cannot write to the finalize store ('insert_key_value')")
    }

    fn update_key_value(
        &self,
        _program_id: console::program::ProgramID<N>,
        _mapping_name: Identifier<N>,
        _key: console::program::Plaintext<N>,
        _value: Value<N>,
    ) -> Result<snarkvm_synthesizer_program::FinalizeOperation<N>> {
        bail!("Forbidden operation: view path cannot write to the finalize store ('update_key_value')")
    }

    fn remove_key_value(
        &self,
        _program_id: console::program::ProgramID<N>,
        _mapping_name: Identifier<N>,
        _key: &console::program::Plaintext<N>,
    ) -> Result<Option<snarkvm_synthesizer_program::FinalizeOperation<N>>> {
        bail!("Forbidden operation: view path cannot write to the finalize store ('remove_key_value')")
    }
}

/// Inner evaluation of a view. Generic over the store; the public path
/// ([`evaluate_view_with_stack`]) wraps the underlying `FinalizeStore` in a
/// [`ConfirmedFinalizeStore`] adapter that pins reads to confirmed state.
pub(crate) fn evaluate_view_inner<N: Network>(
    state: FinalizeGlobalState,
    store: &dyn FinalizeStoreTrait<N>,
    stack: &Stack<N>,
    view_name: &Identifier<N>,
    inputs: Vec<Value<N>>,
) -> Result<Vec<Value<N>>> {
    // Resolve the view function in the stack's program.
    let view = stack.program().get_view_ref(view_name)?;

    // Use the cached view types (computed once at `Stack::new`).
    let types = stack.get_view_types(view_name)?;

    // Views are read-only and externally-callable: no transition is associated. Pass `None`
    // for `transition_id` and `nonce` — the only consumer (rand.chacha) is rejected by
    // `add_command`, so any future reader of these fields must handle the `None` case
    // explicitly (the trait surface makes this a compile-time obligation).
    let mut registers = FinalizeRegisters::new(state, None, *view.name(), types, None);

    // Validate the input arity.
    ensure!(
        view.inputs().len() == inputs.len(),
        "View '{}' expects {} inputs, got {}",
        view.name(),
        view.inputs().len(),
        inputs.len(),
    );

    // Reject non-plaintext inputs up-front. View input statements are typed
    // `FinalizeType::Plaintext` at construction, so the per-register store would reject
    // these as well — but with a generic type-mismatch error. Surfacing the kind here
    // gives a clearer UX.
    for (i, value) in inputs.iter().enumerate() {
        let kind = match value {
            Value::Plaintext(_) => continue,
            Value::Record(_) => "record",
            Value::Future(_) => "future",
            Value::DynamicRecord(_) => "dynamic record",
            Value::DynamicFuture(_) => "dynamic future",
        };
        bail!("View '{}' input #{i} must be a plaintext value, got a {kind}", view.name());
    }

    // Store the inputs.
    for (input_stmt, value) in view.inputs().iter().zip(inputs) {
        registers.store(stack, input_stmt.register(), value)?;
    }

    // Evaluate the commands. Views reject `await` at construction (`add_command`), so the
    // dispatch is identical to `Finalize` / `Constructor` — we share `finalize_command_except_await`
    // directly to avoid drift. `try_vm_runtime` inside that helper also gives views panic-catch
    // protection, which is desirable on the off-consensus / RPC-exposed path.
    //
    // Termination & cost bounds (prototype):
    //   - The loop is bounded by `view.commands().len()`, which is itself bounded by
    //     `N::MAX_COMMANDS` (= `u16::MAX`).
    //   - `branch_to` (used by the helper) permits forward jumps only, so the counter
    //     strictly advances and no command can re-execute. Termination is guaranteed.
    //   - Deploy-time, `view_cost_for_single_view` enforces that the worst-case body
    //     cost is `<= TRANSACTION_SPEND_LIMIT`, so a deployed view cannot register an
    //     unboundedly expensive body.
    //   - There is intentionally NO smaller per-call runtime budget below the deploy
    //     bound. A node serving repeated external view calls can therefore consume up
    //     to the deploy bound per call. Rate-limiting and indexing are expected to be
    //     handled at the snarkOS RPC layer, not here.
    let mut counter = 0;
    let mut finalize_operations: Vec<snarkvm_synthesizer_program::FinalizeOperation<N>> = Vec::new();
    while counter < view.commands().len() {
        let command = &view.commands()[counter];
        crate::finalize::finalize_command_except_await(
            Some((*stack.program_id(), *stack.program_edition())),
            Some(*registers.function_name()),
            store,
            stack,
            &mut registers,
            view.positions(),
            command,
            &mut counter,
            &mut finalize_operations,
            view.name(),
        )?;
    }
    // Defensive: views reject all write-producing commands at construction, so no finalize
    // operations should ever be emitted. Fail closed in release builds too — a regression that
    // allows a write through the type-check path must not silently leak side effects from the
    // view path, where some callers (e.g. RPC views) discard `finalize_operations` entirely.
    ensure!(
        finalize_operations.is_empty(),
        "view '{}' produced finalize operations: {finalize_operations:?}",
        view.name()
    );

    // Load the outputs.
    let mut outputs = Vec::with_capacity(view.outputs().len());
    for output in view.outputs() {
        outputs.push(registers.load(stack, output.operand())?);
    }
    Ok(outputs)
}

// These tests exercise the external `evaluate_view_with_stack` path. Tests for the in-block call
// path live at the v15 VM-tests level (where deploying a program with a finalize-calling-view
// function is straightforward).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Process;
    use console::{
        account::PrivateKey,
        network::MainnetV0,
        program::{Literal, Plaintext},
        types::U64,
    };
    use snarkvm_ledger_store::helpers::memory::FinalizeMemory;
    use snarkvm_synthesizer_program::{FinalizeStoreTrait, Program};

    type CurrentNetwork = MainnetV0;

    /// Builds a synthetic `FinalizeGlobalState` for tests. Production callers go through
    /// `VM::evaluate_view`, which constructs the state from a real block at the call site.
    fn sample_finalize_state(block_height: u32) -> FinalizeGlobalState {
        // Use `from` to avoid the BHP hash done by `new`. The seed is irrelevant for these
        // tests (no rand.chacha) and the round/timestamp aren't read either.
        FinalizeGlobalState::from(block_height as u64, block_height, None, [0u8; 32], None, None)
    }

    #[test]
    fn test_evaluate_view_simple() -> Result<()> {
        // A program with a mapping and a view function that sums two mappings for an address.
        let program = Program::<CurrentNetwork>::from_str(
            r"
program token_with_view.aleo;

mapping balances:
    key as address.public;
    value as u64.public;

mapping staked:
    key as address.public;
    value as u64.public;

function noop:
    input r0 as u64.private;
    output r0 as u64.private;

view total_balance:
    input r0 as address.public;
    get.or_use balances[r0] 0u64 into r1;
    get.or_use staked[r0] 0u64 into r2;
    add r1 r2 into r3;
    output r3 as u64.public;",
        )?;

        // Initialize a process and a stack for this program (no deployment needed here).
        let process = Process::<CurrentNetwork>::load()?;
        let stack = Stack::new(&process, &program)?;

        // Initialize the finalize store and seed mapping values.
        let finalize_store = FinalizeStore::<_, FinalizeMemory<_>>::open(aleo_std::StorageMode::new_test(None))?;

        let program_id = *program.id();
        finalize_store.initialize_mapping(program_id, Identifier::from_str("balances")?)?;
        finalize_store.initialize_mapping(program_id, Identifier::from_str("staked")?)?;

        // Pick a deterministic address.
        let mut rng = console::prelude::TestRng::default();
        let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng)?;
        let address = console::account::Address::try_from(&private_key)?;
        let address_key = Plaintext::from(Literal::Address(address));

        finalize_store.update_key_value(
            program_id,
            Identifier::from_str("balances")?,
            address_key.clone(),
            Value::Plaintext(Plaintext::from(Literal::U64(U64::new(40)))),
        )?;
        finalize_store.update_key_value(
            program_id,
            Identifier::from_str("staked")?,
            address_key.clone(),
            Value::Plaintext(Plaintext::from(Literal::U64(U64::new(2)))),
        )?;

        // Evaluate the view against the confirmed state.
        let outputs = evaluate_view_with_stack(
            sample_finalize_state(0),
            &finalize_store,
            &stack,
            &Identifier::from_str("total_balance")?,
            vec![Value::Plaintext(address_key.clone())],
        )?;

        // Expect a single u64 output equal to 42.
        assert_eq!(outputs.len(), 1);
        match &outputs[0] {
            Value::Plaintext(Plaintext::Literal(Literal::U64(v), _)) => assert_eq!(**v, 42),
            other => panic!("unexpected output: {other}"),
        }

        Ok(())
    }

    #[test]
    fn test_evaluate_view_uses_or_default_when_key_missing() -> Result<()> {
        // Same program, but view an address that's never been stored.
        let program = Program::<CurrentNetwork>::from_str(
            r"
program token_with_view.aleo;

mapping balances:
    key as address.public;
    value as u64.public;

function noop:
    input r0 as u64.private;
    output r0 as u64.private;

view fetch_balance:
    input r0 as address.public;
    get.or_use balances[r0] 7u64 into r1;
    output r1 as u64.public;",
        )?;

        let process = Process::<CurrentNetwork>::load()?;
        let stack = Stack::new(&process, &program)?;

        let finalize_store = FinalizeStore::<_, FinalizeMemory<_>>::open(aleo_std::StorageMode::new_test(None))?;
        finalize_store.initialize_mapping(*program.id(), Identifier::from_str("balances")?)?;

        let mut rng = console::prelude::TestRng::default();
        let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng)?;
        let address = console::account::Address::try_from(&private_key)?;
        let address_key = Plaintext::from(Literal::Address(address));

        let outputs = evaluate_view_with_stack(
            sample_finalize_state(0),
            &finalize_store,
            &stack,
            &Identifier::from_str("fetch_balance")?,
            vec![Value::Plaintext(address_key)],
        )?;

        assert_eq!(outputs.len(), 1);
        match &outputs[0] {
            Value::Plaintext(Plaintext::Literal(Literal::U64(v), _)) => assert_eq!(**v, 7),
            other => panic!("unexpected output: {other}"),
        }
        Ok(())
    }

    #[test]
    fn test_evaluate_view_errors_when_mapping_not_initialized() -> Result<()> {
        // Same shape of program as the other tests, but the finalize store is intentionally not
        // initialized for `balances`. The runtime path must surface the existing
        // "Mapping ... does not exist" error rather than panic or silently succeed.
        let program = Program::<CurrentNetwork>::from_str(
            r"
program token_with_view.aleo;

mapping balances:
    key as address.public;
    value as u64.public;

function noop:
    input r0 as u64.private;
    output r0 as u64.private;

view fetch_balance:
    input r0 as address.public;
    get.or_use balances[r0] 7u64 into r1;
    output r1 as u64.public;",
        )?;

        let process = Process::<CurrentNetwork>::load()?;
        let stack = Stack::new(&process, &program)?;

        // Open an empty finalize store. Note: `initialize_mapping` is deliberately NOT called.
        let finalize_store = FinalizeStore::<_, FinalizeMemory<_>>::open(aleo_std::StorageMode::new_test(None))?;

        let mut rng = console::prelude::TestRng::default();
        let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng)?;
        let address = console::account::Address::try_from(&private_key)?;
        let address_key = Plaintext::from(Literal::Address(address));

        let result = evaluate_view_with_stack(
            sample_finalize_state(0),
            &finalize_store,
            &stack,
            &Identifier::from_str("fetch_balance")?,
            vec![Value::Plaintext(address_key)],
        );

        let err = result.expect_err("expected error when mapping is not initialized").to_string();
        assert!(err.contains("does not exist"), "unexpected error message: {err}");
        Ok(())
    }

    #[test]
    fn test_evaluate_view_rejects_non_plaintext_input() -> Result<()> {
        // A view that takes a single plaintext input. We then call it with a `Value::Future`
        // and expect the entry-point pre-validation to reject with a clear "must be a plaintext
        // value, got a future" error rather than a generic store-mismatch.
        let program = Program::<CurrentNetwork>::from_str(
            r"
program vw_input_kind.aleo;

function noop:
    input r0 as u64.private;
    output r0 as u64.private;

view echo:
    input r0 as u64.public;
    add r0 0u64 into r1;
    output r1 as u64.public;",
        )?;

        let process = Process::<CurrentNetwork>::load()?;
        let stack = Stack::new(&process, &program)?;
        let finalize_store = FinalizeStore::<_, FinalizeMemory<_>>::open(aleo_std::StorageMode::new_test(None))?;

        // Build a non-plaintext Value: a Future with an empty argument list (the contents
        // don't matter; we only care that it's not Plaintext).
        let future_value =
            Value::Future(console::program::Future::new(*program.id(), Identifier::from_str("noop")?, vec![]));

        let result = evaluate_view_with_stack(
            sample_finalize_state(0),
            &finalize_store,
            &stack,
            &Identifier::from_str("echo")?,
            vec![future_value],
        );

        let err = match result {
            Ok(_) => panic!("expected error for non-plaintext input"),
            Err(err) => err.to_string(),
        };
        assert!(err.contains("future"), "unexpected error message: {err}");
        assert!(err.contains("plaintext"), "unexpected error message: {err}");
        Ok(())
    }

    #[test]
    fn test_evaluate_view_ignores_pending_writes() -> Result<()> {
        // The view path uses the `ConfirmedFinalizeStore` adapter, so reads bypass the
        // atomic-batch pending state. This test simulates an in-flight finalize batch by
        // staging a write inside an atomic batch (without committing) and then viewing:
        // the pending write must NOT be visible to the view.
        let program = Program::<CurrentNetwork>::from_str(
            r"
program vw_pending_isolate.aleo;

mapping balances:
    key as address.public;
    value as u64.public;

function noop:
    input r0 as u64.private;
    output r0 as u64.private;

view lookup:
    input r0 as address.public;
    get.or_use balances[r0] 0u64 into r1;
    output r1 as u64.public;",
        )?;

        let process = Process::<CurrentNetwork>::load()?;
        let stack = Stack::new(&process, &program)?;
        let finalize_store = FinalizeStore::<_, FinalizeMemory<_>>::open(aleo_std::StorageMode::new_test(None))?;

        let program_id = *program.id();
        let mapping_name = Identifier::from_str("balances")?;
        finalize_store.initialize_mapping(program_id, mapping_name)?;

        // Pick a deterministic address.
        let mut rng = console::prelude::TestRng::default();
        let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng)?;
        let address = console::account::Address::try_from(&private_key)?;
        let address_key = Plaintext::from(Literal::Address(address));

        // Stage a pending write inside an atomic batch — DO NOT commit it. Mirrors the
        // mid-finalize-batch state a concurrent block-production thread would produce.
        finalize_store.start_atomic();
        finalize_store.update_key_value(
            program_id,
            mapping_name,
            address_key.clone(),
            Value::Plaintext(Plaintext::from(Literal::U64(U64::new(99)))),
        )?;
        assert!(finalize_store.is_atomic_in_progress());

        // View with the batch still open: the confirmed adapter skips pending atomic-batch
        // writes, so the view sees the mapping's default (0), not the pending 99.
        let outputs = evaluate_view_with_stack(
            sample_finalize_state(0),
            &finalize_store,
            &stack,
            &Identifier::from_str("lookup")?,
            vec![Value::Plaintext(address_key)],
        )?;

        // Abort the batch (cleanup; the pending write was a fixture, not a real commit).
        finalize_store.abort_atomic();

        assert_eq!(outputs.len(), 1);
        match &outputs[0] {
            Value::Plaintext(Plaintext::Literal(Literal::U64(v), _)) => assert_eq!(
                **v, 0,
                "view should observe confirmed state (default 0), not the pending in-batch write (99)"
            ),
            other => panic!("unexpected output: {other}"),
        }
        Ok(())
    }

    #[test]
    fn test_view_can_read_block_timestamp() -> Result<()> {
        // Views get a real `FinalizeGlobalState` from the calling VM (built from the
        // current block), so `block.timestamp` is a valid operand inside a view body.
        // Drive it directly here at the process layer with a synthetic state.
        let program = Program::<CurrentNetwork>::from_str(
            r"
program vw_block_ts.aleo;

function noop:
    input r0 as u64.private;
    output r0 as u64.private;

view reads_ts:
    add block.timestamp 0i64 into r0;
    output r0 as i64.public;",
        )?;

        let process = Process::<CurrentNetwork>::load()?;
        let stack = Stack::new(&process, &program)?;
        let finalize_store = FinalizeStore::<_, FinalizeMemory<_>>::open(aleo_std::StorageMode::new_test(None))?;

        // Build a state with a non-trivial timestamp, mimicking what a real VM would supply.
        let state = FinalizeGlobalState::from(1, 1, Some(1234567890), [0u8; 32], None, None);
        let outputs =
            evaluate_view_with_stack(state, &finalize_store, &stack, &Identifier::from_str("reads_ts")?, vec![])?;

        assert_eq!(outputs.len(), 1);
        match &outputs[0] {
            Value::Plaintext(Plaintext::Literal(Literal::I64(v), _)) => assert_eq!(**v, 1234567890),
            other => panic!("expected i64 plaintext, got: {other}"),
        }
        Ok(())
    }
}
