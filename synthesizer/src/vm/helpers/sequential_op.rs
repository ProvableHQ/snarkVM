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

use crate::vm::*;
use console::network::prelude::Network;

use std::{fmt, thread};
use tokio::sync::oneshot;

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
                let SequentialOperationRequest { op, response_tx } = request;
                debug!("Sequentially processing operation '{op}'");

                // Perform the queued operation.
                let ret = match op {
                    SequentialOperation::AddNextBlock(block) => {
                        let ret = vm.add_next_block_inner(block);
                        SequentialOperationResult::AddNextBlock(ret)
                    }
                    SequentialOperation::AtomicSpeculate(a, b, c, d, e, f) => {
                        let ret = vm.atomic_speculate_inner(a, b, c, d, e, f);
                        SequentialOperationResult::AtomicSpeculate(ret)
                    }
                };

                // Relay the results of the operation to the caller.
                let _ = response_tx.send(ret);
            }
        })
    }

    /// Sends the given operation to the thread used for sequential processing.
    pub fn run_sequential_operation(&self, op: SequentialOperation<N>) -> Option<SequentialOperationResult<N>> {
        debug!("Queuing operation '{op}' for sequential processing");

        // Prepare a oneshot channel to obtain the result of the queued operation.
        let (response_tx, response_rx) = oneshot::channel();
        let request = SequentialOperationRequest { op, response_tx };

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
}

/// An operation intended to be executed only in a sequential fashion.
pub enum SequentialOperation<N: Network> {
    AddNextBlock(Block<N>),
    AtomicSpeculate(FinalizeGlobalState, i64, Option<u64>, Vec<Ratify<N>>, Solutions<N>, Vec<Transaction<N>>),
}

impl<N: Network> fmt::Display for SequentialOperation<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SequentialOperation::AddNextBlock(block) => {
                write!(f, "add block ({})", block.hash())
            }
            SequentialOperation::AtomicSpeculate(state, ..) => {
                write!(f, "atomic speculate (height {}, round {})", state.block_height(), state.block_round())
            }
        }
    }
}

/// A sequential operation paired with a oneshot sender used to return its result.
pub struct SequentialOperationRequest<N: Network> {
    op: SequentialOperation<N>,
    response_tx: oneshot::Sender<SequentialOperationResult<N>>,
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
}
