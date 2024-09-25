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

mod to_compute_key;
mod to_view_key;

#[cfg(test)]
use snarkvm_circuit_types::environment::assert_scope;

use crate::{ComputeKey, ViewKey};
use snarkvm_circuit_network::Aleo;
use snarkvm_circuit_types::{environment::prelude::*, Scalar};

pub struct PrivateKey<A: Aleo> {
    /// The signature secret key.
    sk_sig: Scalar<A>,
    /// The signature secret randomizer.
    r_sig: Scalar<A>,
}

#[cfg(console)]
impl<A: Aleo> Inject for PrivateKey<A> {
    type Primitive = console::PrivateKey<A::Network>;

    /// Initializes an account private key from the given mode and native private key.
    fn new(mode: Mode, private_key: Self::Primitive) -> Self {
        Self { sk_sig: Scalar::new(mode, private_key.sk_sig()), r_sig: Scalar::new(mode, private_key.r_sig()) }
    }
}

impl<A: Aleo> PrivateKey<A> {
    /// Returns the signature secret key.
    pub const fn sk_sig(&self) -> &Scalar<A> {
        &self.sk_sig
    }

    /// Returns the signature randomizer.
    pub const fn r_sig(&self) -> &Scalar<A> {
        &self.r_sig
    }
}

#[cfg(console)]
impl<A: Aleo> Eject for PrivateKey<A> {
    type Primitive = (console::Scalar<A::Network>, console::Scalar<A::Network>);

    /// Ejects the mode of the account private key.
    fn eject_mode(&self) -> Mode {
        (&self.sk_sig, &self.r_sig).eject_mode()
    }

    /// Ejects the account private key as `(sk_sig, r_sig)`.
    fn eject_value(&self) -> Self::Primitive {
        (&self.sk_sig, &self.r_sig).eject_value()
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
            let (private_key, _compute_key, _view_key, _address) = generate_account()?;

            // Retrieve the native private key components.
            let sk_sig = private_key.sk_sig();
            let r_sig = private_key.r_sig();

            Circuit::scope(format!("New {mode}"), || {
                let candidate = PrivateKey::<Circuit>::new(mode, private_key);
                assert_eq!(mode, candidate.eject_mode());
                assert_eq!((sk_sig, r_sig), candidate.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_private_key_new_constant() -> Result<()> {
        check_new(Mode::Constant, 2, 0, 0, 0)
    }

    #[test]
    fn test_private_key_new_public() -> Result<()> {
        check_new(Mode::Public, 0, 2, 0, 0)
    }

    #[test]
    fn test_private_key_new_private() -> Result<()> {
        check_new(Mode::Private, 0, 0, 2, 0)
    }
}
