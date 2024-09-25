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

impl<A: Aleo> Signature<A> {
    /// Returns `true` if the signature is valid for the given `address` and `message`.
    pub fn verify(&self, address: &Address<A>, message: &[Field<A>]) -> Boolean<A> {
        // Retrieve pk_sig.
        let pk_sig = self.compute_key.pk_sig();
        // Retrieve pr_sig.
        let pr_sig = self.compute_key.pr_sig();

        // Compute `g_nonce` := (response * G) + (challenge * pk_sig).
        let g_nonce = A::g_scalar_multiply(&self.response) + (pk_sig * &self.challenge);

        // Construct the hash input as (nonce * G, pk_sig, pr_sig, address, message).
        let mut preimage = Vec::with_capacity(4 + message.len());
        preimage.extend([&g_nonce, pk_sig, pr_sig].map(|point| point.to_x_coordinate()));
        preimage.push(address.to_field());
        preimage.extend_from_slice(message);

        // Compute the candidate verifier challenge.
        let candidate_challenge = A::hash_to_scalar_psd8(&preimage);
        // Compute the candidate address.
        let candidate_address = self.compute_key.to_address();

        // Return `true` if the challenge and address is valid.
        self.challenge.is_equal(&candidate_challenge) & address.is_equal(&candidate_address)
    }
}

#[cfg(all(test, console))]
pub(crate) mod tests {
    use super::*;
    use crate::{helpers::generate_account, Circuit};
    use snarkvm_circuit_types::Group;
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    const ITERATIONS: u64 = 50;

    fn check_verify(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Generate a private key, compute key, view key, and address.
            let (private_key, _compute_key, _view_key, address) = generate_account()?;

            // Generate a signature.
            let message = [Field::new(mode, Uniform::rand(rng)), Field::new(mode, Uniform::rand(rng))];
            let signature = console::Signature::sign(&private_key, &message.eject_value(), rng)?;

            // Initialize the signature and address.
            let signature = Signature::<Circuit>::new(mode, signature);
            let address = Address::new(mode, address);

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = signature.verify(&address, &message);
                assert!(candidate.eject_value());
                // TODO (howardwu): Resolve skipping the cost count checks for the burn-in round.
                if i > 0 {
                    assert_scope!(<=num_constants, num_public, num_private, num_constraints);
                }
            });
            Circuit::reset();
        }
        Ok(())
    }

    fn check_verify_large(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Generate a private key, compute key, view key, and address.
            let (private_key, _compute_key, _view_key, address) = generate_account()?;

            // Generate a signature.
            let message = [
                Address::from_group(Group::new(mode, Uniform::rand(rng))).to_field(),
                Field::from_boolean(&Boolean::new(mode, Uniform::rand(rng))),
                Field::new(mode, Uniform::rand(rng)),
                Group::new(mode, Uniform::rand(rng)).to_x_coordinate(),
                Scalar::new(mode, Uniform::rand(rng)).to_field(),
            ];
            let signature = console::Signature::sign(&private_key, &message.eject_value(), rng)?;

            // Initialize the signature and address.
            let signature = Signature::<Circuit>::new(mode, signature);
            let address = Address::new(mode, address);

            Circuit::scope(&format!("{mode} {i}"), || {
                let candidate = signature.verify(&address, &message);
                assert!(candidate.eject_value());
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
    fn test_verify_constant() -> Result<()> {
        check_verify(Mode::Constant, 4514, 0, 0, 0)
    }

    #[test]
    fn test_verify_public() -> Result<()> {
        check_verify(Mode::Public, 1757, 0, 7783, 7789)
    }

    #[test]
    fn test_verify_private() -> Result<()> {
        check_verify(Mode::Private, 1757, 0, 7783, 7789)
    }

    #[test]
    fn test_verify_large_constant() -> Result<()> {
        check_verify_large(Mode::Constant, 4514, 0, 0, 0)
    }

    #[test]
    fn test_verify_large_public() -> Result<()> {
        check_verify_large(Mode::Public, 1757, 0, 8308, 8314)
    }

    #[test]
    fn test_verify_large_private() -> Result<()> {
        check_verify_large(Mode::Private, 1757, 0, 8308, 8314)
    }
}
