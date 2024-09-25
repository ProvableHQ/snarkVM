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

mod from_private_key;
mod to_address;

#[cfg(test)]
use snarkvm_circuit_types::environment::assert_scope;

use crate::PrivateKey;
use snarkvm_circuit_network::Aleo;
use snarkvm_circuit_types::{environment::prelude::*, Address, Scalar};

use core::ops::Deref;

/// The account view key is able to decrypt records and ciphertext.
pub struct ViewKey<A: Aleo>(Scalar<A>, OnceCell<Address<A>>);

#[cfg(console)]
impl<A: Aleo> Inject for ViewKey<A> {
    type Primitive = console::ViewKey<A::Network>;

    /// Initializes an account view key from the given mode and scalar field element.
    fn new(mode: Mode, view_key: Self::Primitive) -> ViewKey<A> {
        Self(Scalar::new(mode, *view_key), Default::default())
    }
}

#[cfg(console)]
impl<A: Aleo> Eject for ViewKey<A> {
    type Primitive = console::ViewKey<A::Network>;

    /// Ejects the mode of the view key.
    fn eject_mode(&self) -> Mode {
        self.0.eject_mode()
    }

    /// Ejects the view key as a scalar field element.
    fn eject_value(&self) -> Self::Primitive {
        Self::Primitive::from_scalar(self.0.eject_value())
    }
}

impl<A: Aleo> Deref for ViewKey<A> {
    type Target = Scalar<A>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use crate::{helpers::generate_account, Circuit};

    use anyhow::Result;

    const ITERATIONS: u64 = 500;

    fn check_new(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        for _ in 0..ITERATIONS {
            // Generate a private key, compute key, view key, and address.
            let (_private_key, _compute_key, view_key, _address) = generate_account()?;

            Circuit::scope(format!("New {mode}"), || {
                let candidate = ViewKey::<Circuit>::new(mode, view_key);
                assert_eq!(mode, candidate.eject_mode());
                assert_eq!(view_key, candidate.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_view_key_new_constant() -> Result<()> {
        check_new(Mode::Constant, 1, 0, 0, 0)
    }

    #[test]
    fn test_view_key_new_public() -> Result<()> {
        check_new(Mode::Public, 0, 1, 0, 0)
    }

    #[test]
    fn test_view_key_new_private() -> Result<()> {
        check_new(Mode::Private, 0, 0, 1, 0)
    }
}
