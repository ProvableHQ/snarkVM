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

pub mod memory;
#[cfg(feature = "rocks")]
pub mod rocksdb;

#[cfg(test)]
pub(crate) mod test_helpers;

mod traits;
pub use traits::*;

/// This macro executes the given block of operations as a new atomic write batch IFF there is no
/// atomic write batch in progress yet. This ensures that complex atomic operations consisting of
/// multiple lower-level operations - which might also need to be atomic if executed individually -
/// are executed as a single large atomic operation regardless.
#[macro_export]
macro_rules! atomic_batch_scope {
    ($self:expr, $ops:block) => {{
        // Check if an atomic batch write is already in progress. If there isn't one, this means
        // this operation is a "top-level" one and is the one to start and finalize the batch.
        let is_atomic_in_progress = $self.is_atomic_in_progress();

        // Start an atomic batch write operation IFF it's not already part of one.
        match is_atomic_in_progress {
            true => $self.atomic_checkpoint(),
            false => $self.start_atomic(),
        }

        // Wrap the operations that should be batched in a closure to be able to rewind the batch on error.
        let run_atomic_ops = || -> Result<_> { $ops };

        // Run the atomic operations.
        match run_atomic_ops() {
            // Save this atomic batch scope and return.
            Ok(result) => match is_atomic_in_progress {
                // A 'true' implies this is a nested atomic batch scope.
                true => {
                    // Once a nested batch scope is completed, clear its checkpoint.
                    // Until a new checkpoint is established,
                    // we can now only rewind to a previous (higher-level) checkpoint.
                    $self.clear_latest_checkpoint();
                    Ok(result)
                }
                // A 'false' implies this is the top-level calling scope.
                // Commit the atomic batch IFF it's the top-level calling scope.
                false => $self.finish_atomic().map(|_| result),
            },
            // Rewind this atomic batch scope.
            Err(err) => {
                $self.atomic_rewind();
                Err(err)
            }
        }
    }};
}

/// A top-level helper macro to perform the finalize operation on a list of transactions.
#[macro_export]
macro_rules! atomic_finalize {
    ($self:expr, $finalize_mode:expr, $ops:block) => {{
        // Ensure that there is no atomic batch write in progress.
        if $self.is_atomic_in_progress() {
            // We intentionally 'bail!' here instead of passing an Err() to the caller because
            // this is a top-level operation and the caller must fix the issue.
            bail!("Cannot start an atomic batch write operation while another one is already in progress.")
        }

        // Start the atomic batch.
        $self.start_atomic();

        // Run the atomic operations.
        //
        // Wrap the operations that should be batched in a closure to be able to abort the entire
        // write batch if any of them fails.
        #[allow(clippy::redundant_closure_call)]
        match ($finalize_mode, || -> Result<_, String> { $ops }()) {
            // If this is a successful real run, commit the atomic batch.
            (FinalizeMode::RealRun, Ok(result)) => {
                $self.finish_atomic()?;
                Ok(result)
            }
            // If this is a failed real run, abort the atomic batch.
            (FinalizeMode::RealRun, Err(error_msg)) => {
                $self.abort_atomic();
                Err(anyhow!("Failed to finalize transactions - {error_msg}"))
            }
            // If this is a successful dry run, abort the atomic batch.
            (FinalizeMode::DryRun, Ok(result)) => {
                $self.abort_atomic();
                Ok(result)
            }
            // If this is a failed dry run, abort the atomic batch.
            (FinalizeMode::DryRun, Err(error_msg)) => {
                $self.abort_atomic();
                Err(anyhow!("Failed to speculate on transactions - {error_msg}"))
            }
        }
    }};
}
