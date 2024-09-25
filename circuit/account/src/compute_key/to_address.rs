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

use super::*;

impl<A: Aleo> ComputeKey<A> {
    /// Returns the account address for this account compute key.
    pub fn to_address(&self) -> Address<A> {
        // Compute pk_prf := G^sk_prf.
        let pk_prf = A::g_scalar_multiply(&self.sk_prf);
        // Compute the address := pk_sig + pr_sig + pk_prf.
        Address::from_group(&self.pk_sig + &self.pr_sig + pk_prf)
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use crate::{helpers::generate_account, Circuit};

    use anyhow::Result;

    const ITERATIONS: u64 = 100;

    fn check_to_address(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        for i in 0..ITERATIONS {
            // Generate a private key, compute key, view key, and address.
            let (_private_key, compute_key, _view_key, address) = generate_account()?;

            // Initialize the compute key.
            let candidate = ComputeKey::<Circuit>::new(mode, compute_key);

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = candidate.to_address();
                assert_eq!(*address, candidate.to_group().eject_value());
                // TODO (howardwu): Resolve skipping the cost count checks for the burn-in round.
                if i > 0 {
                    assert_scope!(<=num_constants, num_public, num_private, num_constraints);
                }
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_to_address_constant() -> Result<()> {
        check_to_address(Mode::Constant, 1008, 0, 0, 0)
    }

    #[test]
    fn test_to_address_public() -> Result<()> {
        check_to_address(Mode::Public, 504, 0, 1260, 1260)
    }

    #[test]
    fn test_to_address_private() -> Result<()> {
        check_to_address(Mode::Private, 504, 0, 1260, 1260)
    }
}
