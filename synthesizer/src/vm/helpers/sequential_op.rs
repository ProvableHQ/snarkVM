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

use crate::vm::*;
use console::network::prelude::Network;

#[cfg(feature = "announce-blocks")]
use interprocess::local_socket::{self, Stream, prelude::*};
use std::{fmt, thread};
use tokio::sync::oneshot;
#[cfg(feature = "announce-blocks")]
use tracing::*;

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// Launches a thread dedicated to the sequential processing of storage-related
    /// operations.
    pub fn start_sequential_queue(
        &self,
        request_rx: mpsc::Receiver<SequentialOperationRequest<N>>,
    ) -> thread::JoinHandle<()> {
        #[cfg(feature = "announce-blocks")]
        let mut stream = start_block_announcement_stream();

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
                        #[cfg(feature = "announce-blocks")]
                        let ipc_payload = (block.height(), bincode::serialize(&block).unwrap()); // Infallible.
                        let ret = vm.add_next_block_inner(block);
                        #[cfg(feature = "announce-blocks")]
                        if ret.is_ok() {
                            if let Err(e) = announce_block(&mut stream, ipc_payload) {
                                error!("IPC error: {e}");
                                // Attempt to restart the stream.
                                stream = start_block_announcement_stream();
                            }
                        }
                        SequentialOperationResult::AddNextBlock(ret)
                    }
                    SequentialOperation::AtomicSpeculate(a, b, c, d, e, f) => {
                        let ret = vm.atomic_speculate_inner(a, b, c, d, e, f);
                        SequentialOperationResult::AtomicSpeculate(ret)
                    }
                    SequentialOperation::Shutdown => {
                        // The thread may be closed.
                        break;
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

        // Remember if this is a shutdown.
        let is_shutdown = matches!(op, SequentialOperation::Shutdown);

        // Prepare a oneshot channel to obtain the result of the queued operation.
        let (response_tx, response_rx) = oneshot::channel();
        let request = SequentialOperationRequest { op, response_tx };

        // This pattern match is infallible unless already shutting down the thread.
        if let Some(tx) = &*self.sequential_ops_tx.read() {
            // Send the operation to be processed sequentially.
            let _ = tx.send(request);

            // If it's a shutdown, just return None.
            if is_shutdown {
                return None;
            }

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
    Shutdown,
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
            SequentialOperation::Shutdown => {
                write!(f, "shutdown")
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

#[cfg(feature = "announce-blocks")]
fn start_block_announcement_stream() -> Option<Stream> {
    let path = std::env::var("BLOCK_ANNOUNCE_PATH")
        .map_err(|_| {
            warn!("BLOCK_ANNOUNCE_PATH env variable must be set in order to publish blocks via IPC");
        })
        .ok()?
        .to_fs_name::<local_socket::GenericFilePath>()
        .expect("Invalid path provided as the BLOCK_ANNOUNCE_PATH");

    match Stream::connect(path) {
        Ok(stream) => {
            debug!("Successfully (re)started the IPC stream for block announcements");
            Some(stream)
        }
        Err(e) => {
            warn!("Couldn't (re)start the IPC stream for block announcements: {e}");
            None
        }
    }
}

#[cfg(feature = "announce-blocks")]
fn announce_block(stream: &mut Option<Stream>, payload: (u32, Vec<u8>)) -> Result<bool> {
    if let Some(stream) = stream {
        let (block_height, block_bytes) = payload;
        debug!("Announcing block {block_height} to the IPC stream");
        let payload_size = u32::try_from(4 + block_bytes.len()).unwrap(); // Safe - blocks are smaller than 4GiB.
        stream.write_all(&payload_size.to_le_bytes())?;
        stream.write_all(&block_height.to_le_bytes())?;
        stream.write_all(&block_bytes)?;

        Ok(true)
    } else {
        *stream = start_block_announcement_stream();
        if stream.is_some() { announce_block(stream, payload) } else { Ok(false) }
    }
}
