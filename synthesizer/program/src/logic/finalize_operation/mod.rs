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

mod bits;
mod bytes;
mod serialize;
mod string;

use console::{network::prelude::*, types::Field};

/// Enum to represent the allowed set of Merkle tree operations.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum FinalizeOperation<N: Network> {
    /// Appends a mapping to the program tree, as (`mapping ID`).
    InitializeMapping(Field<N>),
    /// Inserts a key-value leaf into the mapping tree,
    /// as (`mapping ID`, `key ID`, `value ID`).
    InsertKeyValue(Field<N>, Field<N>, Field<N>),
    /// Updates the key-value leaf in the mapping tree,
    /// as (`mapping ID`, `key ID`, `value ID`).
    UpdateKeyValue(Field<N>, Field<N>, Field<N>),
    /// Removes the key-value leaf in the mapping tree,
    /// as (`mapping ID`, `key ID`).
    RemoveKeyValue(Field<N>, Field<N>),
    /// Replaces a mapping from the program tree, as (`mapping ID`).
    ReplaceMapping(Field<N>),
    /// Removes a mapping from the program tree, as (`mapping ID`).
    RemoveMapping(Field<N>),
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    /// Samples a random `InitializeMapping`.
    pub(crate) fn sample_initialize_mapping(rng: &mut TestRng) -> FinalizeOperation<CurrentNetwork> {
        FinalizeOperation::InitializeMapping(Uniform::rand(rng))
    }

    /// Samples a random `InsertKeyValue`.
    pub(crate) fn sample_insert_key_value(rng: &mut TestRng) -> FinalizeOperation<CurrentNetwork> {
        FinalizeOperation::InsertKeyValue(Uniform::rand(rng), Uniform::rand(rng), Uniform::rand(rng))
    }

    /// Samples a random `UpdateKeyValue`.
    pub(crate) fn sample_update_key_value(rng: &mut TestRng) -> FinalizeOperation<CurrentNetwork> {
        FinalizeOperation::UpdateKeyValue(Uniform::rand(rng), Uniform::rand(rng), Uniform::rand(rng))
    }

    /// Samples a random `RemoveKeyValue`.
    pub(crate) fn sample_remove_key_value(rng: &mut TestRng) -> FinalizeOperation<CurrentNetwork> {
        FinalizeOperation::RemoveKeyValue(Uniform::rand(rng), Uniform::rand(rng))
    }

    /// Samples a random `ReplaceMapping`.
    pub(crate) fn sample_replace_mapping(rng: &mut TestRng) -> FinalizeOperation<CurrentNetwork> {
        FinalizeOperation::ReplaceMapping(Uniform::rand(rng))
    }

    /// Samples a random `RemoveMapping`.
    pub(crate) fn sample_remove_mapping(rng: &mut TestRng) -> FinalizeOperation<CurrentNetwork> {
        FinalizeOperation::RemoveMapping(Uniform::rand(rng))
    }

    /// Samples a list of random `FinalizeOperation`.
    pub(crate) fn sample_finalize_operations() -> Vec<FinalizeOperation<CurrentNetwork>> {
        let rng = &mut TestRng::default();

        vec![
            sample_initialize_mapping(rng),
            sample_insert_key_value(rng),
            sample_update_key_value(rng),
            sample_remove_key_value(rng),
            sample_replace_mapping(rng),
            sample_remove_mapping(rng),
            sample_initialize_mapping(rng),
            sample_insert_key_value(rng),
            sample_update_key_value(rng),
            sample_remove_key_value(rng),
            sample_replace_mapping(rng),
            sample_remove_mapping(rng),
        ]
    }
}
