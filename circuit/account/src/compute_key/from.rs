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

impl<A: Aleo> From<(Group<A>, Group<A>)> for ComputeKey<A> {
    /// Derives the account compute key from a tuple `(pk_sig, pr_sig)`.
    fn from((pk_sig, pr_sig): (Group<A>, Group<A>)) -> Self {
        // Compute sk_prf := HashToScalar(pk_sig || pr_sig).
        let sk_prf = A::hash_to_scalar_psd4(&[pk_sig.to_x_coordinate(), pr_sig.to_x_coordinate()]);
        // Output the compute key.
        Self { pk_sig, pr_sig, sk_prf }
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use crate::{helpers::generate_account, Circuit};

    use anyhow::Result;
    use snarkvm_circuit_network::AleoV0;

    const ITERATIONS: u64 = 100;

    fn check_from(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        for i in 0..ITERATIONS {
            // Generate a private key, compute key, view key, and address.
            let (_, compute_key, _, _) = generate_account()?;

            // Initialize the pk_sig and pr_sig.
            let pk_sig = Group::new(mode, compute_key.pk_sig());
            let pr_sig = Group::new(mode, compute_key.pr_sig());

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = ComputeKey::<AleoV0>::from((pk_sig, pr_sig));
                assert_eq!(compute_key, candidate.eject_value());
                if i > 0 {
                    assert_scope!(num_constants, num_public, num_private, num_constraints);
                }
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_from_constant() -> Result<()> {
        check_from(Mode::Constant, 254, 0, 0, 0)
    }

    #[test]
    fn test_from_public() -> Result<()> {
        check_from(Mode::Public, 1, 0, 845, 847)
    }

    #[test]
    fn test_from_private() -> Result<()> {
        check_from(Mode::Private, 1, 0, 845, 847)
    }
}
