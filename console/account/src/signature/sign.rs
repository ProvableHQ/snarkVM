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

impl<N: Network> Signature<N> {
    /// Returns a signature `(challenge, response, compute_key)` for a given message and RNG, where:
    ///     challenge := HashToScalar(nonce * G, pk_sig, pr_sig, address, message)
    ///     response := nonce - challenge * private_key.sk_sig()
    pub fn sign<R: Rng + CryptoRng>(private_key: &PrivateKey<N>, message: &[Field<N>], rng: &mut R) -> Result<Self> {
        // Ensure the number of field elements does not exceed the maximum allowed size.
        if message.len() > N::MAX_DATA_SIZE_IN_FIELDS as usize {
            bail!("Cannot sign the message: the message exceeds maximum allowed size")
        }

        // Sample a random nonce from the scalar field.
        let nonce = Scalar::rand(rng);
        // Compute `g_r` as `nonce * G`.
        let g_r = N::g_scalar_multiply(&nonce);

        // Derive the compute key from the private key.
        let compute_key = ComputeKey::try_from(private_key)?;
        // Retrieve pk_sig.
        let pk_sig = compute_key.pk_sig();
        // Retrieve pr_sig.
        let pr_sig = compute_key.pr_sig();

        // Derive the address from the compute key.
        let address = Address::try_from(compute_key)?;

        // Construct the hash input as (r * G, pk_sig, pr_sig, address, message).
        let mut preimage = Vec::with_capacity(4 + message.len());
        preimage.extend([g_r, pk_sig, pr_sig, *address].map(|point| point.to_x_coordinate()));
        preimage.extend(message);

        // Compute the verifier challenge.
        let challenge = N::hash_to_scalar_psd8(&preimage)?;
        // Compute the prover response.
        let response = nonce - (challenge * private_key.sk_sig());

        // Output the signature.
        Ok(Self { challenge, response, compute_key })
    }

    /// Returns a signature for the given message (as bytes) using the private key.
    pub fn sign_bytes<R: Rng + CryptoRng>(
        private_key: &PrivateKey<N>,
        message: &[u8],
        rng: &mut R,
    ) -> Result<Signature<N>> {
        // Convert the message into bits, and sign the message.
        Self::sign_bits(private_key, &message.to_bits_le(), rng)
    }

    /// Returns a signature for the given message (as bits) using the private key.
    pub fn sign_bits<R: Rng + CryptoRng>(
        private_key: &PrivateKey<N>,
        message: &[bool],
        rng: &mut R,
    ) -> Result<Signature<N>> {
        // Pack the bits into field elements.
        let fields =
            message.chunks(Field::<N>::size_in_data_bits()).map(Field::from_bits_le).collect::<Result<Vec<_>>>()?;
        // Sign the message.
        Self::sign(private_key, &fields, rng)
    }
}
