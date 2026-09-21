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

use crate::{Stack, vm::*};
use console::network::prelude::Network;
use snarkvm_ledger_block::RejectedReason;

use indexmap::IndexMap;
use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, atomic::Ordering},
    thread,
};
use tokio::sync::oneshot;

/// Identifies one construct-path speculate so hash binding cannot attach to a later candidate.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SpeculationId(u64);

/// State retained after a successful construct-path speculate, so check can skip a second dry-run
/// and add can finish the pending finalize batch instead of replaying it.
pub(crate) struct SelfConstructed<N: Network> {
    /// Opaque id of this speculate, used to bind and take the matching entry.
    pub id: SpeculationId,
    /// Set after the candidate block is built.
    pub hash: Option<N::BlockHash>,
    /// Finalize operations from the construct-path speculate, used by `Block::verify`.
    pub finalize_operations: Vec<FinalizeOperation<N>>,
    /// Deployment stacks from this speculate. Inserted into `Process` when the block is committed.
    pub staged_stacks: IndexMap<ProgramID<N>, Arc<Stack<N>>>,
    /// Rejection reasons recorded by this speculate. RealRun inserts any that are still pending.
    pub rejected_reasons: HashMap<N::TransactionID, RejectedReason<N>>,
    /// When `true`, the finalize-store atomic batch is still open and must be finished or aborted.
    pub batch_kept: bool,
}

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// Launches a thread dedicated to the sequential processing of storage-related
    /// operations.
    pub fn start_sequential_queue(
        &self,
        request_rx: mpsc::Receiver<SequentialOperationRequest<N>>,
    ) -> thread::JoinHandle<()> {
        // Spawn a dedicated thread.
        let vm = self.clone();
        thread::spawn(move || {
            // Sequentially process incoming operations.
            while let Ok(request) = request_rx.recv() {
                let SequentialOperationRequest { op, response_tx, queued_at } = request;
                let op_label = match &op {
                    SequentialOperation::AddNextBlock(_) => "add_next_block",
                    SequentialOperation::AtomicSpeculate { .. } => "atomic_speculate",
                    SequentialOperation::DiscardKeptSpeculation => "discard_kept_speculation",
                };
                let queue_wait = queued_at.elapsed().as_secs_f64();
                #[cfg(feature = "metrics")]
                {
                    snarkvm_metrics::histogram_label(
                        snarkvm_metrics::vm::SEQUENTIAL_OP_QUEUE_WAIT_SECONDS,
                        "op",
                        op_label.to_string(),
                        queue_wait,
                    );
                    if matches!(op, SequentialOperation::AtomicSpeculate { .. }) {
                        snarkvm_metrics::histogram_label(
                            snarkvm_metrics::vm::SPECULATE_STAGE_DURATION_SECONDS,
                            "stage",
                            "queue".to_string(),
                            queue_wait,
                        );
                    }
                }
                let _ = (queued_at, op_label, queue_wait);
                trace!("Sequentially processing operation '{op}'");

                // Perform the queued operation.
                let ret = match op {
                    SequentialOperation::AddNextBlock(block) => {
                        let ret = vm.add_next_block_inner(block);
                        SequentialOperationResult::AddNextBlock(ret)
                    }
                    SequentialOperation::AtomicSpeculate {
                        state,
                        time_since_last_block,
                        coinbase_reward,
                        ratifications,
                        solutions,
                        transactions,
                        keep,
                    } => {
                        let ret = vm.atomic_speculate_inner(
                            state,
                            time_since_last_block,
                            coinbase_reward,
                            ratifications,
                            solutions,
                            transactions,
                            keep,
                        );
                        SequentialOperationResult::AtomicSpeculate(ret)
                    }
                    SequentialOperation::DiscardKeptSpeculation => {
                        vm.discard_kept_speculation_inner();
                        SequentialOperationResult::DiscardKeptSpeculation
                    }
                };

                // Relay the results of the operation to the caller.
                let _ = response_tx.send(ret);
            }
        })
    }

    /// Sends the given operation to the thread used for sequential processing.
    pub fn run_sequential_operation(&self, op: SequentialOperation<N>) -> Option<SequentialOperationResult<N>> {
        trace!("Queuing operation '{op}' for sequential processing");

        // Prepare a oneshot channel to obtain the result of the queued operation.
        let (response_tx, response_rx) = oneshot::channel();
        let request = SequentialOperationRequest { op, response_tx, queued_at: std::time::Instant::now() };

        // This pattern match is infallible unless already shutting down the thread.
        if let Some(tx) = &*self.sequential_ops_tx.read() {
            // Send the operation to be processed sequentially.
            let _ = tx.send(request);

            // Wait for the result of the queued operation. This is a blocking method,
            // and will panic in async contexts (which doesn't happen in production, as
            // we already perform all these operations within blocking tasks).
            let Ok(response) = response_rx.blocking_recv() else {
                return None;
            };

            Some(response)
        } else {
            None
        }
    }

    /// A safeguard used to ensure that the given operation is processed in the thread
    /// enforcing sequential processing of operations.
    pub fn ensure_sequential_processing(&self) {
        assert_eq!(thread::current().id(), self.sequential_ops_thread.lock().as_ref().unwrap().thread().id());
    }

    /// Returns `true` when the caller is the sequential operations thread.
    fn is_on_sequential_thread(&self) -> bool {
        self.sequential_ops_thread.lock().as_ref().is_some_and(|handle| handle.thread().id() == thread::current().id())
    }

    /// Allocates an opaque id for one construct-path speculate.
    pub(crate) fn allocate_speculation_id(&self) -> SpeculationId {
        SpeculationId(self.next_speculation_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Records construct-path speculate outputs for skip-check and optional keep-batch commit.
    pub(crate) fn store_self_constructed(
        &self,
        finalize_operations: Vec<FinalizeOperation<N>>,
        staged_stacks: IndexMap<ProgramID<N>, Arc<Stack<N>>>,
        rejected_reasons: HashMap<N::TransactionID, RejectedReason<N>>,
        id: SpeculationId,
        batch_kept: bool,
    ) {
        *self.self_constructed.lock() =
            Some(SelfConstructed { id, hash: None, finalize_operations, staged_stacks, rejected_reasons, batch_kept });
    }

    /// Removes rejection reasons from the current speculate entry, leaving the rest in place.
    pub(crate) fn take_rejected_reasons(&self) -> HashMap<N::TransactionID, RejectedReason<N>> {
        self.self_constructed
            .lock()
            .as_mut()
            .map(|constructed| std::mem::take(&mut constructed.rejected_reasons))
            .unwrap_or_default()
    }

    /// Associates a constructed block hash with the matching construct-path speculate.
    pub fn bind_self_constructed_hash(&self, id: SpeculationId, hash: N::BlockHash) {
        if let Some(constructed) = self.self_constructed.lock().as_mut()
            && constructed.id == id
        {
            constructed.hash = Some(hash);
        }
    }

    /// Returns finalize operations from construct-path speculate when `hash` matches.
    pub(crate) fn self_constructed_ops_for(&self, hash: N::BlockHash) -> Option<Vec<FinalizeOperation<N>>> {
        self.self_constructed
            .lock()
            .as_ref()
            .and_then(|constructed| (constructed.hash == Some(hash)).then(|| constructed.finalize_operations.clone()))
    }

    /// Takes a kept finalize batch when it belongs to `hash`.
    pub(crate) fn take_kept_matching(&self, hash: N::BlockHash) -> Option<SelfConstructed<N>> {
        let mut constructed = self.self_constructed.lock();
        match constructed.as_ref() {
            Some(entry) if entry.hash == Some(hash) && entry.batch_kept => constructed.take(),
            _ => None,
        }
    }

    /// Aborts a kept finalize batch and drops parked stacks. Safe to call when nothing is kept.
    pub fn discard_kept_speculation(&self) {
        if self.is_on_sequential_thread() {
            self.discard_kept_speculation_inner();
            return;
        }
        let _ = self.run_sequential_operation(SequentialOperation::DiscardKeptSpeculation);
    }

    /// Queues a kept-batch abort without waiting, so `Drop` can run from an async context.
    pub(crate) fn discard_kept_speculation_on_drop(&self) {
        if self.is_on_sequential_thread() {
            self.discard_kept_speculation_inner();
            return;
        }
        let (response_tx, _response_rx) = oneshot::channel();
        if let Some(tx) = &*self.sequential_ops_tx.read() {
            let _ = tx.send(SequentialOperationRequest {
                op: SequentialOperation::DiscardKeptSpeculation,
                response_tx,
                queued_at: std::time::Instant::now(),
            });
        }
    }

    /// Aborts a kept finalize batch. Must run on the sequential operations thread.
    pub(crate) fn discard_kept_speculation_inner(&self) {
        let Some(kept) = self.self_constructed.lock().take() else {
            return;
        };
        if !kept.batch_kept && !kept.rejected_reasons.is_empty() {
            warn!("There are pending rejection reasons, clearing them up: {:?}", kept.rejected_reasons);
        }
        if kept.batch_kept && self.finalize_store().is_atomic_in_progress() {
            self.finalize_store().abort_atomic();
        }
    }
}

/// An operation intended to be executed only in a sequential fashion.
pub enum SequentialOperation<N: Network> {
    AddNextBlock(Block<N>),
    AtomicSpeculate {
        state: FinalizeGlobalState,
        time_since_last_block: i64,
        coinbase_reward: Option<u64>,
        ratifications: Vec<Ratify<N>>,
        solutions: Solutions<N>,
        transactions: Vec<Transaction<N>>,
        keep: Option<SpeculationId>,
    },
    DiscardKeptSpeculation,
}

impl<N: Network> fmt::Display for SequentialOperation<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SequentialOperation::AddNextBlock(block) => {
                write!(f, "add block ({})", block.hash())
            }
            SequentialOperation::AtomicSpeculate { state, .. } => {
                write!(f, "atomic speculate (height {}, round {})", state.block_height(), state.block_round())
            }
            SequentialOperation::DiscardKeptSpeculation => {
                write!(f, "discard kept speculation")
            }
        }
    }
}

/// A sequential operation paired with a oneshot sender used to return its result.
pub struct SequentialOperationRequest<N: Network> {
    op: SequentialOperation<N>,
    response_tx: oneshot::Sender<SequentialOperationResult<N>>,
    queued_at: std::time::Instant,
}

/// Represents the results of all the sequential operations.
pub enum SequentialOperationResult<N: Network> {
    AddNextBlock(Result<()>),
    AtomicSpeculate(
        Result<(
            Ratifications<N>,
            Vec<ConfirmedTransaction<N>>,
            Vec<(Transaction<N>, String)>,
            Vec<FinalizeOperation<N>>,
        )>,
    ),
    DiscardKeptSpeculation,
}

#[cfg(test)]
mod tests {
    use crate::vm::test_helpers::{CurrentNetwork, sample_vm};
    use console::{network::prelude::Network, types::Field};
    use indexmap::IndexMap;

    #[test]
    fn bind_self_constructed_hash_ignores_stale_speculation_id() {
        let vm = sample_vm();
        let hash_a = <CurrentNetwork as Network>::BlockHash::from(Field::<CurrentNetwork>::from_u64(1));
        let hash_b = <CurrentNetwork as Network>::BlockHash::from(Field::<CurrentNetwork>::from_u64(2));

        let id_a = vm.allocate_speculation_id();
        let id_b = vm.allocate_speculation_id();
        vm.store_self_constructed(Vec::new(), IndexMap::new(), Default::default(), id_a, false);
        vm.store_self_constructed(Vec::new(), IndexMap::new(), Default::default(), id_b, false);

        vm.bind_self_constructed_hash(id_a, hash_a);
        assert!(vm.self_constructed_ops_for(hash_a).is_none());

        vm.bind_self_constructed_hash(id_b, hash_b);
        assert!(vm.self_constructed_ops_for(hash_b).is_some());
        assert!(vm.self_constructed_ops_for(hash_a).is_none());
    }
}
