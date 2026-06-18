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

use console::{
    account::{ComputeKey, GraphKey, Signature, ViewKey},
    program::{DynamicRecord, InputID, Plaintext, Request, compute_function_id},
};

use super::*;

pub(crate) fn construct_authorization(
    vm: &VM<CurrentNetwork, LedgerType>,
    private_key: PrivateKey<CurrentNetwork>,
    root_program_id: ProgramID<CurrentNetwork>,
    root_function_name: Identifier<CurrentNetwork>,
    root_inputs: &[Value<CurrentNetwork>],
    rng: &mut TestRng,
) -> Authorization<CurrentNetwork> {
    let signer = Address::try_from(&private_key).unwrap();

    // Derive key material. Different protocol flows and party setups will structure how this is handled differently
    let compute_key = ComputeKey::try_from(&private_key).unwrap();
    let view_key = ViewKey::try_from(&private_key).unwrap();
    // sk_tag is needed
    //  - If any Request has a static Record input, in step 2.4 (to compute the corresponding input ID)
    //  - At the end of Request population, since it is one of the fields of the Request object
    let sk_tag = GraphKey::try_from(view_key).unwrap().sk_tag();
    // sk_sig is needed
    //  - If any Request has a static Record input, in step 2.4 (to compute the value gamma, in turned used to compute the serial number)
    //  - In step 2.5, to produce the request signature
    let sk_sig = private_key.sk_sig();
    // pk_sig and pr_sig are only needed in step 2.5, since they are part of the prefix of the message being signed
    let pk_sig = compute_key.pk_sig();
    let pr_sig = compute_key.pr_sig();

    // ********************************************************************************
    // Step 1: Produce the mock authorization from the root-call information and
    //         extract the requests and record-tracking information

    let (mock_authorization, record_tracking, record_names, program_checksums) = vm
        .process()
        .get_stack(root_program_id)
        .unwrap()
        .sample_authorization_extended::<CurrentAleo, _>(
            signer,
            root_program_id,
            root_function_name,
            root_inputs.iter(),
            rng,
        )
        .unwrap();

    let mock_requests = mock_authorization.to_vec_deque();

    // ********************************************************************************************
    // Step 2: Populate the requests using the record-tracking information. Note that, in general,
    //         we do not need to generate all the tvks at the start.
    //
    // Summary of the fields of a Request:
    //  - signer: Address<N>,           // Already ok in the mocked request
    //  - network_id: U16<N>,           // Already ok in the mocked request
    //  - program_id: ProgramID<N>,     // Already ok in the mocked request
    //  - function_name: Identifier<N>, // Already ok in the mocked request
    //  - input_ids: Vec<InputID<N>>,   // Computed in step 2.4
    //  - inputs: Vec<Value<N>>,        // Computed in step 2.2
    //  - signature: Signature<N>,      // Computed in step 2.5
    //  - sk_tag: Field<N>,             // Computed above
    //  - tvk: Field<N>,                // Sampled in step 2.1
    //  - tcm: Field<N>,                // Computed in step 2.3
    //  - scm: Field<N>,                // Computed in step 2.3
    //  - is_dynamic: bool,             // Already ok in the mocked request

    // TODO (Antonio) maybe tvks can be removed
    let mut tvks = Vec::with_capacity(mock_requests.len());
    let mut populated_requests: Vec<Request<CurrentNetwork>> = Vec::with_capacity(mock_requests.len());

    for (request_index, mock_request) in mock_requests.iter().enumerate() {
        // Determine whether this request corresponds to the root call (the first request).
        let is_root = request_index == 0;
        let is_root_field = if is_root { Field::<CurrentNetwork>::one() } else { Field::<CurrentNetwork>::zero() };

        // ****************************************************************************************
        // Step 2.1: Derive a tvk with some mechanism
        let tvk = Field::rand(rng);
        tvks.push(tvk);

        // ****************************************************************************************
        // Step 2.2: Correct the inputs which are dependent on previous tvks: Record, (incl.
        //           ExternalRecord) and DynamicRecord

        let corrected_inputs = mock_request
            .inputs()
            .iter()
            .enumerate()
            .map(|(input_index, input)| {
                match input {
                    Value::Record(_) | Value::DynamicRecord(_) => {
                        // Use the tracking information to find the index (in mocked_requests) of the
                        // request which minted the corresponding static record and its output register
                        // *if* that happened in the transaction being processed. Otherwise, the
                        // static/dynamic/external record ultimately comes from a root-call-input static
                        // record and it is already correct
                        if let Some((minter_request_index, output_register)) =
                            record_tracking.get(&(request_index, input_index))
                        {
                            // Compute the updated nonce with the tvk used to populate the minting request
                            let minter_tvk = tvks[*minter_request_index];
                            let index = Field::from_u64(*output_register);
                            let randomizer = CurrentNetwork::hash_to_scalar_psd2(&[minter_tvk, index]).unwrap();
                            let nonce = CurrentNetwork::g_scalar_multiply(&randomizer);

                            match input {
                                Value::Record(record) => Value::Record(
                                    Record::<CurrentNetwork, Plaintext<CurrentNetwork>>::from_plaintext(
                                        record.owner().clone(),
                                        record.data().clone(),
                                        nonce,
                                        *record.version(),
                                    )
                                    .unwrap(),
                                ),
                                Value::DynamicRecord(dynamic_record) => {
                                    Value::DynamicRecord(DynamicRecord::new_unchecked(
                                        *dynamic_record.owner(),
                                        *dynamic_record.root(),
                                        nonce,
                                        *dynamic_record.version(),
                                        dynamic_record.data().clone(),
                                    ))
                                }
                                _ => unreachable!(),
                            }
                        } else {
                            input.clone()
                        }
                    }
                    _ => input.clone(),
                }
            })
            .collect_vec();

        // ****************************************************************************************
        // Step 2.3: Derive the tcm and scm

        let root_tvk = tvks[0];
        let tcm = CurrentNetwork::hash_psd2(&[tvk]).unwrap();
        let scm = CurrentNetwork::hash_psd2(&[signer.to_field().unwrap(), root_tvk]).unwrap();

        // ****************************************************************************************
        // Step 2.4 (Requires private key material if any inputs are of type Record)
        //          Derive the input IDs (of the corrected inputs) using the updated tvk

        // Compute the function ID.
        let function_id =
            compute_function_id(mock_request.network_id(), mock_request.program_id(), mock_request.function_name())
                .unwrap();

        // We only use the mocked request's input IDs to determine the type of each input.
        // Note that the value of the input is not quite enough: for instance, an input with value
        // of type Value::Record can correspond to a Record or an ExternalRecord input.
        let corrected_input_ids = corrected_inputs
            .iter()
            .zip_eq(mock_request.input_ids().iter())
            .enumerate()
            .map(|(input_index, (input, mock_input_id))| {
                // The (console) input index as a field element.
                let index_field = Field::from_u16(u16::try_from(input_index).unwrap());

                match (input, mock_input_id) {
                    // A constant input is hashed (using `tcm`) to a field element as
                    // `Hash(function_id || input || tcm || index)`.
                    (Value::Plaintext(_), InputID::Constant(_)) => {
                        let mut preimage = Vec::new();
                        preimage.push(function_id);
                        preimage.extend(input.to_fields().unwrap());
                        preimage.push(tcm);
                        preimage.push(index_field);
                        InputID::Constant(CurrentNetwork::hash_psd8(&preimage).unwrap())
                    }
                    // A public input is hashed (using `tcm`) to a field element as
                    // `Hash(function_id || input || tcm || index)`.
                    (Value::Plaintext(_), InputID::Public(_)) => {
                        let mut preimage = Vec::new();
                        preimage.push(function_id);
                        preimage.extend(input.to_fields().unwrap());
                        preimage.push(tcm);
                        preimage.push(index_field);
                        InputID::Public(CurrentNetwork::hash_psd8(&preimage).unwrap())
                    }
                    // A private input is encrypted (using the input view key derived from `tvk`)
                    // and the ciphertext is hashed to a field element.
                    (Value::Plaintext(plaintext), InputID::Private(_)) => {
                        // Compute the input view key as `Hash(function_id || tvk || index)`.
                        let input_view_key = CurrentNetwork::hash_psd4(&[function_id, tvk, index_field]).unwrap();
                        // Encrypt the input and hash the ciphertext to a field element.
                        let ciphertext = plaintext.encrypt_symmetric(input_view_key).unwrap();
                        InputID::Private(CurrentNetwork::hash_psd8(&ciphertext.to_fields().unwrap()).unwrap())
                    }
                    // A record input is computed to its serial number.
                    (Value::Record(record), InputID::Record(..)) => {
                        // The record name should be provided in record_names
                        let record_name = record_names.get(&(request_index, input_index)).unwrap();
                        // Compute the record view key and commitment.
                        let record_view_key = (*record.nonce() * *view_key).to_x_coordinate();
                        let commitment =
                            record.to_commitment(mock_request.program_id(), record_name, &record_view_key).unwrap();
                        // Compute the generator `H` as `HashToGroup(commitment)` and `gamma` as `sk_sig * H`.
                        let h =
                            CurrentNetwork::hash_to_group_psd2(&[CurrentNetwork::serial_number_domain(), commitment])
                                .unwrap();
                        let gamma = h * sk_sig;
                        // Compute the serial number (from `gamma`) and the tag.
                        let serial_number =
                            Record::<CurrentNetwork, Plaintext<CurrentNetwork>>::serial_number_from_gamma(
                                &gamma, commitment,
                            )
                            .unwrap();
                        let tag = Record::<CurrentNetwork, Plaintext<CurrentNetwork>>::tag(sk_tag, commitment).unwrap();
                        InputID::Record(commitment, gamma, record_view_key, serial_number, tag)
                    }
                    // An external record input is hashed (using `tvk`) to a field element as
                    // `Hash(function_id || input || tvk || index)`.
                    (Value::Record(_), InputID::ExternalRecord(_)) => {
                        let mut preimage = Vec::new();
                        preimage.push(function_id);
                        preimage.extend(input.to_fields().unwrap());
                        preimage.push(tvk);
                        preimage.push(index_field);
                        InputID::ExternalRecord(CurrentNetwork::hash_psd8(&preimage).unwrap())
                    }
                    // A dynamic record input is hashed (using `tvk`) to a field element as
                    // `Hash(function_id || input || tvk || index)`.
                    (Value::DynamicRecord(_), InputID::DynamicRecord(_)) => {
                        let mut preimage = Vec::new();
                        preimage.push(function_id);
                        preimage.extend(input.to_fields().unwrap());
                        preimage.push(tvk);
                        preimage.push(index_field);
                        InputID::DynamicRecord(CurrentNetwork::hash_psd8(&preimage).unwrap())
                    }
                    // Any other combination of input value and input type is invalid.
                    _ => panic!("The input value and input type combination is invalid"),
                }
            })
            .collect_vec();

        // ****************************************************************************************
        // Step 2.5 (Requires private key material)
        //           Sign the request.

        // Sample the transition secret `r` and compute `g_r` as `r * G`.
        let nonce = Field::rand(rng);
        let r = CurrentNetwork::hash_to_scalar_psd4(&[
            CurrentNetwork::serial_number_domain(),
            sk_sig.to_field().unwrap(),
            nonce,
        ])
        .unwrap();
        let g_r = CurrentNetwork::g_scalar_multiply(&r);

        // Construct the signature message as
        // (g_r, pk_sig, pr_sig, signer, [tvk, tcm, function ID, is_root, program checksum?, input IDs]).
        let mut message = Vec::with_capacity(9 + 2 * corrected_input_ids.len());
        message.extend([g_r, pk_sig, pr_sig, *signer].map(|point| point.to_x_coordinate()));
        message.extend([tvk, tcm, function_id, is_root_field]);
        // Add the program checksum to the message if it was provided.
        if let Some(program_checksum) = program_checksums.get(&request_index) {
            message.push(*program_checksum);
        }
        // Append each input ID's contribution to the message.
        for input_id in corrected_input_ids.iter() {
            match input_id {
                // A record input contributes `(H, r * H, gamma, tag)`.
                InputID::Record(commitment, gamma, _, _, tag) => {
                    let h = CurrentNetwork::hash_to_group_psd2(&[CurrentNetwork::serial_number_domain(), *commitment])
                        .unwrap();
                    let h_r = h * r;
                    message.extend([h, h_r, *gamma].iter().map(|point| point.to_x_coordinate()));
                    message.push(*tag);
                }
                // All other inputs contribute their (single) input ID field.
                _ => message.push(*input_id.id()),
            }
        }

        // Compute `challenge` as `HashToScalar(message)` and `response` as `r - challenge * sk_sig`.
        let challenge = CurrentNetwork::hash_to_scalar_psd8(&message).unwrap();
        let response = r - challenge * sk_sig;
        let signature = Signature::from((challenge, response, compute_key));

        // ****************************************************************************************
        // Step 2.6: Construct the signed request from the computed values.

        let request = Request::from((
            signer,
            *mock_request.network_id(),
            *mock_request.program_id(),
            *mock_request.function_name(),
            corrected_input_ids,
            corrected_inputs,
            signature,
            sk_tag,
            tvk,
            tcm,
            scm,
            mock_request.is_dynamic(),
        ));

        populated_requests.push(request);
    }

    // ********************************************************************************************
    // Step 3: Call authorize_requests to obten correct authorizations

    vm.process()
        .get_stack(root_program_id)
        .unwrap()
        .authorize_requests::<CurrentAleo, _>(populated_requests, rng)
        .unwrap()
}
