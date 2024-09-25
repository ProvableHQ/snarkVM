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

mod bytes;
mod serialize;
mod string;
mod to_bits;

use snarkvm_console_network::prelude::*;
use snarkvm_console_types::Field;

/// The transition leaf version.
const TRANSITION_LEAF_VERSION: u8 = 1u8;

/// The Merkle leaf for an input or output ID in the transition.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct TransitionLeaf<N: Network> {
    /// The version of the Merkle leaf.
    version: u8,
    /// The index of the Merkle leaf.
    index: u8,
    /// The variant of the Merkle leaf.
    variant: u8,
    /// The ID.
    id: Field<N>,
}

impl<N: Network> TransitionLeaf<N> {
    /// Initializes a new instance of `TransitionLeaf`.
    pub const fn new_with_version(index: u8, variant: u8, id: Field<N>) -> Self {
        Self { version: TRANSITION_LEAF_VERSION, index, variant, id }
    }

    /// Initializes a new instance of `TransitionLeaf`.
    pub const fn from(version: u8, index: u8, variant: u8, id: Field<N>) -> Self {
        Self { version, index, variant, id }
    }

    /// Returns the version of the Merkle leaf.
    pub const fn version(&self) -> u8 {
        self.version
    }

    /// Returns the index of the Merkle leaf.
    pub const fn index(&self) -> u8 {
        self.index
    }

    /// Returns the variant of the Merkle leaf.
    pub const fn variant(&self) -> u8 {
        self.variant
    }

    /// Returns the ID in the Merkle leaf.
    pub const fn id(&self) -> Field<N> {
        self.id
    }
}

#[cfg(test)]
mod test_helpers {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    pub(super) fn sample_leaf(rng: &mut TestRng) -> TransitionLeaf<CurrentNetwork> {
        // Construct a new leaf.
        TransitionLeaf::new_with_version(rng.gen(), rng.gen(), Uniform::rand(rng))
    }
}
