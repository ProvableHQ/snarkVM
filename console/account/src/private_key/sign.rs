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
use crate::Signature;

impl<N: Network> PrivateKey<N> {
    /// Returns a signature for the given message (as field elements) using the private key.
    pub fn sign<R: Rng + CryptoRng>(&self, message: &[Field<N>], rng: &mut R) -> Result<Signature<N>> {
        Signature::sign(self, message, rng)
    }

    /// Returns a signature for the given message (as bytes) using the private key.
    pub fn sign_bytes<R: Rng + CryptoRng>(&self, message: &[u8], rng: &mut R) -> Result<Signature<N>> {
        Signature::sign_bytes(self, message, rng)
    }

    /// Returns a signature for the given message (as bits) using the private key.
    pub fn sign_bits<R: Rng + CryptoRng>(&self, message: &[bool], rng: &mut R) -> Result<Signature<N>> {
        Signature::sign_bits(self, message, rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Address;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 100;

    #[test]
    fn test_sign_and_verify() -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample an address and a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
            let address = Address::try_from(&private_key)?;

            // Check that the signature is valid for the message.
            let message: Vec<_> = (0..i).map(|_| Uniform::rand(rng)).collect();
            let signature = private_key.sign(&message, rng)?;
            assert!(signature.verify(&address, &message));

            // Check that the signature is invalid for an incorrect message.
            let failure_message: Vec<_> = (0..i).map(|_| Uniform::rand(rng)).collect();
            if message != failure_message {
                assert!(!signature.verify(&address, &failure_message));
            }
        }
        Ok(())
    }

    #[test]
    fn test_sign_and_verify_bytes() -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample an address and a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
            let address = Address::try_from(&private_key)?;

            // Check that the signature is valid for the message.
            let message: Vec<_> = (0..i).map(|_| Uniform::rand(rng)).collect();
            let signature = private_key.sign_bytes(&message, rng)?;
            assert!(signature.verify_bytes(&address, &message));

            // Check that the signature is invalid for an incorrect message.
            let failure_message: Vec<_> = (0..i).map(|_| Uniform::rand(rng)).collect();
            if message != failure_message {
                assert!(!signature.verify_bytes(&address, &failure_message));
            }
        }
        Ok(())
    }

    #[test]
    fn test_sign_and_verify_bits() -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample an address and a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
            let address = Address::try_from(&private_key)?;

            // Check that the signature is valid for the message.
            let message: Vec<_> = (0..i).map(|_| Uniform::rand(rng)).collect();
            let signature = private_key.sign_bits(&message, rng)?;
            assert!(signature.verify_bits(&address, &message));

            // Check that the signature is invalid for an incorrect message.
            let failure_message: Vec<_> = (0..i).map(|_| Uniform::rand(rng)).collect();
            if message != failure_message {
                assert!(!signature.verify_bits(&address, &failure_message));
            }
        }
        Ok(())
    }
}
