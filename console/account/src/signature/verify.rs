// Copyright (c) 2019-2026 Provable Inc.
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

// V1 Verification methods
impl<N: Network> Signature<N> {
    /// Verifies (challenge == challenge') && (address == address') where:
    ///     challenge' := HashToScalar(response * G + challenge * pk_sig, pk_sig, pr_sig, address, message)
    #[deprecated(note="Please migrate to `verify_v2`")]
    pub fn verify(&self, address: &Address<N>, message: &[Field<N>]) -> bool {
        self.verify_internal(address, message, &[])
    }

    /// Verifies a signature for the given address and message (as bytes).
    #[deprecated(note="Please migrate to `verify_bytes_v2`")]
    pub fn verify_bytes(&self, address: &Address<N>, message: &[u8]) -> bool {
        #[allow(deprecated)]
        // Convert the message into bits, and verify the signature.
        self.verify_bits(address, &message.to_bits_le())
    }

    /// Verifies a signature for the given address and message (as bits).
    #[deprecated(note="Please migrate to `verify_bits_v2`")]
    pub fn verify_bits(&self, address: &Address<N>, message: &[bool]) -> bool {
        // Pack the bits into field elements.
        match message.chunks(Field::<N>::size_in_data_bits()).map(Field::from_bits_le).collect::<Result<Vec<_>>>() {
            #[allow(deprecated)]
            Ok(fields) => self.verify(address, &fields),
            Err(error) => {
                eprintln!("Failed to verify signature: {error}");
                false
            }
        }
    }
}

// V2 Verification methods
impl<N: Network> Signature<N> {
    /// Verifies (challenge == challenge') && (address == address') where:
    ///     challenge' := HashToScalar(ALEO_SIGNATURE_V2, response * G + challenge * pk_sig, pk_sig, pr_sig, address, message)
    pub fn verify_v2(&self, address: &Address<N>, message: &[Field<N>]) -> bool {
        let prefix = Field::<N>::new_domain_separator(SIGNATURE_V2_PREFIX);
        self.verify_internal(address, message, &[prefix])
    }

    /// Verifies a signature produced with `sign_bytes_v2` for the given address and message (as bytes).
    pub fn verify_bytes_v2(&self, address: &Address<N>, message: &[u8]) -> bool {
        // Convert the message into bits, and verify the signature.
        self.verify_bits_v2(address, &message.to_bits_le())
    }

    /// Verifies a signature produced with `sign_bytes_raw_v2` for the given address and message (as bytes).
    /// Message length is not encoded and must be checked by the caller if relevant.
    pub fn verify_bytes_raw_v2(&self, address: &Address<N>, message: &[u8]) -> bool {
        // Convert the message into bits, and verify the signature.
        self.verify_bits_raw_v2(address, &message.to_bits_le())
    }

    /// Verifies a signature produced with `sign_bits_v2` for the given address and message (as bits).
    pub fn verify_bits_v2(&self, address: &Address<N>, message: &[bool]) -> bool {
        // Encode the number of bits of the message as a field element:
        if let Ok(message_length_u128) = u128::try_from(message.len()) {
            let message_length_field = Field::<N>::from_u128(message_length_u128);

            // Pack the bits into field elements.
            match message.chunks(Field::<N>::size_in_data_bits()).map(Field::from_bits_le).collect::<Result<Vec<_>>>() {
                Ok(fields) => {
                    let mut message_with_length = Vec::with_capacity(fields.len() + 1);
                    message_with_length.push(message_length_field);
                    message_with_length.extend(fields);
                    self.verify_v2(address, &message_with_length)
                }
                Err(error) => {
                    eprintln!("Failed to verify signature: {error}");
                    false
                }
            }
        } else {
            eprintln!("Failed to verify signature: number of bits in the mesage does not fit in a u128");
            false
        }
    }

    /// Verifies a signature produced with `sign_bits_raw_v2` for the given address and message (as bits).
    /// Message length is not encoded and must be checked by the caller if relevant.
    pub fn verify_bits_raw_v2(&self, address: &Address<N>, message: &[bool]) -> bool {
        // Pack the bits into field elements.
        match message.chunks(Field::<N>::size_in_data_bits()).map(Field::from_bits_le).collect::<Result<Vec<_>>>() {
            // TODO (Antonio) re-introduce
            // Ok(fields) => self.verify_v2(address, &fields),
            Ok(fields) => {
                for f in fields.iter() {
                    println!("  f: {f}");
                }
                self.verify_v2(address, &fields)
            }
            Err(error) => {
                eprintln!("Failed to verify signature: {error}");
                false
            }
        }
    }
}

// Internal functions common to several verification versions.
impl<N: Network> Signature<N> {
    /// Verifies a signature produced with `sign` or `sign_v2` for the given address and message.
    fn verify_internal(&self, address: &Address<N>, message: &[Field<N>], prefix: &[Field<N>]) -> bool {
        // Ensure the number of field elements does not exceed the maximum allowed size.
        if message.len() > N::MAX_DATA_SIZE_IN_FIELDS as usize {
            eprintln!("Cannot sign the signature: the signed message exceeds maximum allowed size");
            return false;
        }

        // Retrieve pk_sig.
        let pk_sig = self.compute_key.pk_sig();
        // Retrieve pr_sig.
        let pr_sig = self.compute_key.pr_sig();

        // Compute `g_r` := (response * G) + (challenge * pk_sig).
        let g_r = N::g_scalar_multiply(&self.response) + (pk_sig * self.challenge);

        // Construct the hash input as (prefix [if present], r * G, pk_sig, pr_sig, address, message).
        let mut preimage = Vec::with_capacity(prefix.len() + 4 + message.len());
        preimage.extend(prefix);
        preimage.extend([g_r, pk_sig, pr_sig, **address].map(|point| point.to_x_coordinate()));
        preimage.extend(message);

        // Hash to derive the verifier challenge, and return `false` if this operation fails.
        let candidate_challenge = match N::hash_to_scalar_psd8(&preimage) {
            // Output the computed candidate challenge.
            Ok(candidate_challenge) => candidate_challenge,
            // Return `false` if the challenge errored.
            Err(_) => return false,
        };

        // Derive the address from the compute key, and return `false` if this operation fails.
        let candidate_address = match Address::try_from(self.compute_key) {
            // Output the computed candidate address.
            Ok(candidate_address) => candidate_address,
            // Return `false` if the address errored.
            Err(_) => return false,
        };

        // Return `true` if the candidate challenge and address are correct.
        self.challenge == candidate_challenge && *address == candidate_address
    }
}

#[cfg(test)]
#[cfg(feature = "private_key")]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 100;

    #[test]
    #[allow(deprecated)]
    fn test_sign_and_verify() -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample an address and a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
            let address = Address::try_from(&private_key)?;

            // Check that the v1 and v2 signatures are valid for the message.
            let message: Vec<Field<CurrentNetwork>> = (0..i).map(|_| Uniform::rand(rng)).collect();

            let signature_v1 = Signature::sign(&private_key, &message, rng)?;
            assert!(signature_v1.verify(&address, &message));

            let signature_v2 = Signature::sign_v2(&private_key, &message, rng)?;
            assert!(signature_v2.verify_v2(&address, &message));

            // Check that the signature is invalid for an incorrect message.
            let failure_message: Vec<Field<CurrentNetwork>> = (0..i).map(|_| Uniform::rand(rng)).collect();
            if message != failure_message {
                assert!(!signature_v1.verify(&address, &failure_message));
                assert!(!signature_v2.verify_v2(&address, &failure_message));
            }

            // Sanity-check that the v1 signature doesn't verify under verify_v2 and viceversa
            assert!(!signature_v1.verify_v2(&address, &message));
            assert!(!signature_v2.verify(&address, &message));
        }
        Ok(())
    }

    #[test]
    #[allow(deprecated)]
    fn test_sign_and_verify_bytes() -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample an address and a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
            let address = Address::try_from(&private_key)?;

            // Check that the v1 and v2 signatures are valid for the message.
            let message: Vec<u8> = (0..i).map(|_| Uniform::rand(rng)).collect();

            let signature_v1 = Signature::sign_bytes(&private_key, &message, rng)?;
            assert!(signature_v1.verify_bytes(&address, &message));

            let signature_v2 = Signature::sign_bytes_v2(&private_key, &message, rng)?;
            assert!(signature_v2.verify_bytes_v2(&address, &message));

            let signature_raw_v2 = Signature::sign_bytes_raw_v2(&private_key, &message, rng)?;
            assert!(signature_raw_v2.verify_bytes_raw_v2(&address, &message));

            // Check that the signatures are invalid for an incorrect message.
            let failure_message: Vec<u8> = (0..i).map(|_| Uniform::rand(rng)).collect();
            if message != failure_message {
                assert!(!signature_v1.verify_bytes(&address, &failure_message));
                assert!(!signature_v2.verify_bytes_v2(&address, &failure_message));
                assert!(!signature_raw_v2.verify_bytes_raw_v2(&address, &failure_message));
            }

            // Sanity-check that the v1 signature doesn't verify under verify_bytes_v2 and viceversa,
            // and that the raw signature doesn't verify under verify_bytes_v2 and viceversa
            assert!(!signature_v1.verify_bytes_v2(&address, &message));
            assert!(!signature_v2.verify_bytes(&address, &message));
            assert!(!signature_v2.verify_bytes_raw_v2(&address, &message));
            assert!(!signature_raw_v2.verify_bytes_v2(&address, &message));
        }
        Ok(())
    }

    #[test]
    #[allow(deprecated)]
    fn test_sign_and_verify_bits() -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample an address and a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
            let address = Address::try_from(&private_key)?;

            // Check that the v1 and v2 signatures are valid for the message.
            let message: Vec<bool> = (0..i).map(|_| Uniform::rand(rng)).collect();

            let signature_v1 = Signature::sign_bits(&private_key, &message, rng)?;
            assert!(signature_v1.verify_bits(&address, &message));

            let signature_v2 = Signature::sign_bits_v2(&private_key, &message, rng)?;
            assert!(signature_v2.verify_bits_v2(&address, &message));

            let signature_raw_v2 = Signature::sign_bits_raw_v2(&private_key, &message, rng)?;
            assert!(signature_raw_v2.verify_bits_raw_v2(&address, &message));

            // Check that the signature is invalid for an incorrect message.
            let failure_message: Vec<bool> = (0..i).map(|_| Uniform::rand(rng)).collect();
            if message != failure_message {
                assert!(!signature_v1.verify_bits(&address, &failure_message));
                assert!(!signature_v2.verify_bits_v2(&address, &failure_message));
                assert!(!signature_raw_v2.verify_bits_raw_v2(&address, &failure_message));
            }

            // Sanity-check that the v1 signature doesn't verify under verify_bits_v2 and viceversa,
            // and that the raw signature doesn't verify under verify_bits_v2 and viceversa
            assert!(!signature_v1.verify_bits_v2(&address, &message));
            assert!(!signature_v2.verify_bits(&address, &message));
            assert!(!signature_v2.verify_bits_raw_v2(&address, &message));
            assert!(!signature_raw_v2.verify_bits_v2(&address, &message));
        }
        Ok(())
    }

    #[test]
    fn test_sign_and_verify_bits_v2_padding() -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample an address and a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
            let address = Address::try_from(&private_key)?;

            // Construct a message and a copy with an extra zero.
            let message: Vec<bool> = (0..i).map(|_| Uniform::rand(rng)).collect();
            let mut padded_message = message.clone();
            padded_message.push(false);

            let signature = Signature::sign_bits_v2(&private_key, &message, rng)?;
            let signature_padded = Signature::sign_bits_v2(&private_key, &padded_message, rng)?;

            // Check the signature of the padded message does not verify on the unpadded one and viceversa
            assert!(!signature.verify_bits_v2(&address, &padded_message));
            assert!(!signature_padded.verify_bits_v2(&address, &message));

            // Check the two signatures verify as expected
            assert!(signature.verify_bits_v2(&address, &message));
            assert!(signature_padded.verify_bits_v2(&address, &padded_message));
        }
        Ok(())
    }

    #[test]
    fn test_sign_and_verify_bytes_v2_padding() -> Result<()> {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample an address and a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
            let address = Address::try_from(&private_key)?;

            // Construct a message and a copy with an extra zero byte.
            let message: Vec<u8> = (0..i).map(|_| Uniform::rand(rng)).collect();
            let mut padded_message = message.clone();
            padded_message.push(0u8);

            let signature = Signature::sign_bytes_v2(&private_key, &message, rng)?;
            let signature_padded = Signature::sign_bytes_v2(&private_key, &padded_message, rng)?;

            // Check the signature of the padded message does not verify on the unpadded one and viceversa
            assert!(!signature.verify_bytes_v2(&address, &padded_message));
            assert!(!signature_padded.verify_bytes_v2(&address, &message));

            // Check the two signatures verify as expected
            assert!(signature.verify_bytes_v2(&address, &message));
            assert!(signature_padded.verify_bytes_v2(&address, &padded_message));
        }
        Ok(())
    }
}
