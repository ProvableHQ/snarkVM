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

use crate::vm::{
    VM,
    test_helpers::{CurrentAleo, CurrentNetwork, LedgerType},
};
use console::{
    account::{Address, ComputeKey, GraphKey, PrivateKey, Signature, ViewKey},
    prelude::*,
    program::{DynamicRecord, InputID, Plaintext, ProgramID, Record, Request, Value, ValueType, compute_function_id},
    types::{Field, Scalar, U16},
};
use snarkvm_ledger_block::{Execution, Output, Transition};
use snarkvm_synthesizer_process::Authorization;
use snarkvm_synthesizer_program::StackTrait;

use std::collections::HashMap;

// Populates and signs a mocked request (e.g. one produced by Request::sample), reusing its
// identifying data while recomputing tvk, tcm, scm, the input IDs, and the signature so that the
// resulting request is well-formed and verifies. The corrected input values are supplied
// explicitly via `inputs`; they must be correct for the resulting request to be valid. Note that
// the request's program, function name and signer must also be correct.
//
// Unlike Request::sign, the transition view key tvk is provided externally rather than being
// derived from the transition randomness r.
#[allow(clippy::too_many_arguments)]
pub(crate) fn populate_request_and_sign<N: Network, R: Rng + CryptoRng>(
    request: &Request<N>,
    private_key: &PrivateKey<N>,
    input_types: &[ValueType<N>],
    inputs: &[Value<N>],
    tvk: Field<N>,
    root_tvk: Option<Field<N>>,
    is_root: bool,
    program_checksum: Option<Field<N>>,
    rng: &mut R,
) -> Result<Request<N>> {
    // Reuse the mocked request's identifying data.
    let program_id = *request.program_id();
    let function_name = *request.function_name();
    let is_dynamic = request.is_dynamic();

    // Ensure the number of inputs matches the number of input types.
    if input_types.len() != inputs.len() {
        bail!(
            "'{program_id}/{function_name}' expects {} inputs, but {} were provided.",
            input_types.len(),
            inputs.len()
        )
    }

    let sk_sig = private_key.sk_sig();
    let compute_key = ComputeKey::try_from(private_key)?;
    let pk_sig = compute_key.pk_sig();
    let pr_sig = compute_key.pr_sig();
    let view_key = ViewKey::try_from((private_key, &compute_key))?;
    let sk_tag = GraphKey::try_from(view_key)?.sk_tag();

    let signer = Address::try_from(compute_key)?;
    ensure!(signer == *request.signer(), "The private key does not correspond to the mocked request's signer");

    // Sample a random nonce.
    let nonce = Field::<N>::rand(rng);
    // Compute `r` as `HashToScalar(sk_sig || nonce)`.
    // Unlike in the usual Request::sign method, this r is unrelated to the tvk.
    let r = N::hash_to_scalar_psd4(&[N::serial_number_domain(), sk_sig.to_field()?, nonce])?;
    // Compute `g_r` as `r * G`. Note: this is the transition public key `tpk`.
    let g_r = N::g_scalar_multiply(&r);

    // Compute the transition commitment `tcm` as `Hash(tvk)`.
    let tcm = N::hash_psd2(&[tvk])?;
    // Compute the signer commitment `scm` as `Hash(signer || root_tvk)`.
    let root_tvk = root_tvk.unwrap_or(tvk);
    let scm = N::hash_psd2(&[(*signer).to_x_coordinate(), root_tvk])?;
    // Compute 'is_root' as a field element.
    let is_root = if is_root { Field::<N>::one() } else { Field::<N>::zero() };

    // Retrieve the network ID.
    let network_id = U16::new(N::ID);
    // Compute the function ID.
    let function_id = compute_function_id(&network_id, &program_id, &function_name)?;

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

    // Compute the input IDs from the (already prepared) inputs.
    for (index, (input, input_type)) in inputs.iter().zip_eq(input_types).enumerate() {
        // Convert index to u16.
        let index = u16::try_from(index).map_err(|_| anyhow!("Input index exceeds u16"))?;

        match input_type {
            // A constant input is hashed (using `tcm`) to a field element.
            ValueType::Constant(..) => {
                let input_id = InputID::constant(function_id, input, tcm, index)?;
                message.push(*input_id.id());
                input_ids.push(input_id);
            }
            // A public input is hashed (using `tcm`) to a field element.
            ValueType::Public(..) => {
                let input_id = InputID::public(function_id, input, tcm, index)?;
                message.push(*input_id.id());
                input_ids.push(input_id);
            }
            // A private input is encrypted (using `tvk`) and hashed to a field element.
            ValueType::Private(..) => {
                let input_id = InputID::private(function_id, input, tvk, index)?;
                message.push(*input_id.id());
                input_ids.push(input_id);
            }
            // A record input is computed to its serial number.
            ValueType::Record(record_name) => {
                // Compute the input ID (commitment, gamma, record view key, serial number, tag).
                let input_id = InputID::record(&program_id, record_name, input, &signer, &view_key, &sk_sig, sk_tag)?;
                // Extract the commitment, gamma, and tag for the message.
                let (commitment, gamma, tag) = match &input_id {
                    InputID::Record(c, g, _, _, t) => (*c, *g, *t),
                    // InputID::record always returns the Record variant.
                    _ => unreachable!(),
                };
                // Compute the generator `H` as `HashToGroup(commitment)`.
                let h = N::hash_to_group_psd2(&[N::serial_number_domain(), commitment])?;
                // Compute `h_r` as `r * H`.
                let h_r = h * r;
                // Add (`H`, `r * H`, `gamma`, `tag`) to the preimage.
                message.extend([h, h_r, gamma].iter().map(|point| point.to_x_coordinate()));
                message.push(tag);
                input_ids.push(input_id);
            }
            // An external record input is hashed (using `tvk`) to a field element.
            ValueType::ExternalRecord(..) => {
                let input_id = InputID::external_record(function_id, input, tvk, index)?;
                message.push(*input_id.id());
                input_ids.push(input_id);
            }
            // A future is not a valid input.
            ValueType::Future(..) => bail!("A future is not a valid input"),
            // A dynamic record input is hashed (using `tvk`) to a field element.
            ValueType::DynamicRecord => {
                let input_id = InputID::dynamic_record(function_id, input, tvk, index)?;
                message.push(*input_id.id());
                input_ids.push(input_id);
            }
            // A dynamic future is not a valid input.
            ValueType::DynamicFuture => bail!("A dynamic future is not a valid input"),
        }
    }

    // Compute `challenge` as `HashToScalar(r * G, pk_sig, pr_sig, signer, [tvk, tcm, function ID, is_root, program checksum?, input IDs])`.
    let challenge = N::hash_to_scalar_psd8(&message)?;
    // Compute `response` as `r - challenge * sk_sig`.
    let response = r - challenge * sk_sig;

    // Construct the populated and signed request via the public tuple constructor.
    Ok(Request::from((
        signer,
        network_id,
        program_id,
        function_name,
        input_ids,
        inputs.to_vec(),
        Signature::from((challenge, response, compute_key)),
        sk_tag,
        tvk,
        tcm,
        scm,
        is_dynamic,
    )))
}

// Resolves the correct value of a record-bearing input slot of a reauthorized request.
//
// The mocked authorization derives intermediate records (those produced within the call tree) using
// mocked transition view keys, so their nonces (and thus commitments, serial numbers, and input
// IDs) do not match the execution. This function recovers the correct value by selecting, among the
// candidate records, the one whose recomputed input ID matches the corresponding transition input.
//
// The candidates are the mocked value (correct for records derived purely from the root inputs,
// since translation is tvk-independent) followed by the records decrypted from the execution's
// outputs (correct for records produced within the call tree, since every such record is emitted as
// a static record output by its birth transition).
#[allow(clippy::too_many_arguments)]
fn resolve_record_input(
    mock_value: &Value<CurrentNetwork>,
    value_type: &ValueType<CurrentNetwork>,
    target_id: &Field<CurrentNetwork>,
    output_records: &[Record<CurrentNetwork, Plaintext<CurrentNetwork>>],
    program_id: &ProgramID<CurrentNetwork>,
    function_id: Field<CurrentNetwork>,
    signer: &Address<CurrentNetwork>,
    view_key: &ViewKey<CurrentNetwork>,
    sk_sig: &Scalar<CurrentNetwork>,
    sk_tag: Field<CurrentNetwork>,
    tvk: Field<CurrentNetwork>,
    index: u16,
) -> Result<Value<CurrentNetwork>> {
    // Computes the transition-level input ID that the given candidate value would yield, returning
    // `None` if the candidate's shape is incompatible with the input type. For a record input, the
    // transition stores the serial number, so that is extracted; for external and dynamic record
    // inputs, the transition stores the input ID hash directly.
    let candidate_id = |value: &Value<CurrentNetwork>| -> Option<Field<CurrentNetwork>> {
        match value_type {
            ValueType::Record(record_name) => {
                match InputID::record(program_id, record_name, value, signer, view_key, sk_sig, sk_tag) {
                    Ok(InputID::Record(_, _, _, serial_number, _)) => Some(serial_number),
                    _ => None,
                }
            }
            ValueType::ExternalRecord(..) => {
                InputID::external_record(function_id, value, tvk, index).ok().map(|id| *id.id())
            }
            ValueType::DynamicRecord => InputID::dynamic_record(function_id, value, tvk, index).ok().map(|id| *id.id()),
            _ => None,
        }
    };

    // First, try the mocked value, which is correct for records derived purely from the root inputs.
    if candidate_id(mock_value) == Some(*target_id) {
        return Ok(mock_value.clone());
    }
    // Otherwise, search the records decrypted from the execution's outputs.
    for record in output_records {
        let value = match value_type {
            ValueType::Record(..) | ValueType::ExternalRecord(..) => Value::Record(record.clone()),
            ValueType::DynamicRecord => Value::DynamicRecord(DynamicRecord::from_record(record)?),
            _ => continue,
        };
        if candidate_id(&value) == Some(*target_id) {
            return Ok(value);
        }
    }
    bail!("Could not resolve the value of record input {index} for '{program_id}'")
}

// Reconstructs an authorization for the given execution, extracting suitable requests and calling
// authorize_requests. More specifically, this function:
// - recovers each transition's actual tvk from its tpk and the signer's view key,
// - samples a mocked authorization for the same root call via sample_authorization
//   In particular, the requests in the authoriztion have correct program IDs, function names and
//   input values (not IDs), but their tvk, tcm and signatures are mocked.
// - corrects the input values of records produced within the call tree, whose mocked nonces do not
//   match the execution, by decrypting the corresponding records from the execution's outputs,
// - populates each mocked request with the correct data derived from its recovered tvk (tcm, input
//   IDs and signature) via populate_request_and_sign
// - passes the populated requests to authorize_requests
pub(crate) fn reauthorize_from_execution(
    vm: &VM<CurrentNetwork, LedgerType>,
    execution: &Execution<CurrentNetwork>,
    root_inputs: &[Value<CurrentNetwork>],
    private_key: &PrivateKey<CurrentNetwork>,
    rng: &mut TestRng,
) -> Authorization<CurrentNetwork> {
    // Derive the signer's view key, address, and signing material.
    let view_key = ViewKey::try_from(private_key).unwrap();
    let signer = view_key.to_address();
    let sk_sig = private_key.sk_sig();
    let sk_tag = GraphKey::try_from(view_key).unwrap().sk_tag();

    // Decrypt every record emitted as a (static) record output of the execution. These provide the
    // correct values for records produced within the call tree, whose nonces depend on the
    // producing transition's tvk and so do not match the mocked authorization.
    let output_records: Vec<Record<CurrentNetwork, Plaintext<CurrentNetwork>>> = execution
        .transitions()
        .flat_map(|transition| transition.outputs())
        .filter_map(|output| match output {
            Output::Record(_, _, Some(record), _) | Output::RecordWithDynamicID(_, _, Some(record), _, _) => {
                record.decrypt(&view_key).ok()
            }
            _ => None,
        })
        .collect();

    // Recover the transition view keys (tvks) from the transitions' tpks, in post-order (the order
    // in which the transitions appear in the execution).
    let recovered_tvks: Vec<Field<CurrentNetwork>> =
        execution.transitions().map(|transition| (*transition.tpk() * *view_key).to_x_coordinate()).collect();
    // The root call is the last transition (post-order), and its tvk is the root tvk.
    let root_transition = execution.transitions().last().unwrap();
    let root_tvk = *recovered_tvks.last().unwrap();

    // Sample a mocked authorization for the same root call.
    let root_stack = vm.process().get_stack(*root_transition.program_id()).unwrap();
    let sampled = root_stack
        .sample_authorization::<CurrentAleo, _>(
            signer,
            *root_transition.program_id(),
            *root_transition.function_name(),
            root_inputs.iter(),
            rng,
        )
        .unwrap();

    // The mocked transitions are in the same post-order as the execution's transitions, so map each
    // mocked transition's tcm to the recovered tvk and execution transition by position. Each mocked
    // request is then matched to its tvk and transition via its (mocked) tcm.
    let tcm_to_tvk: HashMap<Field<CurrentNetwork>, Field<CurrentNetwork>> = sampled
        .transitions()
        .values()
        .zip_eq(recovered_tvks.iter().copied())
        .map(|(transition, tvk)| (*transition.tcm(), tvk))
        .collect();
    let tcm_to_transition: HashMap<Field<CurrentNetwork>, &Transition<CurrentNetwork>> = sampled
        .transitions()
        .values()
        .zip_eq(execution.transitions())
        .map(|(mock, real)| (*mock.tcm(), real))
        .collect();

    // Populate each mocked request with the correct data derived from its recovered tvk.
    let populated_requests: Vec<Request<CurrentNetwork>> = sampled
        .to_vec_deque()
        .into_iter()
        .map(|request| {
            // Recover this request's tvk and execution transition via its (mocked) tcm.
            let tvk = *tcm_to_tvk.get(request.tcm()).expect("every mocked request has a matching transition");
            let transition = *tcm_to_transition.get(request.tcm()).expect("every mocked request has a transition");
            // Look up the callee program's stack, function input types, and program checksum.
            let stack = vm.process().get_stack(*request.program_id()).unwrap();
            let input_types = stack.get_function(request.function_name()).unwrap().input_types();
            let program_checksum = match stack.program().contains_constructor() {
                true => Some(stack.program_checksum_as_field().unwrap()),
                false => None,
            };
            // The root request is the one whose tvk matches the root tvk.
            let is_root = tvk == root_tvk;
            // Compute the function ID used to recompute external and dynamic record input IDs.
            let function_id =
                compute_function_id(&U16::new(CurrentNetwork::ID), request.program_id(), request.function_name())
                    .unwrap();
            // Correct the value of each record-bearing input, leaving other inputs untouched.
            let corrected_inputs: Vec<Value<CurrentNetwork>> = request
                .inputs()
                .iter()
                .zip_eq(input_types.iter())
                .enumerate()
                .map(|(index, (mock_value, value_type))| match value_type {
                    ValueType::Record(..) | ValueType::ExternalRecord(..) | ValueType::DynamicRecord => {
                        let target_id = transition.inputs()[index].id();
                        resolve_record_input(
                            mock_value,
                            value_type,
                            target_id,
                            &output_records,
                            request.program_id(),
                            function_id,
                            &signer,
                            &view_key,
                            &sk_sig,
                            sk_tag,
                            tvk,
                            u16::try_from(index).unwrap(),
                        )
                        .unwrap()
                    }
                    _ => mock_value.clone(),
                })
                .collect();
            // Populate and sign the request.
            populate_request_and_sign(
                &request,
                private_key,
                &input_types,
                &corrected_inputs,
                tvk,
                Some(root_tvk),
                is_root,
                program_checksum,
                rng,
            )
            .unwrap()
        })
        .collect();

    // Authorize from the populated requests.
    root_stack.authorize_requests::<CurrentAleo, _>(populated_requests, rng).unwrap()
}
