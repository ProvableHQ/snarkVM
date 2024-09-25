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

#[cfg(test)]
use snarkvm_circuit_types::environment::assert_scope;

use snarkvm_circuit_network::Aleo;
use snarkvm_circuit_types::{environment::prelude::*, Field};

pub struct GraphKey<A: Aleo> {
    /// The graph key `sk_tag` := Hash(view_key || ctr).
    sk_tag: Field<A>,
}

#[cfg(console)]
impl<A: Aleo> Inject for GraphKey<A> {
    type Primitive = console::GraphKey<A::Network>;

    /// Initializes an account graph key from the given mode and native graph key.
    fn new(mode: Mode, graph_key: Self::Primitive) -> Self {
        // Inject `sk_tag`.
        let sk_tag = Field::new(mode, graph_key.sk_tag());
        // Output the graph key.
        Self { sk_tag }
    }
}

impl<A: Aleo> GraphKey<A> {
    /// Returns the graph key.
    pub const fn sk_tag(&self) -> &Field<A> {
        &self.sk_tag
    }
}

#[cfg(console)]
impl<A: Aleo> Eject for GraphKey<A> {
    type Primitive = console::GraphKey<A::Network>;

    /// Ejects the mode of the graph key.
    fn eject_mode(&self) -> Mode {
        self.sk_tag.eject_mode()
    }

    /// Ejects the graph key.
    fn eject_value(&self) -> Self::Primitive {
        match Self::Primitive::try_from(self.sk_tag.eject_value()) {
            Ok(graph_key) => graph_key,
            Err(error) => A::halt(format!("Failed to eject the graph key: {error}")),
        }
    }
}

#[cfg(all(test, console))]
pub(crate) mod tests {
    use super::*;
    use crate::{helpers::generate_account, Circuit};

    use anyhow::Result;

    const ITERATIONS: u64 = 250;

    fn check_new(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        for i in 0..ITERATIONS {
            // Generate a view key and graph key.
            let (_, _, view_key, _) = generate_account()?;
            let graph_key = console::GraphKey::try_from(&view_key)?;

            Circuit::scope(format!("New {mode}"), || {
                let candidate = GraphKey::<Circuit>::new(mode, graph_key);
                assert_eq!(mode, candidate.eject_mode());
                assert_eq!(graph_key, candidate.eject_value());
                // TODO (howardwu): Resolve skipping the cost count checks for the burn-in round.
                if i > 0 {
                    assert_scope!(num_constants, num_public, num_private, num_constraints);
                }
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_graph_key_new_constant() -> Result<()> {
        check_new(Mode::Constant, 1, 0, 0, 0)
    }

    #[test]
    fn test_graph_key_new_public() -> Result<()> {
        check_new(Mode::Public, 0, 1, 0, 0)
    }

    #[test]
    fn test_graph_key_new_private() -> Result<()> {
        check_new(Mode::Private, 0, 0, 1, 0)
    }
}
