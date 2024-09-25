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

impl<N: Network> Request<N> {
    /// Returns `true` if the request is valid, and `false` otherwise.
    ///
    /// Verifies (challenge == challenge') && (address == address') && (serial_numbers == serial_numbers') where:
    ///     challenge' := HashToScalar(r * G, pk_sig, pr_sig, signer, \[tvk, tcm, function ID, input IDs\])
    pub fn verify(&self, input_types: &[ValueType<N>], is_root: bool) -> bool {
        // Verify the transition public key, transition view key, and transition commitment are well-formed.
        {
            // Compute the transition commitment `tcm` as `Hash(tvk)`.
            match N::hash_psd2(&[self.tvk]) {
                Ok(tcm) => {
                    // Ensure the computed transition commitment matches.
                    if tcm != self.tcm {
                        eprintln!("Invalid transition commitment in request.");
                        return false;
                    }
                }
                Err(error) => {
                    eprintln!("Failed to compute transition commitment in request verification: {error}");
                    return false;
                }
            }
        }

        // Retrieve the challenge from the signature.
        let challenge = self.signature.challenge();
        // Retrieve the response from the signature.
        let response = self.signature.response();

        // Compute the function ID.
        let function_id = match compute_function_id(&self.network_id, &self.program_id, &self.function_name) {
            Ok(function_id) => function_id,
            Err(error) => {
                eprintln!("Failed to construct the function ID: {error}");
                return false;
            }
        };

        // Compute the 'is_root' field.
        let is_root = if is_root { Field::<N>::one() } else { Field::<N>::zero() };

        // Construct the signature message as `[tvk, tcm, function ID, input IDs]`.
        let mut message = Vec::with_capacity(3 + self.input_ids.len());
        message.push(self.tvk);
        message.push(self.tcm);
        message.push(function_id);
        message.push(is_root);

        if let Err(error) = self.input_ids.iter().zip_eq(&self.inputs).zip_eq(input_types).enumerate().try_for_each(
            |(index, ((input_id, input), input_type))| {
                match input_id {
                    // A constant input is hashed (using `tcm`) to a field element.
                    InputID::Constant(input_hash) => {
                        // Ensure the input is a plaintext.
                        ensure!(matches!(input, Value::Plaintext(..)), "Expected a plaintext input");

                        // Construct the (console) input index as a field element.
                        let index = Field::from_u16(u16::try_from(index).or_halt_with::<N>("Input index exceeds u16"));
                        // Construct the preimage as `(function ID || input || tcm || index)`.
                        let mut preimage = Vec::new();
                        preimage.push(function_id);
                        preimage.extend(input.to_fields()?);
                        preimage.push(self.tcm);
                        preimage.push(index);
                        // Hash the input to a field element.
                        let candidate_hash = N::hash_psd8(&preimage)?;
                        // Ensure the input hash matches.
                        ensure!(*input_hash == candidate_hash, "Expected a constant input with the same hash");

                        // Add the input hash to the message.
                        message.push(candidate_hash);
                    }
                    // A public input is hashed (using `tcm`) to a field element.
                    InputID::Public(input_hash) => {
                        // Ensure the input is a plaintext.
                        ensure!(matches!(input, Value::Plaintext(..)), "Expected a plaintext input");

                        // Construct the (console) input index as a field element.
                        let index = Field::from_u16(u16::try_from(index).or_halt_with::<N>("Input index exceeds u16"));
                        // Construct the preimage as `(function ID || input || tcm || index)`.
                        let mut preimage = Vec::new();
                        preimage.push(function_id);
                        preimage.extend(input.to_fields()?);
                        preimage.push(self.tcm);
                        preimage.push(index);
                        // Hash the input to a field element.
                        let candidate_hash = N::hash_psd8(&preimage)?;
                        // Ensure the input hash matches.
                        ensure!(*input_hash == candidate_hash, "Expected a public input with the same hash");

                        // Add the input hash to the message.
                        message.push(candidate_hash);
                    }
                    // A private input is encrypted (using `tvk`) and hashed to a field element.
                    InputID::Private(input_hash) => {
                        // Ensure the input is a plaintext.
                        ensure!(matches!(input, Value::Plaintext(..)), "Expected a plaintext input");

                        // Construct the (console) input index as a field element.
                        let index = Field::from_u16(u16::try_from(index).or_halt_with::<N>("Input index exceeds u16"));
                        // Compute the input view key as `Hash(function ID || tvk || index)`.
                        let input_view_key = N::hash_psd4(&[function_id, self.tvk, index])?;
                        // Compute the ciphertext.
                        let ciphertext = match &input {
                            Value::Plaintext(plaintext) => plaintext.encrypt_symmetric(input_view_key)?,
                            // Ensure the input is a plaintext.
                            Value::Record(..) => bail!("Expected a plaintext input, found a record input"),
                            Value::Future(..) => bail!("Expected a plaintext input, found a future input"),
                        };
                        // Hash the ciphertext to a field element.
                        let candidate_hash = N::hash_psd8(&ciphertext.to_fields()?)?;
                        // Ensure the input hash matches.
                        ensure!(*input_hash == candidate_hash, "Expected a private input with the same commitment");

                        // Add the input hash to the message.
                        message.push(candidate_hash);
                    }
                    // A record input is computed to its serial number.
                    InputID::Record(commitment, gamma, serial_number, tag) => {
                        // Retrieve the record.
                        let record = match &input {
                            Value::Record(record) => record,
                            // Ensure the input is a record.
                            Value::Plaintext(..) => bail!("Expected a record input, found a plaintext input"),
                            Value::Future(..) => bail!("Expected a record input, found a future input"),
                        };
                        // Retrieve the record name.
                        let record_name = match input_type {
                            ValueType::Record(record_name) => record_name,
                            // Ensure the input type is a record.
                            _ => bail!("Expected a record type at input {index}"),
                        };
                        // Ensure the record belongs to the signer.
                        ensure!(**record.owner() == self.signer, "Input record does not belong to the signer");

                        // Compute the record commitment.
                        let candidate_cm = record.to_commitment(&self.program_id, record_name)?;
                        // Ensure the commitment matches.
                        ensure!(*commitment == candidate_cm, "Expected a record input with the same commitment");

                        // Compute the `candidate_sn` from `gamma`.
                        let candidate_sn = Record::<N, Plaintext<N>>::serial_number_from_gamma(gamma, *commitment)?;
                        // Ensure the serial number matches.
                        ensure!(*serial_number == candidate_sn, "Expected a record input with the same serial number");

                        // Compute the generator `H` as `HashToGroup(commitment)`.
                        let h = N::hash_to_group_psd2(&[N::serial_number_domain(), *commitment])?;
                        // Compute `h_r` as `(challenge * gamma) + (response * H)`, equivalent to `r * H`.
                        let h_r = (*gamma * challenge) + (h * response);

                        // Compute the tag as `Hash(sk_tag || commitment)`.
                        let candidate_tag = N::hash_psd2(&[self.sk_tag, *commitment])?;
                        // Ensure the tag matches.
                        ensure!(*tag == candidate_tag, "Expected a record input with the same tag");

                        // Add (`H`, `r * H`, `gamma`, `tag`) to the message.
                        message.extend([h, h_r, *gamma].iter().map(|point| point.to_x_coordinate()));
                        message.push(*tag);
                    }
                    // An external record input is hashed (using `tvk`) to a field element.
                    InputID::ExternalRecord(input_hash) => {
                        // Ensure the input is a record.
                        ensure!(matches!(input, Value::Record(..)), "Expected a record input");

                        // Construct the (console) input index as a field element.
                        let index = Field::from_u16(u16::try_from(index).or_halt_with::<N>("Input index exceeds u16"));
                        // Construct the preimage as `(function ID || input || tvk || index)`.
                        let mut preimage = Vec::new();
                        preimage.push(function_id);
                        preimage.extend(input.to_fields()?);
                        preimage.push(self.tvk);
                        preimage.push(index);
                        // Hash the input to a field element.
                        let candidate_hash = N::hash_psd8(&preimage)?;
                        // Ensure the input hash matches.
                        ensure!(*input_hash == candidate_hash, "Expected a locator input with the same hash");

                        // Add the input hash to the message.
                        message.push(candidate_hash);
                    }
                }
                Ok(())
            },
        ) {
            eprintln!("Request verification failed on input checks: {error}");
            return false;
        }

        // Verify the signature.
        self.signature.verify(&self.signer, &message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_account::PrivateKey;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    pub(crate) const ITERATIONS: usize = 1000;

    #[test]
    fn test_sign_and_verify() {
        let rng = &mut TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random private key and address.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
            let address = Address::try_from(&private_key).unwrap();

            // Construct a program ID and function name.
            let program_id = ProgramID::from_str("token.aleo").unwrap();
            let function_name = Identifier::from_str("transfer").unwrap();

            // Prepare a record belonging to the address.
            let record_string = format!(
                "{{ owner: {address}.private, token_amount: 100u64.private, _nonce: 2293253577170800572742339369209137467208538700597121244293392265726446806023group.public }}"
            );

            // Construct four inputs.
            let input_constant = Value::from_str("{ token_amount: 9876543210u128 }").unwrap();
            let input_public = Value::from_str("{ token_amount: 9876543210u128 }").unwrap();
            let input_private = Value::from_str("{ token_amount: 9876543210u128 }").unwrap();
            let input_record = Value::from_str(&record_string).unwrap();
            let input_external_record = Value::from_str(&record_string).unwrap();
            let inputs = [input_constant, input_public, input_private, input_record, input_external_record];

            // Construct the input types.
            let input_types = vec![
                ValueType::from_str("amount.constant").unwrap(),
                ValueType::from_str("amount.public").unwrap(),
                ValueType::from_str("amount.private").unwrap(),
                ValueType::from_str("token.record").unwrap(),
                ValueType::from_str("token.aleo/token.record").unwrap(),
            ];

            // Sample 'root_tvk'.
            let root_tvk = None;
            // Sample 'is_root'.
            let is_root = Uniform::rand(rng);

            // Compute the signed request.
            let request = Request::sign(
                &private_key,
                program_id,
                function_name,
                inputs.into_iter(),
                &input_types,
                root_tvk,
                is_root,
                rng,
            )
            .unwrap();
            assert!(request.verify(&input_types, is_root));
        }
    }
}
