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

mod bytes;
mod serialize;
mod string;

use super::*;

/// The reason a transaction was rejected.
#[derive(Clone, PartialEq, Eq)]
pub enum RejectedReason {
    /// The transaction was rejected due to a duplicate program ID deployment in the same block.
    DuplicateProgramID(String),

    /// The transaction was rejected due to a failed finalize command. (locator, index, command).
    /// Note: We do not log the actual error message from the finalize command, as it may contain
    /// sensitive information or lead to DOS vectors by storing string representations of large structs.
    Finalize(String, usize, String),

    /// The transaction was rejected due to a VM error not captured by a finalize command.
    NonFinalize(String),
}

impl RejectedReason {
    /// Initializes the rejected reason from an indexed finalize error.
    pub fn from_indexed_finalize_error(indexed_finalize_error: IndexedFinalizeError) -> Self {
        match indexed_finalize_error.command {
            Some((index, command)) => {
                Self::Finalize(indexed_finalize_error.locator.to_string(), index, command.to_string())
            }
            None => Self::NonFinalize(indexed_finalize_error.locator.to_string()),
        }
    }
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;

    /// Returns one instance of each `RejectedReason` variant for testing.
    pub(crate) fn sample_rejected_reasons() -> Vec<RejectedReason> {
        vec![
            RejectedReason::DuplicateProgramID("credits.aleo".to_string()),
            RejectedReason::Finalize("credits.aleo/transfer_public".to_string(), 3, "set r0 r1".to_string()),
            RejectedReason::NonFinalize("credits.aleo/bond_public".to_string()),
        ]
    }
}
