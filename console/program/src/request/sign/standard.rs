// Copyright (c) 2019-2025 Provable Inc.
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
    /// Returns the request for a given private key, program ID, function name, inputs, input types, and RNG, where:
    ///     challenge := HashToScalar(r * G, pk_sig, pr_sig, signer, \[tvk, tcm, function ID, is_root, program checksum?, input IDs\])
    ///     response := r - challenge * sk_sig
    /// The program checksum must be provided if the program has a constructor and should not be provided otherwise.
    pub fn sign<R: Rng + CryptoRng>(
        private_key: &PrivateKey<N>,
        program_id: ProgramID<N>,
        function_name: Identifier<N>,
        inputs: impl ExactSizeIterator<Item = impl TryInto<Value<N>>>,
        input_types: &[ValueType<N>],
        root_tvk: Option<Field<N>>,
        is_root: bool,
        program_checksum: Option<Field<N>>,
        rng: &mut R,
    ) -> Result<Self> {
        // Ensure the number of inputs matches the number of input types.
        if input_types.len() != inputs.len() {
            bail!("Expected {} inputs, but {} were provided.", input_types.len(), inputs.len())
        }

        // Parse the inputs.
        let mut parsed_inputs = Vec::with_capacity(inputs.len());
        for (i, input) in inputs.enumerate() {
            let input = input.try_into().map_err(|_| anyhow!("Failed to parse input #{i}"))?;
            parsed_inputs.push(input);
        }
        let inputs = parsed_inputs;

        // Retrieve `sk_sig`.
        let sk_sig = private_key.sk_sig();

        // Derive the compute key.
        let compute_key = ComputeKey::try_from(private_key)?;
        // Retrieve `pk_sig`.
        let pk_sig = compute_key.pk_sig();
        // Retrieve `pr_sig`.
        let pr_sig = compute_key.pr_sig();

        // Derive the view key.
        let view_key = ViewKey::try_from((private_key, &compute_key))?;
        // Derive `sk_tag` from the graph key.
        let sk_tag = GraphKey::try_from(view_key)?.sk_tag();

        // Sample a random nonce.
        let nonce = Field::<N>::rand(rng);
        // Compute a `r` as `HashToScalar(sk_sig || nonce)`. Note: This is the transition secret key `tsk`.
        let r = N::hash_to_scalar_psd4(&[N::serial_number_domain(), sk_sig.to_field()?, nonce])?;
        // Compute `g_r` as `r * G`. Note: This is the transition public key `tpk`.
        let g_r = N::g_scalar_multiply(&r);

        // Derive the signer from the compute key.
        let signer = Address::try_from(compute_key)?;
        // Compute the transition view key `tvk` as `r * signer`.
        let tvk = (*signer * r).to_x_coordinate();
        // Compute the transition commitment `tcm` as `Hash(tvk)`.
        let tcm = N::hash_psd2(&[tvk])?;
        // Compute the signer commitment `scm` as `Hash(signer || root_tvk)`.
        let root_tvk = root_tvk.unwrap_or(tvk);
        let scm = N::hash_psd2(&[signer.deref().to_x_coordinate(), root_tvk])?;
        // Compute 'is_root' as a field element.
        let is_root = if is_root { Field::<N>::one() } else { Field::<N>::zero() };

        // Retrieve the network ID.
        let network_id = U16::new(N::ID);
        // Compute the function ID.
        let function_id = compute_function_id(&network_id, &program_id, &function_name, false)?;

        // Construct the hash input as `(r * G, pk_sig, pr_sig, signer, [tvk, tcm, function ID, is_root, program checksum?, input IDs])`.
        let mut message = Vec::with_capacity(9 + 2 * inputs.len());
        message.extend([g_r, pk_sig, pr_sig, *signer].map(|point| point.to_x_coordinate()));
        message.extend([tvk, tcm, function_id, is_root]);
        // Add the program checksum to the hash input if it was provided.
        if let Some(program_checksum) = program_checksum {
            message.push(program_checksum);
        }

        // Initialize a vector to store the input IDs.
        let mut input_ids = Vec::with_capacity(inputs.len());

        // Prepare the inputs.
        for (index, (input, input_type)) in inputs.iter().zip_eq(input_types).enumerate() {
            // Convert index to u16.
            let index = u16::try_from(index).or_halt_with::<N>("Input index exceeds u16");
            // Process the inputs.
            match input_type {
                // A constant input is hashed (using `tcm`) to a field element.
                ValueType::Constant(..) => {
                    let input_id = InputID::constant(function_id, input, tcm, index)?;
                    // Add the input ID to the preimage.
                    message.push(*input_id.id());
                    // Add the input ID to the inputs.
                    input_ids.push(input_id);
                }
                // A public input is hashed (using `tcm`) to a field element.
                ValueType::Public(..) => {
                    let input_id = InputID::public(function_id, input, tcm, index)?;
                    // Add the input ID to the preimage.
                    message.push(*input_id.id());
                    // Add the input ID to the inputs.
                    input_ids.push(input_id);
                }
                // A private input is encrypted (using `tvk`) and hashed to a field element.
                ValueType::Private(..) => {
                    let input_id = InputID::private(function_id, input, tvk, index)?;
                    // Add the input ID to the preimage.
                    message.push(*input_id.id());
                    // Add the input ID to the inputs.
                    input_ids.push(input_id);
                }
                // A record input is computed to its serial number.
                ValueType::Record(record_name) => {
                    let input_id =
                        InputID::record(&program_id, record_name, input, &signer, &view_key, &sk_sig, sk_tag)?;

                    // Extract the components for the message.
                    if let InputID::Record(commitment, gamma, _, _, tag) = input_id {
                        // Compute the generator `H` as `HashToGroup(commitment)`.
                        let h = N::hash_to_group_psd2(&[N::serial_number_domain(), commitment])?;
                        // Compute `h_r` as `r * H`.
                        let h_r = h * r;

                        // Add (`H`, `r * H`, `gamma`, `tag`) to the preimage.
                        message.extend([h, h_r, gamma].iter().map(|point| point.to_x_coordinate()));
                        message.push(tag);

                        // Add the input ID.
                        input_ids.push(input_id);
                    } else {
                        bail!("Expected InputID::Record variant");
                    }
                }
                // An external record input is hashed (using `tvk`) to a field element.
                ValueType::ExternalRecord(..) => {
                    let input_id = InputID::external_record(function_id, input, tvk, index)?;
                    // Add the input ID to the preimage.
                    message.push(*input_id.id());
                    // Add the input ID to the inputs.
                    input_ids.push(input_id);
                }
                // A future is not a valid input.
                ValueType::Future(..) => bail!("A future is not a valid input"),
                // A dynamic record input is hashed (using `tvk`) to a field element.
                ValueType::DynamicRecord => {
                    let input_id = InputID::dynamic_record(function_id, input, tvk, index)?;
                    // Add the input ID to the preimage.
                    message.push(*input_id.id());
                    // Add the input ID to the inputs.
                    input_ids.push(input_id);
                }
                // A dynamic future is not a valid input.
                ValueType::DynamicFuture => bail!("A dynamic future is not a valid input"),
            }
        }

        // Compute `challenge` as `HashToScalar(r * G, pk_sig, pr_sig, signer, [tvk, tcm, function ID, is_root, program checksum?, input IDs, dynamic input IDs?])`.
        let challenge = N::hash_to_scalar_psd8(&message)?;
        // Compute `response` as `r - challenge * sk_sig`.
        let response = r - challenge * sk_sig;

        Ok(Self {
            signer,
            network_id,
            program_id,
            function_name,
            input_ids,
            inputs,
            signature: Signature::from((challenge, response, compute_key)),
            sk_tag,
            tvk,
            tcm,
            scm,
            caller_input_ids: None,
            caller_inputs: None,
            caller_output_types: None,
            caller_request: None,
        })
    }
}
